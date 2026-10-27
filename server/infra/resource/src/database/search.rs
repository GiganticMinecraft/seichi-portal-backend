use crate::database::meilisearch_http::{MeilisearchClient, TimeoutHttpClient};
use crate::database::{
    components::{FormAnswerDatabase, SearchDatabase},
    connection::ConnectionPool,
};
use crate::records::FormAnswerRecord;
use async_trait::async_trait;
use domain::{
    account::models::UserId,
    form::{
        answer::{AnswerEntry, AnswerId, AnswerStatus, FormAnswerContentId},
        comment::CommentId,
        models::FormId,
        question::QuestionId,
    },
    search::models::{
        AnswerLabelSearchHit, AnswerSearchHit, AnswerTitleSearchDocument, CommentSearchHit,
        FormLabelSearchHit, FormSearchHit, NumberOfRecords, NumberOfRecordsPerAggregate, Operation,
        SearchIndex, SearchableFields, SearchableFieldsWithOperation, UserSearchHit,
    },
};
use errors::infra::InfraError;
use futures::{StreamExt, TryStreamExt, future::try_join_all, stream, try_join};
use itertools::{Either, Itertools};
use meilisearch_sdk::{
    documents::DocumentsQuery,
    errors::{Error as MeilisearchError, ErrorCode},
    indexes::Index,
    search::{SearchQuery, Selectors},
    settings::{LocalizedAttributes, Settings},
    tasks::Task,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use uuid::Uuid;

/// 1 リクエストでまとめて投入するドキュメント数の上限。
///
/// 1 ドキュメントごとにリクエストを投げると、再同期のように大量のドキュメントを扱うときに
/// 同時接続数と検索エンジン側のタスク数がドキュメント数分だけ増えてしまうため、必ずまとめて投入する。
const SYNC_DOCUMENT_CHUNK_SIZE: usize = 1_000;
/// 検索エンジンへ同時に投げるリクエスト数の上限。
const SYNC_REQUEST_CONCURRENCY: usize = 4;
const SYNC_TASK_POLL_INTERVAL: Duration = Duration::from_millis(200);
const SYNC_TASK_TIMEOUT: Duration = Duration::from_secs(120);
/// 検索エンジンからドキュメント ID を取得するときの 1 リクエストあたりの件数。
const INDEXED_ID_FETCH_LIMIT: usize = 1_000;
/// 回答を投影しているインデックス。`form_id` と `status` で絞り込めるようにしている。
const ANSWER_SEARCH_INDEXES: [SearchIndex; 2] = [SearchIndex::Answers, SearchIndex::RealAnswers];
/// 1 回の検索で検索エンジンから受け取るヒット数の上限。
///
/// 検索エンジンの既定値 (20 件) のままだと、呼び出し側が権限で除外したあとに残る件数が
/// 少なすぎる。一方でフロントエンドは検索結果をページングせずに表示し、usecase 側は
/// ヒットごとに回答本文やコメントスレッドを引くため、無制限に増やすと応答が重くなる。
const SEARCH_HIT_LIMIT: usize = 100;
/// 検索対象の言語。
///
/// 日本語の分かち書きを明示し、クエリとドキュメントで同じトークナイザを使わせる。
/// 自動判定に任せると短い漢字の語が中国語と判定され、トークンが食い違ってヒットしないことがある。
const SEARCH_LOCALES: [&str; 1] = ["jpn"];
/// 検索で受け取る属性。ヒットの中身は使わず ID だけ使うので、本文は受け取らない。
const ID_ONLY: [&str; 1] = ["id"];
const ANSWER_ID_ONLY: [&str; 1] = ["answer_id"];
const ID_AND_ANSWER_ID: [&str; 2] = ["id", "answer_id"];

#[derive(Serialize, Deserialize)]
struct AnswerContentSearchDocument {
    id: FormAnswerContentId,
    form_id: FormId,
    answer_id: AnswerId,
    question_id: QuestionId,
    answer: String,
    status: AnswerStatus,
}

#[derive(Deserialize)]
struct IndexedDocumentId {
    id: Uuid,
}

#[derive(Deserialize)]
struct AnswerContentHit {
    answer_id: AnswerId,
}

#[derive(Deserialize)]
struct CommentHit {
    id: CommentId,
    answer_id: AnswerId,
}

/// 検索エンジンへ反映したいドキュメント 1 件分の操作。
enum SearchDocumentWrite {
    Upsert(SearchIndex, serde_json::Value),
    Delete(SearchIndex, String),
}

/// 同じインデックスへの [`SearchDocumentWrite`] をまとめた、検索エンジンへの 1 リクエスト。
enum SearchDocumentRequest {
    Upsert {
        index: SearchIndex,
        documents: Vec<serde_json::Value>,
    },
    Delete {
        index: SearchIndex,
        ids: Vec<String>,
    },
}

impl SearchDocumentRequest {
    async fn execute(self, client: &MeilisearchClient) -> Result<(), InfraError> {
        let task = match self {
            Self::Upsert { index, documents } => {
                client
                    .index(index.as_str())
                    .add_or_replace(&documents, Some("id"))
                    .await?
            }
            Self::Delete { index, ids } => {
                client.index(index.as_str()).delete_documents(&ids).await?
            }
        };

        // 投入は 202 が返るだけで完了を保証しないため、タスクの完了まで待って結果を確認する。
        // ここで待たないと、失敗した投入を再試行できず、件数比較による同期判定も投入直後に必ず外れる
        let task = task
            .wait_for_completion(
                client,
                Some(SYNC_TASK_POLL_INTERVAL),
                Some(SYNC_TASK_TIMEOUT),
            )
            .await?;

        ensure_meilisearch_task_succeeded(task, false).map_err(Into::into)
    }
}

/// インデックスごとにまとめたドキュメントを [`SYNC_DOCUMENT_CHUNK_SIZE`] 件単位のリクエストに分割する。
fn chunked_requests<T: Clone>(
    grouped: HashMap<SearchIndex, Vec<T>>,
    into_request: impl Fn(SearchIndex, Vec<T>) -> SearchDocumentRequest,
) -> impl Iterator<Item = SearchDocumentRequest> {
    grouped.into_iter().flat_map(move |(index, values)| {
        values
            .chunks(SYNC_DOCUMENT_CHUNK_SIZE)
            .map(|chunk| into_request(index, chunk.to_vec()))
            .collect_vec()
    })
}

/// [`SearchDocumentWrite`] をインデックスごとにまとめ、リクエスト単位に分割する。
fn search_document_requests(
    writes: impl IntoIterator<Item = SearchDocumentWrite>,
) -> Vec<SearchDocumentRequest> {
    let (upserts, deletes): (Vec<_>, Vec<_>) =
        writes.into_iter().partition_map(|write| match write {
            SearchDocumentWrite::Upsert(index, document) => Either::Left((index, document)),
            SearchDocumentWrite::Delete(index, id) => Either::Right((index, id)),
        });

    chunked_requests(upserts.into_iter().into_group_map(), |index, documents| {
        SearchDocumentRequest::Upsert { index, documents }
    })
    .chain(chunked_requests(
        deletes.into_iter().into_group_map(),
        |index, ids| SearchDocumentRequest::Delete { index, ids },
    ))
    .collect()
}

async fn execute_search_document_requests(
    client: &MeilisearchClient,
    requests: Vec<SearchDocumentRequest>,
) -> Result<(), InfraError> {
    stream::iter(requests)
        .map(|request| request.execute(client))
        .buffer_unordered(SYNC_REQUEST_CONCURRENCY)
        .try_collect()
        .await
}

fn upsert_document(fields: &SearchableFields) -> Result<serde_json::Value, InfraError> {
    Ok(match fields {
        SearchableFields::FormMetaData(data) => serde_json::to_value(data)?,
        SearchableFields::AnswerTitle(_) => {
            unreachable!("answer title updates are handled by database reprojection")
        }
        SearchableFields::RealAnswers(_) => {
            unreachable!("answer contents are projected with the answer's form id and status")
        }
        SearchableFields::FormAnswerComments(comments) => serde_json::to_value(comments)?,
        SearchableFields::LabelForFormAnswers(label) => serde_json::to_value(label)?,
        SearchableFields::LabelForForms(label) => serde_json::to_value(label)?,
        SearchableFields::Users(users) => serde_json::to_value(users)?,
    })
}

#[derive(Deserialize)]
struct FormIdPresence {
    #[serde(default)]
    form_id: Option<FormId>,
    #[serde(default)]
    status: Option<AnswerStatus>,
}

fn answer_filter(form_id: Option<FormId>, status: Option<AnswerStatus>) -> Option<String> {
    [
        form_id.map(|id| format!("form_id = \"{id}\"")),
        status.map(|status| format!("status = \"{status}\"")),
    ]
    .into_iter()
    .flatten()
    .reduce(|left, right| format!("{left} AND {right}"))
}

/// 回答本文のイベントを、回答の現在の状態 (`metadata_by_answer_id`) をもとに検索ドキュメントへ
/// 投影する。
///
/// 回答がすでに存在しない (削除またはアーカイブ済み) 本文は結果に含まれない。CDC のイベントは
/// 本文が作られた時点のもので、同期するころには回答が消えていることがあるため、呼び出し側は
/// 含まれなかった本文を検索エンジンから削除する。
fn answer_content_documents(
    data: &[SearchableFieldsWithOperation],
    metadata_by_answer_id: &HashMap<AnswerId, (FormId, AnswerStatus)>,
) -> HashMap<FormAnswerContentId, AnswerContentSearchDocument> {
    data.iter()
        .filter_map(|(fields, operation)| match (fields, operation) {
            (SearchableFields::RealAnswers(content), Operation::Create | Operation::Update) => {
                Some(content)
            }
            _ => None,
        })
        .filter_map(|content| {
            let (form_id, status) = metadata_by_answer_id.get(&content.answer_id).copied()?;
            Some((
                content.id,
                AnswerContentSearchDocument {
                    id: content.id,
                    form_id,
                    answer_id: content.answer_id,
                    question_id: content.question_id,
                    answer: content.answer.clone(),
                    status,
                },
            ))
        })
        .collect()
}

fn answer_metadata_from_records(
    records: &[FormAnswerRecord],
) -> Result<HashMap<AnswerId, (FormId, AnswerStatus)>, InfraError> {
    records
        .iter()
        .map(|record| {
            let status = AnswerStatus::try_from(record.status.clone()).map_err(|error| {
                InfraError::Unexpected {
                    cause: error.to_string(),
                }
            })?;
            Ok((
                Uuid::parse_str(&record.id)?.into(),
                (Uuid::parse_str(&record.form_id)?.into(), status),
            ))
        })
        .collect()
}

fn answer_documents_from_entry(entry: &AnswerEntry) -> Vec<SearchableFieldsWithOperation> {
    let title = SearchableFields::AnswerTitle(AnswerTitleSearchDocument {
        id: *entry.id(),
        form_id: *entry.form_id(),
        title: entry.title().clone(),
        status: *entry.status(),
    });
    let contents = entry.contents().iter().map(|content| {
        (
            SearchableFields::RealAnswers(domain::search::models::RealAnswers {
                id: content.id,
                answer_id: *entry.id(),
                question_id: content.question_id,
                answer: content.answer.clone(),
                status: *entry.status(),
            }),
            Operation::Update,
        )
    });
    std::iter::once((title, Operation::Update))
        .chain(contents)
        .collect()
}

fn answer_documents_from_record(
    record: FormAnswerRecord,
) -> Result<Vec<SearchableFieldsWithOperation>, InfraError> {
    let entry: AnswerEntry =
        record
            .try_into()
            .map_err(|error: errors::Error| InfraError::Unexpected {
                cause: error.to_string(),
            })?;
    Ok(answer_documents_from_entry(&entry))
}

async fn answer_documents_need_reprojection(
    connection: &ConnectionPool,
) -> Result<bool, InfraError> {
    for index in ANSWER_SEARCH_INDEXES {
        let missing_form_id = connection
            .meilisearch_client
            .index(index.as_str())
            .search()
            .with_filter("form_id NOT EXISTS OR status NOT EXISTS")
            .with_limit(1)
            .execute::<FormIdPresence>()
            .await?
            .hits
            .into_iter()
            .any(|hit| hit.result.form_id.is_none() || hit.result.status.is_none());
        if missing_form_id {
            return Ok(true);
        }
    }

    Ok(false)
}

fn merge_answer_hits(
    title_answer_ids: impl IntoIterator<Item = AnswerId>,
    content_answer_ids: impl IntoIterator<Item = AnswerId>,
) -> Vec<AnswerSearchHit> {
    title_answer_ids
        .into_iter()
        .chain(content_answer_ids)
        .map(|answer_id| AnswerSearchHit { answer_id })
        .unique_by(|hit| hit.answer_id)
        .collect()
}

/// 検索リクエストを組み立てる。ヒット数の上限、言語、受け取る属性をすべての検索でそろえる。
fn configured_search<'a>(
    index: &'a Index<TimeoutHttpClient>,
    query: &'a str,
    attributes_to_retrieve: &'a [&'a str],
    filter: Option<&'a str>,
) -> SearchQuery<'a, TimeoutHttpClient> {
    let mut search = index.search();
    search
        .with_query(query)
        .with_limit(SEARCH_HIT_LIMIT)
        .with_locales(&SEARCH_LOCALES)
        .with_attributes_to_retrieve(Selectors::Some(attributes_to_retrieve));
    if let Some(filter) = filter {
        search.with_filter(filter);
    }
    search
}

