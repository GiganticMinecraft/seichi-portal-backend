use crate::database::config::{MEILISEARCH, MeiliSearch};
use crate::database::meilisearch_schemas::MeilisearchStatsSchema;
use crate::database::{
    components::{FormAnswerDatabase, SearchDatabase},
    connection::ConnectionPool,
};
use crate::outgoing::http::HTTP_CLIENT;
use crate::records::FormAnswerRecord;
use async_trait::async_trait;
use domain::{
    account::models::UserId,
    form::{
        answer::{AnswerEntry, AnswerId, AnswerStatus, FormAnswerContentId},
        models::FormId,
        question::QuestionId,
    },
    search::models::{
        AnswerLabelSearchHit, AnswerSearchHit, AnswerTitleSearchDocument, CommentSearchHit,
        FormAnswerComments, FormLabelSearchHit, FormMetaData, FormSearchHit, LabelForFormAnswers,
        LabelForForms, NumberOfRecordsPerAggregate, Operation, SearchIndex, SearchableFields,
        SearchableFieldsWithOperation, UserSearchHit, Users,
    },
};
use errors::infra::InfraError;
use futures::{StreamExt, TryStreamExt, future::try_join_all, stream, try_join};
use itertools::{Either, Itertools};
use meilisearch_sdk::{
    client::Client,
    documents::DocumentsQuery,
    errors::{Error as MeilisearchError, ErrorCode},
    search::Selectors,
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
    async fn execute(self, client: &Client) -> Result<(), InfraError> {
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
    client: &Client,
    requests: Vec<SearchDocumentRequest>,
) -> Result<(), InfraError> {
    stream::iter(requests)
        .map(|request| request.execute(client))
        .buffer_unordered(SYNC_REQUEST_CONCURRENCY)
        .try_collect()
        .await
}

fn upsert_document(
    fields: &SearchableFields,
    content_documents: &HashMap<FormAnswerContentId, AnswerContentSearchDocument>,
) -> Result<serde_json::Value, InfraError> {
    Ok(match fields {
        SearchableFields::FormMetaData(data) => serde_json::to_value(data)?,
        SearchableFields::AnswerTitle(_) => {
            unreachable!("answer title updates are handled by database reprojection")
        }
        SearchableFields::RealAnswers(answers) => {
            serde_json::to_value(&content_documents[&answers.id])?
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

fn answer_content_documents(
    data: &[SearchableFieldsWithOperation],
    metadata_by_answer_id: &HashMap<AnswerId, (FormId, AnswerStatus)>,
) -> Result<HashMap<FormAnswerContentId, AnswerContentSearchDocument>, InfraError> {
    data.iter()
        .filter_map(|(fields, operation)| match (fields, operation) {
            (SearchableFields::RealAnswers(content), Operation::Create | Operation::Update) => {
                Some(content)
            }
            _ => None,
        })
        .map(|content| {
            let (form_id, status) = metadata_by_answer_id
                .get(&content.answer_id)
                .copied()
                .ok_or_else(|| InfraError::Unexpected {
                    cause: format!(
                        "form id for answer {} was not found while updating its search document",
                        content.answer_id
                    ),
                })?;

            Ok((
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

fn add_meilisearch_stats_auth(
    request: reqwest_middleware::RequestBuilder,
    api_key: Option<&str>,
) -> reqwest_middleware::RequestBuilder {
    match api_key {
        Some(api_key) => request.bearer_auth(api_key),
        None => request,
    }
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

#[async_trait]
impl SearchDatabase for ConnectionPool {
    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.collection.name = "users"))]
    async fn search_users(&self, query: &str) -> Result<Vec<UserSearchHit>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("users")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<Users>()
            .await?
            .hits
            .into_iter()
            .map(|hit| UserSearchHit {
                user_id: UserId::from(hit.result.id),
            })
            .collect_vec())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.collection.name = "form_meta_data"))]
    async fn search_forms(&self, query: &str) -> Result<Vec<FormSearchHit>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("form_meta_data")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<FormMetaData>()
            .await?
            .hits
            .into_iter()
            .map(|hit| FormSearchHit {
                form_id: hit.result.id,
            })
            .collect_vec())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.collection.name = "label_for_forms"))]
    async fn search_labels_for_forms(
        &self,
        query: &str,
    ) -> Result<Vec<FormLabelSearchHit>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("label_for_forms")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<LabelForForms>()
            .await?
            .hits
            .into_iter()
            .map(|hit| FormLabelSearchHit {
                label_id: hit.result.id,
            })
            .collect_vec())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.collection.name = "label_for_form_answers"))]
    async fn search_labels_for_answers(
        &self,
        query: &str,
    ) -> Result<Vec<AnswerLabelSearchHit>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("label_for_form_answers")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<LabelForFormAnswers>()
            .await?
            .hits
            .into_iter()
            .map(|hit| AnswerLabelSearchHit {
                label_id: hit.result.id,
            })
            .collect_vec())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch"))]
    async fn search_answers(
        &self,
        query: &str,
        form_id: Option<FormId>,
        status: Option<AnswerStatus>,
    ) -> Result<Vec<AnswerSearchHit>, InfraError> {
        let filter = answer_filter(form_id, status);
        let title_search = async {
            let index = self.meilisearch_client.index("answers");
            let mut search = index.search();
            search
                .with_query(query)
                .with_attributes_to_highlight(Selectors::All);
            if let Some(filter) = filter.as_deref() {
                search.with_filter(filter);
            }
            search.execute::<AnswerTitleSearchDocument>().await
        };
        let content_search = async {
            let index = self.meilisearch_client.index("real_answers");
            let mut search = index.search();
            search
                .with_query(query)
                .with_attributes_to_highlight(Selectors::All);
            if let Some(filter) = filter.as_deref() {
                search.with_filter(filter);
            }
            search.execute::<AnswerContentSearchDocument>().await
        };
        let (title_results, content_results) = try_join!(title_search, content_search)?;

        Ok(merge_answer_hits(
            title_results.hits.into_iter().map(|hit| hit.result.id),
            content_results
                .hits
                .into_iter()
                .map(|hit| hit.result.answer_id),
        ))
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch", db.collection.name = "form_answer_comments"))]
    async fn search_comments(&self, query: &str) -> Result<Vec<CommentSearchHit>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("form_answer_comments")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<FormAnswerComments>()
            .await?
            .hits
            .into_iter()
            .map(|hit| CommentSearchHit {
                comment_id: hit.result.id,
                answer_id: hit.result.answer_id,
            })
            .collect_vec())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch"))]
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
        let content_documents = answer_content_documents(data, &metadata_by_answer_id)?;

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
            .map(|(searchable_fields, operation)| match operation {
                Operation::Create | Operation::Update => {
                    upsert_document(searchable_fields, &content_documents).map(|document| {
                        SearchDocumentWrite::Upsert(searchable_fields.index(), document)
                    })
                }
                Operation::Delete => Ok(SearchDocumentWrite::Delete(
                    searchable_fields.index(),
                    searchable_fields.document_id().to_string(),
                )),
            });

        let writes = reprojected_writes
            .chain(event_writes)
            .collect::<Result<Vec<_>, InfraError>>()?;

        execute_search_document_requests(&self.meilisearch_client, search_document_requests(writes))
            .await
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch"))]
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

            let documents = query.execute::<IndexedDocumentId>().await?.results;
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

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch"))]
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

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch"))]
    async fn search_engine_stats(&self) -> Result<NumberOfRecordsPerAggregate, InfraError> {
        let MeiliSearch { host, api_key } = &*MEILISEARCH;

        let response = add_meilisearch_stats_auth(
            HTTP_CLIENT.get(format!("{}/stats", host)),
            api_key.as_deref(),
        )
        .send()
        .await?;

        Ok(
            serde_json::from_str::<MeilisearchStatsSchema>(response.text().await?.as_str())?
                .indexes
                .into(),
        )
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "meilisearch"))]
    async fn initialize_search_engine(&self) -> Result<bool, InfraError> {
        let futures = SearchIndex::ALL
            .into_iter()
            .map(async |index| {
                let task = self
                    .meilisearch_client
                    .create_index(index.as_str(), Some("id"))
                    .await?
                    .wait_for_completion(&self.meilisearch_client, None, None)
                    .await?;

                ensure_meilisearch_task_succeeded(task, true)
            })
            .collect_vec();

        try_join_all(futures).await?;

        let settings_futures = ANSWER_SEARCH_INDEXES.into_iter().map(async |index| {
            let task = self
                .meilisearch_client
                .index(index.as_str())
                .set_filterable_attributes(["form_id", "status"])
                .await?
                .wait_for_completion(&self.meilisearch_client, None, None)
                .await?;

            ensure_meilisearch_task_succeeded(task, false)
        });
        try_join_all(settings_futures).await?;

        answer_documents_need_reprojection(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SYNC_DOCUMENT_CHUNK_SIZE, SearchDocumentRequest, SearchDocumentWrite,
        add_meilisearch_stats_auth, answer_content_documents, answer_documents_from_entry,
        answer_filter, merge_answer_hits, search_document_requests,
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
    fn meilisearch_stats_uses_bearer_authorization() {
        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
        let request =
            add_meilisearch_stats_auth(client.get("http://localhost/stats"), Some("sentinel"))
                .build()
                .unwrap();

        assert!(
            request
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .is_some_and(|value| value.as_bytes() == b"Bearer sentinel")
        );
        assert!(request.headers().get("X-Meili-API-Key").is_none());
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
        .unwrap()
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
                AnswerAuthor::AuthenticatedUser(Uuid::from_u128(5).into()),
                chrono::Utc::now(),
                AnswerTitle::new(None),
                domain::form::answer::AnswerPublication::PUBLIC,
                AnswerStatus::COMPLETED,
                vec![domain::form::answer::FormAnswerContent {
                    id: content_id,
                    question_id,
                    answer: "本文".to_string(),
                }],
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