/// 各インデックスに適用する設定。
///
/// - `searchableAttributes`: 本文に当たる属性だけを検索対象にする。既定 (`*`) では ID や
///   ステータスも検索対象になり、`COMPLETED` のような語で全件がヒットしてしまう
/// - `localizedAttributes`: [`SEARCH_LOCALES`] の分かち書きをドキュメント側にも明示する
/// - `filterableAttributes`: 回答のインデックスは `form_id` と `status` で絞り込む
fn index_settings(index: SearchIndex) -> Settings {
    let searchable_attributes: &[&str] = match index {
        SearchIndex::FormMetaData => &["title", "description"],
        SearchIndex::Answers => &["title"],
        SearchIndex::RealAnswers => &["answer"],
        SearchIndex::FormAnswerComments => &["content"],
        SearchIndex::LabelForFormAnswers | SearchIndex::LabelForForms | SearchIndex::Users => {
            &["name"]
        }
    };
    let settings = Settings::new()
        .with_searchable_attributes(searchable_attributes)
        .with_localized_attributes([LocalizedAttributes {
            locales: SEARCH_LOCALES.iter().map(ToString::to_string).collect(),
            attribute_patterns: vec!["*".to_string()],
        }]);

    if ANSWER_SEARCH_INDEXES.contains(&index) {
        settings.with_filterable_attributes(["form_id", "status"])
    } else {
        settings
    }
}

/// `desired` で指定した設定が `current` にすべて反映されているか。
///
/// 指定していない設定 (`None`) は比較しない。
fn settings_match(current: &Settings, desired: &Settings) -> bool {
    current.searchable_attributes == desired.searchable_attributes
        && current.localized_attributes == desired.localized_attributes
        && desired
            .filterable_attributes
            .as_ref()
            .is_none_or(|filterable| current.filterable_attributes.as_ref() == Some(filterable))
}

fn is_index_not_found(error: &MeilisearchError) -> bool {
    matches!(error, MeilisearchError::Meilisearch(error) if error.error_code == ErrorCode::IndexNotFound)
}

fn ensure_meilisearch_task_succeeded(
    task: Task,
    allow_index_already_exists: bool,
) -> Result<(), MeilisearchError> {
    if task.is_failure() {
        let error = task.unwrap_failure();
        if allow_index_already_exists && error.error_code == ErrorCode::IndexAlreadyExists {
            return Ok(());
        }

        return Err(MeilisearchError::Meilisearch(error));
    }

    if matches!(task, Task::Succeeded { .. }) {
        return Ok(());
    }

    Err(MeilisearchError::Other(Box::new(std::io::Error::other(
        "Meilisearch task did not finish successfully",
    ))))
}

impl ConnectionPool {
    /// `index` を検索し、ヒットしたドキュメントの ID を返す。
    async fn search_document_ids(
        &self,
        index: SearchIndex,
        query: &str,
    ) -> Result<Vec<Uuid>, InfraError> {
        let index = self.meilisearch_client.index(index.as_str());
        Ok(configured_search(&index, query, &ID_ONLY, None)
            .execute::<IndexedDocumentId>()
            .await?
            .hits
            .into_iter()
            .map(|hit| hit.result.id)
            .collect())
    }

    /// インデックスが無ければ作る。
    ///
    /// 毎回 `create_index` を投げると、既存のときに失敗したタスクが検索エンジンのタスク履歴に
    /// 残り続けるため、先に存在を確認する。
    async fn ensure_search_index(&self, index: SearchIndex) -> Result<(), InfraError> {
        match self.meilisearch_client.get_index(index.as_str()).await {
            Ok(_) => return Ok(()),
            Err(error) if is_index_not_found(&error) => {}
            Err(error) => return Err(error.into()),
        }

        let task = self
            .meilisearch_client
            .create_index(index.as_str(), Some("id"))
            .await?
            .wait_for_completion(
                &self.meilisearch_client,
                Some(SYNC_TASK_POLL_INTERVAL),
                Some(SYNC_TASK_TIMEOUT),
            )
            .await?;

        // 存在確認と作成の間に別のプロセスが作った場合だけ already exists になる
        ensure_meilisearch_task_succeeded(task, true).map_err(Into::into)
    }

    /// インデックスの設定が [`index_settings`] と違うときだけ更新する。
    ///
    /// 設定の更新はドキュメントの再インデックスを伴うため、変わっていなければ投げない。
    async fn ensure_search_index_settings(&self, index: SearchIndex) -> Result<(), InfraError> {
        let meilisearch_index = self.meilisearch_client.index(index.as_str());
        let desired = index_settings(index);
        let current = meilisearch_index.get_settings().await?;
        if settings_match(&current, &desired) {
            return Ok(());
        }

        tracing::info!(index = index.as_str(), "updating search index settings");
        let task = meilisearch_index
            .set_settings(&desired)
            .await?
            .wait_for_completion(
                &self.meilisearch_client,
                Some(SYNC_TASK_POLL_INTERVAL),
                Some(SYNC_TASK_TIMEOUT),
            )
            .await?;

        ensure_meilisearch_task_succeeded(task, false).map_err(Into::into)
    }
}

#[async_trait]
impl SearchDatabase for ConnectionPool {
    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "search", db.collection.name = "users"))]
    async fn search_users(&self, query: &str) -> Result<Vec<UserSearchHit>, InfraError> {
        Ok(self
            .search_document_ids(SearchIndex::Users, query)
            .await?
            .into_iter()
            .map(|id| UserSearchHit {
                user_id: UserId::from(id),
            })
            .collect_vec())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "search", db.collection.name = "form_meta_data"))]
    async fn search_forms(&self, query: &str) -> Result<Vec<FormSearchHit>, InfraError> {
        Ok(self
            .search_document_ids(SearchIndex::FormMetaData, query)
            .await?
            .into_iter()
            .map(|id| FormSearchHit {
                form_id: FormId::from(id),
            })
            .collect_vec())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "search", db.collection.name = "label_for_forms"))]
    async fn search_labels_for_forms(
        &self,
        query: &str,
    ) -> Result<Vec<FormLabelSearchHit>, InfraError> {
        Ok(self
            .search_document_ids(SearchIndex::LabelForForms, query)
            .await?
            .into_iter()
            .map(|id| FormLabelSearchHit {
                label_id: id.into(),
            })
            .collect_vec())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "search", db.collection.name = "label_for_form_answers"))]
    async fn search_labels_for_answers(
        &self,
        query: &str,
    ) -> Result<Vec<AnswerLabelSearchHit>, InfraError> {
        Ok(self
            .search_document_ids(SearchIndex::LabelForFormAnswers, query)
            .await?
            .into_iter()
            .map(|id| AnswerLabelSearchHit {
                label_id: id.into(),
            })
            .collect_vec())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "search"))]
    async fn search_answers(
        &self,
        query: &str,
        form_id: Option<FormId>,
        status: Option<AnswerStatus>,
    ) -> Result<Vec<AnswerSearchHit>, InfraError> {
        let filter = answer_filter(form_id, status);
        let title_index = self.meilisearch_client.index(SearchIndex::Answers.as_str());
        let content_index = self
            .meilisearch_client
            .index(SearchIndex::RealAnswers.as_str());
        let title_search = configured_search(&title_index, query, &ID_ONLY, filter.as_deref());
        let content_search =
            configured_search(&content_index, query, &ANSWER_ID_ONLY, filter.as_deref());
        let (title_results, content_results) = try_join!(
            title_search.execute::<IndexedDocumentId>(),
            content_search.execute::<AnswerContentHit>(),
        )?;

        Ok(merge_answer_hits(
            title_results
                .hits
                .into_iter()
                .map(|hit| AnswerId::from(hit.result.id)),
            content_results
                .hits
                .into_iter()
                .map(|hit| hit.result.answer_id),
        ))
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "search", db.collection.name = "form_answer_comments"))]
    async fn search_comments(&self, query: &str) -> Result<Vec<CommentSearchHit>, InfraError> {
        let index = self
            .meilisearch_client
            .index(SearchIndex::FormAnswerComments.as_str());
        Ok(configured_search(&index, query, &ID_AND_ANSWER_ID, None)
            .execute::<CommentHit>()
            .await?
            .hits
            .into_iter()
            .map(|hit| CommentSearchHit {
                comment_id: hit.result.id,
                answer_id: hit.result.answer_id,
            })
            .collect_vec())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "sync_search_engine"))]
    async fn sync_search_engine(
        &self,
        data: &[SearchableFieldsWithOperation],
    ) -> Result<(), InfraError> {
        let answer_ids_to_fetch = data
            .iter()
            .filter_map(|(fields, operation)| match (fields, operation) {
                (SearchableFields::AnswerTitle(answer), Operation::Create | Operation::Update) => {
                    Some(answer.id)
                }
                (SearchableFields::RealAnswers(content), Operation::Create | Operation::Update) => {
                    Some(content.answer_id)
                }
                _ => None,
            })
            .unique()
            .collect_vec();
        let answer_records = self.get_answers_by_answer_ids(answer_ids_to_fetch).await?;
        let metadata_by_answer_id = answer_metadata_from_records(&answer_records)?;
        let reprojected_documents = answer_records
            .into_iter()
            .map(answer_documents_from_record)
            .try_fold(Vec::new(), |mut documents, result| {
                documents.extend(result?);
                Ok::<_, InfraError>(documents)
            })?;
        let content_documents = answer_content_documents(data, &metadata_by_answer_id);
        let mut orphaned_contents = 0usize;

        let reprojected_writes =
            reprojected_documents
                .iter()
                .filter_map(|(fields, _)| match fields {
                    SearchableFields::AnswerTitle(answer) => Some(
                        serde_json::to_value(answer)
                            .map(|document| {
                                SearchDocumentWrite::Upsert(SearchIndex::Answers, document)
                            })
                            .map_err(InfraError::from),
                    ),
                    SearchableFields::RealAnswers(content) => Some(
                        metadata_by_answer_id
                            .get(&content.answer_id)
                            .map(|(form_id, _)| *form_id)
                            .ok_or_else(|| InfraError::Unexpected {
                                cause: format!(
                                    "form id for answer {} was not found while reprojecting",
                                    content.answer_id
                                ),
                            })
                            .and_then(|form_id| {
                                serde_json::to_value(AnswerContentSearchDocument {
                                    id: content.id,
                                    form_id,
                                    answer_id: content.answer_id,
                                    question_id: content.question_id,
                                    answer: content.answer.clone(),
                                    status: content.status,
                                })
                                .map_err(InfraError::from)
                            })
                            .map(|document| {
                                SearchDocumentWrite::Upsert(SearchIndex::RealAnswers, document)
                            }),
                    ),
                    _ => None,
                });

        let event_writes = data
            .iter()
            .filter(|(searchable_fields, operation)| {
                !matches!(
                    (searchable_fields, operation),
                    (
                        SearchableFields::AnswerTitle(_),
                        Operation::Create | Operation::Update
                    )
                )
            })
            .map(
                |(searchable_fields, operation)| match (searchable_fields, operation) {
                    (
                        SearchableFields::RealAnswers(content),
                        Operation::Create | Operation::Update,
                    ) => match content_documents.get(&content.id) {
                        Some(document) => serde_json::to_value(document)
                            .map(|document| {
                                SearchDocumentWrite::Upsert(SearchIndex::RealAnswers, document)
                            })
                            .map_err(InfraError::from),
                        // 回答が消えているので、投入ではなく検索エンジンから消す
                        None => {
                            orphaned_contents += 1;
                            Ok(SearchDocumentWrite::Delete(
                                SearchIndex::RealAnswers,
                                searchable_fields.document_id().to_string(),
                            ))
                        }
                    },
                    (_, Operation::Create | Operation::Update) => {
                        upsert_document(searchable_fields).map(|document| {
                            SearchDocumentWrite::Upsert(searchable_fields.index(), document)
                        })
                    }
                    (_, Operation::Delete) => Ok(SearchDocumentWrite::Delete(
                        searchable_fields.index(),
                        searchable_fields.document_id().to_string(),
                    )),
                },
            );

        let writes = reprojected_writes
            .chain(event_writes)
            .collect::<Result<Vec<_>, InfraError>>()?;
        if orphaned_contents > 0 {
            tracing::warn!(
                count = orphaned_contents,
                "answers of some answer contents no longer exist; removing their search documents instead of indexing them",
            );
        }

        execute_search_document_requests(&self.meilisearch_client, search_document_requests(writes))
            .await
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "fetch_indexed_document_ids"))]
    async fn fetch_indexed_document_ids(
        &self,
        index: SearchIndex,
    ) -> Result<HashSet<Uuid>, InfraError> {
        let meilisearch_index = self.meilisearch_client.index(index.as_str());
        let meilisearch_index = &meilisearch_index;

        // `None` は読み終えたことを表す。1 ページ取るごとに次のオフセットを決めて畳み込む
        stream::try_unfold(Some(0), |offset| async move {
            let Some(offset) = offset else {
                return Ok(None);
            };

            let mut query = DocumentsQuery::new(meilisearch_index);
            query
                .with_fields(["id"])
                .with_offset(offset)
                .with_limit(INDEXED_ID_FETCH_LIMIT);

            let documents = match query.execute::<IndexedDocumentId>().await {
                Ok(documents) => documents.results,
                // インデックスごと消えているときは投影済みのドキュメントも存在しないため、
                // 空集合として扱って再同期に全件を投入させる (投入時にインデックスが作り直される)
                Err(error) if is_index_not_found(&error) => return Ok(None),
                Err(error) => return Err(error.into()),
            };
            let next_offset =
                (documents.len() == INDEXED_ID_FETCH_LIMIT).then(|| offset + documents.len());

            Ok::<_, InfraError>(Some((documents, next_offset)))
        })
        .try_fold(HashSet::new(), async |mut ids, documents: Vec<_>| {
            ids.extend(documents.into_iter().map(|document| document.id));

            Ok(ids)
        })
        .await
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "delete_search_documents"))]
    async fn delete_search_documents(
        &self,
        index: SearchIndex,
        ids: Vec<Uuid>,
    ) -> Result<(), InfraError> {
        let writes = ids
            .into_iter()
            .map(|id| SearchDocumentWrite::Delete(index, id.to_string()));

        execute_search_document_requests(&self.meilisearch_client, search_document_requests(writes))
            .await
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "search_engine_stats"))]
    async fn search_engine_stats(&self) -> Result<NumberOfRecordsPerAggregate, InfraError> {
        let stats = self.meilisearch_client.get_stats().await?;
        // まだ作られていないインデックスはドキュメント 0 件として扱う
        let records_of = |index: SearchIndex| {
            let documents = stats
                .indexes
                .get(index.as_str())
                .map_or(0, |index_stats| index_stats.number_of_documents);
            u32::try_from(documents)
                .map(NumberOfRecords)
                .map_err(|_| InfraError::Unexpected {
                    cause: format!(
                        "search index {} holds too many documents to count: {documents}",
                        index.as_str()
                    ),
                })
        };

        Ok(NumberOfRecordsPerAggregate {
            form_meta_data: records_of(SearchIndex::FormMetaData)?,
            answers: records_of(SearchIndex::Answers)?,
            real_answers: records_of(SearchIndex::RealAnswers)?,
            form_answer_comments: records_of(SearchIndex::FormAnswerComments)?,
            label_for_form_answers: records_of(SearchIndex::LabelForFormAnswers)?,
            label_for_forms: records_of(SearchIndex::LabelForForms)?,
            users: records_of(SearchIndex::Users)?,
        })
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.operation.name = "initialize_search_engine"))]
    async fn initialize_search_engine(&self) -> Result<bool, InfraError> {
        try_join_all(
            SearchIndex::ALL
                .into_iter()
                .map(|index| self.ensure_search_index(index)),
        )
        .await?;
        try_join_all(
            SearchIndex::ALL
                .into_iter()
                .map(|index| self.ensure_search_index_settings(index)),
        )
        .await?;

        answer_documents_need_reprojection(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SYNC_DOCUMENT_CHUNK_SIZE, SearchDocumentRequest, SearchDocumentWrite,
        answer_content_documents, answer_documents_from_entry, answer_filter, index_settings,
        merge_answer_hits, search_document_requests, settings_match,
    };
    use domain::search::models::SearchIndex;
    use domain::{
        form::{
            answer::{AnswerAuthor, AnswerEntry, AnswerId, AnswerStatus, AnswerTitle},
            models::FormId,
        },
        search::models::{Operation, RealAnswers, SearchableFields},
    };
    use itertools::Itertools;
    use meilisearch_sdk::settings::{FilterableAttribute, Settings};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn answer_id(value: u128) -> AnswerId {
        Uuid::from_u128(value).into()
    }

    #[test]
    fn search_document_requests_group_documents_per_index_and_split_them_into_chunks() {
        let writes = (0..=SYNC_DOCUMENT_CHUNK_SIZE)
            .map(|number| {
                SearchDocumentWrite::Upsert(SearchIndex::Users, serde_json::json!({ "id": number }))
            })
            .chain([
                SearchDocumentWrite::Upsert(
                    SearchIndex::Answers,
                    serde_json::json!({ "id": "answer" }),
                ),
                SearchDocumentWrite::Delete(SearchIndex::Users, "orphan".to_string()),
            ]);

        let mut requests = search_document_requests(writes)
            .iter()
            .map(|request| match request {
                SearchDocumentRequest::Upsert { index, documents } => {
                    (index.as_str(), "upsert", documents.len())
                }
                SearchDocumentRequest::Delete { index, ids } => {
                    (index.as_str(), "delete", ids.len())
                }
            })
            .collect_vec();
        requests.sort_unstable();

        assert_eq!(
            requests,
            vec![
                ("answers", "upsert", 1),
                ("users", "delete", 1),
                ("users", "upsert", 1),
                ("users", "upsert", SYNC_DOCUMENT_CHUNK_SIZE),
            ]
        );
    }

    #[test]
    fn answer_filter_combines_form_id_and_status() {
        let form_id = FormId::from(Uuid::from_u128(1));

        assert_eq!(
            answer_filter(Some(form_id), Some(AnswerStatus::IN_PROGRESS)),
            Some(format!(
                "form_id = \"{form_id}\" AND status = \"IN_PROGRESS\""
            ))
        );
        assert_eq!(answer_filter(None, None), None);
    }

    #[test]
    fn index_settings_restrict_searchable_attributes_and_declare_japanese() {
        let settings = index_settings(SearchIndex::RealAnswers);

        assert_eq!(
            settings.searchable_attributes,
            Some(vec!["answer".to_string()])
        );
        assert_eq!(
            settings.filterable_attributes,
            Some(vec![
                FilterableAttribute::Attribute("form_id".to_string()),
                FilterableAttribute::Attribute("status".to_string()),
            ])
        );
        let localized = settings.localized_attributes.unwrap();
        assert_eq!(localized.len(), 1);
        assert_eq!(localized[0].locales, vec!["jpn".to_string()]);
        assert_eq!(localized[0].attribute_patterns, vec!["*".to_string()]);
        assert!(
            index_settings(SearchIndex::Users)
                .filterable_attributes
                .is_none()
        );
    }

    #[test]
    fn settings_match_ignores_unspecified_settings_and_detects_differences() {
        let desired = index_settings(SearchIndex::Users);
        let applied = Settings {
            // 検索エンジンは指定していない設定も返す
            displayed_attributes: Some(vec!["*".to_string()]),
            filterable_attributes: Some(Vec::new()),
            ..desired.clone()
        };
        assert!(settings_match(&applied, &desired));

        let default_settings = Settings {
            searchable_attributes: Some(vec!["*".to_string()]),
            localized_attributes: None,
            ..Settings::new()
        };
        assert!(!settings_match(&default_settings, &desired));

        let desired_answers = index_settings(SearchIndex::Answers);
        let without_filterable = Settings {
            filterable_attributes: Some(Vec::new()),
            ..desired_answers.clone()
        };
        assert!(!settings_match(&without_filterable, &desired_answers));
    }

    #[test]
    fn answer_contents_of_missing_answers_are_left_out_of_the_projection() {
        let existing_answer = answer_id(1);
        let missing_answer = answer_id(2);
        let existing_content = Uuid::from_u128(3).into();
        let orphaned_content = Uuid::from_u128(4).into();
        let content = |id, answer_id| {
            (
                SearchableFields::RealAnswers(RealAnswers {
                    id,
                    answer_id,
                    question_id: Uuid::from_u128(5).into(),
                    answer: "content".to_string(),
                    status: AnswerStatus::UNADDRESSED,
                }),
                Operation::Create,
            )
        };
        let data = [
            content(existing_content, existing_answer),
            content(orphaned_content, missing_answer),
        ];

        let documents = answer_content_documents(
            &data,
            &HashMap::from([(
                existing_answer,
                (FormId::from(Uuid::from_u128(6)), AnswerStatus::UNADDRESSED),
            )]),
        );

        assert!(documents.contains_key(&existing_content));
        assert!(!documents.contains_key(&orphaned_content));
    }

    #[test]
    fn answer_content_search_document_contains_form_id() {
        let form_id = FormId::from(Uuid::from_u128(1));
        let content_id = Uuid::from_u128(2).into();
        let answer_id = answer_id(3);
        let data = [(
            SearchableFields::RealAnswers(RealAnswers {
                id: content_id,
                answer_id,
                question_id: Uuid::from_u128(4).into(),
                answer: "content".to_string(),
                status: AnswerStatus::UNADDRESSED,
            }),
            Operation::Update,
        )];
        let document = answer_content_documents(
            &data,
            &HashMap::from([(answer_id, (form_id, AnswerStatus::UNADDRESSED))]),
        )
        .remove(&content_id)
        .unwrap();

        assert_eq!(
            serde_json::to_value(document).unwrap()["form_id"],
            form_id.to_string()
        );
    }

    #[test]
    fn answer_search_hits_include_title_and_content_matches_with_title_first() {
        let title_match = answer_id(1);
        let content_match = answer_id(2);

        let hits = merge_answer_hits([title_match], [content_match]);

        assert_eq!(
            hits.into_iter()
                .map(|hit| hit.answer_id)
                .collect::<Vec<_>>(),
            vec![title_match, content_match]
        );
    }

    #[test]
    fn answer_search_hits_are_unique_when_title_and_content_both_match() {
        let answer_id = answer_id(1);

        let hits = merge_answer_hits([answer_id], [answer_id, answer_id]);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].answer_id, answer_id);
    }

    #[test]
    fn status_reprojection_updates_title_and_content_documents_together() {
        let answer_id = answer_id(1);
        let content_id = Uuid::from_u128(2).into();
        let question_id = Uuid::from_u128(3).into();
        let entry = unsafe {
            AnswerEntry::from_raw_parts_with_status_and_redmine_reference(
                answer_id,
                Uuid::from_u128(4).into(),
                Uuid::from_u128(6).into(),
                AnswerAuthor::AuthenticatedUser(Uuid::from_u128(5).into()),
                chrono::Utc::now(),
                AnswerTitle::new(None),
                domain::form::answer::AnswerPublication::PUBLIC,
                AnswerStatus::COMPLETED,
                vec![
                    domain::form::answer::AnsweredQuestionContent::from_raw_parts(
                        content_id,
                        question_id,
                        "本文".to_string(),
                        "質問".to_string().try_into().unwrap(),
                    ),
                ],
                None,
            )
        };

        let documents = answer_documents_from_entry(&entry);
        assert!(matches!(
            &documents[0].0,
            SearchableFields::AnswerTitle(document)
                if document.status == AnswerStatus::COMPLETED
        ));
        assert!(matches!(
            &documents[1].0,
            SearchableFields::RealAnswers(document)
                if document.status == AnswerStatus::COMPLETED
        ));
    }
}
