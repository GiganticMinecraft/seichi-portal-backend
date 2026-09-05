use crate::{
    models::{
        ActiveFormWithLabels, AnswerDetails, CommentAuthor, CommentWithAuthor, CrossSearchComment,
        CrossSearchOutput, PublishedAnswerAuthor, PublishedAnswerEntry,
        answer_response_visibility_for,
    },
    user_reference_resolver::resolve_user_references,
};
use domain::repository::form::answer_entry_repository::{AnswerEntryRepository, AnswerListFilter};
use domain::repository::form::answer_label_repository::AnswerLabelRepository;
use domain::repository::form::comment_thread_repository::CommentThreadRepository;
use domain::repository::form::form_label_repository::FormLabelRepository;
use domain::repository::user_repository::UserRepository;
use domain::{
    account::models::{AccountUser, UserPagePosition},
    auth::Actor,
    form::{
        answer::{
            AnswerAuthor, AnswerAuthorDisclosure, AnswerEntry, AnswerId, AnswerPagePosition,
            AnswerStatus,
        },
        comment::Comment,
        comment_thread::CommentThread,
        models::{ActiveForm, FormId},
    },
    pagination::{PageLimit, PageRequest},
    repository::{
        form::active_form_repository::ActiveFormRepository, search_repository::SearchRepository,
    },
    search::models::{
        AnswerSearchHit, AnswerTitleSearchDocument, FormAnswerComments, FormMetaData,
        LabelForFormAnswers, LabelForForms, NumberOfRecords, NumberOfRecordsPerAggregate,
        Operation, RealAnswers, SearchIndex, SearchableFields, SearchableFieldsWithOperation,
        UserSearchHit, Users,
    },
    types::authorization_guard::{
        Allowed, AuthorizationGuard, AuthorizationGuardDefinitions, Read,
    },
};
use errors::{Error, domain::DomainError};
use futures::{StreamExt, TryStreamExt, stream, try_join};
use std::{
    collections::{HashMap, HashSet},
    future::ready,
    iter::once,
    time::Duration,
};
use tokio::sync::{mpsc::Receiver, watch};
use tokio::time;
use tracing::Instrument;
use uuid::Uuid;

const SEARCH_DETAIL_FETCH_CONCURRENCY: usize = 10;
#[cfg(not(test))]
const SEARCH_ENGINE_SYNC_RETRY_DELAY: Duration = Duration::from_secs(10);
#[cfg(test)]
const SEARCH_ENGINE_SYNC_RETRY_DELAY: Duration = Duration::ZERO;
const OUT_OF_SYNC_CHECK_INTERVAL: Duration = Duration::from_secs(60);
/// 1 回の再同期で走査するドキュメント数の上限。
///
/// 全件を一度に処理すると、検索エンジンへの投入とデータベースの読み出しが同時に跳ね上がるため、
/// ここまで走査したら打ち切り、続きは次の [`OUT_OF_SYNC_CHECK_INTERVAL`] で再開する。
const RESYNC_DOCUMENT_SCAN_BUDGET: usize = 2_000;

fn resync_page_limit() -> PageLimit {
    PageLimit::default_limit()
}

fn next_page_request<Position>(next: Option<Position>) -> Option<PageRequest<Position>> {
    next.map(|position| PageRequest::after(position, resync_page_limit()))
}

/// 1 ページ分の検索ドキュメントと、続きを読むための次のページ要求。
type DocumentPage<Position> = (
    Vec<SearchableFieldsWithOperation>,
    Option<PageRequest<Position>>,
);

/// システムとして読み取った集約を、検索エンジンへ投入するドキュメントへ投影する。
fn system_readable_documents<T: AuthorizationGuardDefinitions>(
    guards: Vec<AuthorizationGuard<T, Read>>,
    into_searchable_fields: impl Fn(&T) -> SearchableFields,
) -> Result<Vec<SearchableFieldsWithOperation>, Error> {
    guards
        .into_iter()
        .map(|guard| {
            let value = guard.try_read(Actor::System)?.into_inner();

            Ok((into_searchable_fields(&value), Operation::Update))
        })
        .collect()
}

/// 再同期 1 巡分の進捗。
///
/// 1 回の [`SearchUseCase::resync_search_engine_step`] では走査しきれないため、
/// 走査位置と検索エンジン側の ID 集合を呼び出しをまたいで持ち回る。
struct ResyncPass {
    /// 巡回開始時点で検索エンジンに存在した、乖離しているインデックスのドキュメント ID。
    ///
    /// 走査で見つかった ID は取り除いていくので、巡回し終えたときに残っている ID が
    /// リポジトリ側に対応する行を持たないドキュメントになる。
    indexed_ids: HashMap<SearchIndex, HashSet<Uuid>>,
    /// 巡回開始時点のリポジトリ側の件数。走査漏れの検出に使う。
    repository_records: NumberOfRecordsPerAggregate,
    scanned: HashMap<SearchIndex, usize>,
    small_aggregates_done: bool,
    /// 次に読むページ。`None` は走査済みを表す。
    users: Option<PageRequest<UserPagePosition>>,
    answers: Option<PageRequest<AnswerPagePosition>>,
}

impl ResyncPass {
    fn new(
        indexed_ids: HashMap<SearchIndex, HashSet<Uuid>>,
        repository_records: NumberOfRecordsPerAggregate,
    ) -> Self {
        Self {
            indexed_ids,
            repository_records,
            scanned: HashMap::new(),
            small_aggregates_done: false,
            users: Some(PageRequest::first(resync_page_limit())),
            answers: Some(PageRequest::first(resync_page_limit())),
        }
    }

    fn is_complete(&self) -> bool {
        self.small_aggregates_done && self.users.is_none() && self.answers.is_none()
    }

    /// `index` が今回の巡回の対象かどうか。
    fn is_scanning(&self, index: SearchIndex) -> bool {
        self.indexed_ids.contains_key(&index)
    }

    /// 走査したドキュメントのうち、検索エンジンに存在しないものだけを返す。
    ///
    /// 存在が確認できたドキュメントは [`Self::indexed_ids`] から取り除く。
    fn take_missing(
        &mut self,
        documents: Vec<SearchableFieldsWithOperation>,
    ) -> Vec<SearchableFieldsWithOperation> {
        documents
            .iter()
            .map(|(fields, _)| fields.index())
            .filter(|index| self.indexed_ids.contains_key(index))
            .for_each(|index| *self.scanned.entry(index).or_default() += 1);

        documents
            .into_iter()
            .filter(|(fields, _)| {
                // 乖離していないインデックスは `indexed_ids` を持たないので、投入対象にならない
                self.indexed_ids
                    .get_mut(&fields.index())
                    .is_some_and(|indexed_ids| !indexed_ids.remove(&fields.document_id()))
            })
            .collect()
    }
}

pub struct SearchUseCase<
    'a,
    SearchRepo: SearchRepository,
    FormRepo: ActiveFormRepository,
    FormAnswerLabelRepo: AnswerLabelRepository,
    FormLabelRepo: FormLabelRepository,
    UserRepo: UserRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    CommentThreadRepo: CommentThreadRepository,
> {
    pub search_repository: &'a SearchRepo,
    pub active_form_repository: &'a FormRepo,
    pub form_answer_label_repository: &'a FormAnswerLabelRepo,
    pub form_label_repository: &'a FormLabelRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub comment_thread_repository: &'a CommentThreadRepo,
}

impl<
    R1: SearchRepository,
    R2: ActiveFormRepository,
    R3: AnswerLabelRepository,
    R4: FormLabelRepository,
    R5: UserRepository,
    R6: AnswerEntryRepository,
    R7: CommentThreadRepository,
> SearchUseCase<'_, R1, R2, R3, R4, R5, R6, R7>
{
    async fn list_all_form_guards(
        &self,
    ) -> Result<Vec<AuthorizationGuard<ActiveForm, Read>>, Error> {
        self.active_form_repository.list_all().await
    }

    async fn visible_answer_entries_by_id(
        &self,
        actor: &Actor,
        answer_ids: Vec<AnswerId>,
    ) -> Result<HashMap<AnswerId, Allowed<AnswerEntry, Read>>, Error> {
        if answer_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let readable_forms = self
            .list_all_form_guards()
            .await?
            .into_iter()
            .filter_map(|form| form.try_read(actor.clone()).ok())
            .collect::<Vec<_>>();

        Ok(self
            .answer_entry_repository
            .find_by_ids(&readable_forms, answer_ids)
            .await?
            .into_iter()
            .map(|answer| (*answer.id(), answer))
            .collect())
    }

    async fn comment_thread_for_answer(
        &self,
        actor: &Actor,
        answer: Allowed<AnswerEntry, Read>,
    ) -> Result<Option<Allowed<CommentThread, Read>>, Error> {
        let Some(form) = self.active_form_repository.get(*answer.form_id()).await? else {
            return Ok(None);
        };
        let Ok(form) = form.try_read(actor.clone()) else {
            return Ok(None);
        };

        self.comment_thread_repository
            .get_with_comments_for_answer(&form, answer.into_inner())
            .await
            .map(Some)
            .or_else(|error| {
                if matches!(
                    error,
                    Error::Domain {
                        source: DomainError::Forbidden
                    }
                ) {
                    Ok(None)
                } else {
                    Err(error)
                }
            })
    }

    async fn comment_search_documents(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        answers: &[Allowed<AnswerEntry, Read>],
    ) -> Result<Vec<SearchableFieldsWithOperation>, Error> {
        stream::iter(answers.iter().cloned())
            .then(|answer| async move {
                let form = forms
                    .iter()
                    .find(|form| form.id() == answer.form_id())
                    .ok_or(DomainError::NotFound)?;
                let thread = self
                    .comment_thread_repository
                    .get_with_comments_for_answer(form, answer.into_inner())
                    .await?;
                Ok::<_, Error>(
                    thread
                        .comments()
                        .iter()
                        .map(|comment| {
                            (
                                SearchableFields::FormAnswerComments(FormAnswerComments {
                                    id: comment.comment_id().to_owned(),
                                    answer_id: comment.answer_id().to_owned(),
                                    content: comment.content().to_owned().into_inner().into_inner(),
                                }),
                                Operation::Update,
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .try_collect::<Vec<_>>()
            .await
            .map(|documents| documents.into_iter().flatten().collect())
    }

    async fn visible_users(
        &self,
        actor: &Actor,
        users: Vec<UserSearchHit>,
    ) -> Result<Vec<AccountUser>, Error> {
        let user_ids = users
            .iter()
            .map(|user| user.user_id.into_inner())
            .collect::<Vec<_>>();

        let visible_users_by_id = self
            .user_repository
            .find_by_ids(user_ids)
            .await?
            .into_iter()
            .filter_map(|guard| {
                guard.try_read(actor.clone()).ok().map(|user| {
                    let user = user.into_inner();
                    (*user.id(), user)
                })
            })
            .collect::<HashMap<_, _>>();

        Ok(users
            .into_iter()
            .filter_map(|user| visible_users_by_id.get(&user.user_id).cloned())
            .collect())
    }

    async fn visible_form_with_labels(
        &self,
        actor: &Actor,
        form_id: FormId,
    ) -> Result<Option<ActiveFormWithLabels>, Error> {
        let Some(form) = self.active_form_repository.get(form_id).await? else {
            return Ok(None);
        };
        let Ok(form) = form.try_read(actor.clone()) else {
            return Ok(None);
        };
        let labels = self
            .form_label_repository
            .fetch_labels_by_form_id(form_id)
            .await?
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(ActiveFormWithLabels {
            form: form.into_inner(),
            labels,
        }))
    }

    async fn answer_details(
        &self,
        account_user: &AccountUser,
        actor: &Actor,
        answer: Allowed<AnswerEntry, Read>,
    ) -> Result<Option<AnswerDetails>, Error> {
        let answer_id = *answer.id();
        let form_id = *answer.form_id();
        let Some(form) = self.active_form_repository.get(form_id).await? else {
            return Ok(None);
        };
        let Ok(form) = form.try_read(actor.clone()) else {
            return Ok(None);
        };
        let labels = self
            .form_answer_label_repository
            .get_labels_for_answers_by_answer_id(answer_id)
            .await?
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let answer_response_visibility = answer_response_visibility_for(
            *form.answer_settings().answer_response_visibility(),
            answer.value(),
            account_user,
        );
        let author = match form.answer_settings().author_disclosure_for(actor) {
            AnswerAuthorDisclosure::Anonymous => PublishedAnswerAuthor::Anonymous,
            AnswerAuthorDisclosure::Disclosed => {
                let users = resolve_user_references(
                    self.user_repository,
                    account_user,
                    answer
                        .author()
                        .authenticated_user_id()
                        .into_iter()
                        .collect(),
                )
                .await?;
                match answer.author() {
                    AnswerAuthor::AuthenticatedUser(user_id) => {
                        let Some(user) = users.get(user_id).cloned() else {
                            return Ok(None);
                        };
                        PublishedAnswerAuthor::AuthenticatedUser(user)
                    }
                    AnswerAuthor::Temporary(temporary_user) => {
                        PublishedAnswerAuthor::Temporary(temporary_user.clone())
                    }
                    AnswerAuthor::ImportedFromRedmine(author) => {
                        PublishedAnswerAuthor::ImportedFromRedmine(author.clone())
                    }
                }
            }
        };

        Ok(Some(AnswerDetails {
            form_id,
            answer: PublishedAnswerEntry::new(answer.into_inner(), author),
            labels,
            answer_response_visibility,
        }))
    }

    async fn visible_answer_details(
        &self,
        account_user: &AccountUser,
        actor: &Actor,
        hits: Vec<AnswerSearchHit>,
        visible_answers_by_id: &HashMap<AnswerId, Allowed<AnswerEntry, Read>>,
    ) -> Result<Vec<AnswerDetails>, Error> {
        stream::iter(hits)
            .map(|hit| async move {
                let Some(answer) = visible_answers_by_id.get(&hit.answer_id).cloned() else {
                    return Ok::<_, Error>(None);
                };

                self.answer_details(account_user, actor, answer).await
            })
            .buffered(SEARCH_DETAIL_FETCH_CONCURRENCY)
            .try_filter_map(|visible| ready(Ok(visible)))
            .try_collect()
            .await
    }

    async fn cross_search_comments_with_authors(
        &self,
        account_user: &AccountUser,
        comments: Vec<(FormId, Comment)>,
    ) -> Result<Vec<CrossSearchComment>, Error> {
        let users = resolve_user_references(
            self.user_repository,
            account_user,
            comments
                .iter()
                .filter_map(|(_, comment)| comment.commented_by().copied())
                .collect(),
        )
        .await?;

        Ok(comments
            .into_iter()
            .filter_map(|(form_id, comment)| {
                let commented_by = match comment.commented_by() {
                    Some(user_id) => users.get(user_id).cloned().map(CommentAuthor::Portal)?,
                    None => comment
                        .redmine_author()
                        .cloned()
                        .map(CommentAuthor::ImportedFromRedmine)?,
                };
                Some(CrossSearchComment {
                    form_id,
                    comment: CommentWithAuthor {
                        comment,
                        commented_by,
                    },
                })
            })
            .collect())
    }

    pub async fn search_users(
        &self,
        actor: &AccountUser,
        query: String,
    ) -> Result<Vec<AccountUser>, Error> {
        let actor = Actor::from(actor.clone());
        let users = self.search_repository.search_users(&query).await?;

        self.visible_users(&actor, users).await
    }

    pub async fn search_answers(
        &self,
        account_user: &AccountUser,
        query: String,
        form_id: Option<FormId>,
        status: Option<AnswerStatus>,
    ) -> Result<Vec<AnswerDetails>, Error> {
        let actor = Actor::from(account_user.clone());
        if let Some(form_id) = form_id {
            let Some(form) = self.active_form_repository.get(form_id).await? else {
                return Ok(Vec::new());
            };
            if form.try_read(actor.clone()).is_err() {
                return Ok(Vec::new());
            }
        }

        let hits = self
            .search_repository
            .search_answers(&query, form_id, status)
            .await?;
        let answer_ids = unique_answer_ids(hits.iter().map(|hit| hit.answer_id));
        let visible_answers_by_id = self
            .visible_answer_entries_by_id(&actor, answer_ids)
            .await?;

        self.visible_answer_details(account_user, &actor, hits, &visible_answers_by_id)
            .await
    }

    pub async fn cross_search(
        &self,
        account_user: &AccountUser,
        query: String,
    ) -> Result<CrossSearchOutput, Error> {
        let actor = Actor::from(account_user.clone());
        let (forms, users, label_for_forms, label_for_answers, answers, comments) = try_join!(
            self.search_repository.search_forms(&query),
            self.search_repository.search_users(&query),
            self.search_repository.search_labels_for_forms(&query),
            self.search_repository.search_labels_for_answers(&query),
            self.search_repository.search_answers(&query, None, None),
            self.search_repository.search_comments(&query)
        )?;

        let actor_ref = &actor;
        let answer_ids = unique_answer_ids(
            answers
                .iter()
                .map(|hit| hit.answer_id)
                .chain(comments.iter().map(|hit| hit.answer_id)),
        );

        let visible_forms = stream::iter(forms)
            .map(|form| async move { self.visible_form_with_labels(actor_ref, form.form_id).await })
            .buffered(SEARCH_DETAIL_FETCH_CONCURRENCY)
            .try_filter_map(|visible| ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_users = self.visible_users(actor_ref, users).await?;

        let visible_label_for_forms = stream::iter(label_for_forms)
            .map(|label| async move {
                self.form_label_repository
                    .fetch_label(label.label_id)
                    .await
                    .map(|guard| {
                        guard.and_then(|guard| {
                            guard
                                .try_read(actor_ref.clone())
                                .ok()
                                .map(|label| label.into_inner())
                        })
                    })
            })
            .buffered(SEARCH_DETAIL_FETCH_CONCURRENCY)
            .try_filter_map(|visible| ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_label_for_answers = stream::iter(label_for_answers)
            .map(|label| async move {
                self.form_answer_label_repository
                    .get_label_for_answers(label.label_id)
                    .await
                    .map(|guard| {
                        guard.and_then(|guard| {
                            guard
                                .try_read(actor_ref.clone())
                                .ok()
                                .map(|label| label.into_inner())
                        })
                    })
            })
            .buffered(SEARCH_DETAIL_FETCH_CONCURRENCY)
            .try_filter_map(|visible| ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_answers_by_id = self
            .visible_answer_entries_by_id(actor_ref, answer_ids)
            .await?;
        let visible_answers = self
            .visible_answer_details(account_user, actor_ref, answers, &visible_answers_by_id)
            .await?;

        let visible_comments: Vec<(FormId, Comment)> = stream::iter(comments)
            .map(|comment| {
                let visible_answers_by_id = &visible_answers_by_id;

                async move {
                    let Some(answer) = visible_answers_by_id.get(&comment.answer_id).cloned()
                    else {
                        return Ok::<_, Error>(None);
                    };
                    let form_id = *answer.form_id();
                    let Some(thread) = self.comment_thread_for_answer(actor_ref, answer).await?
                    else {
                        return Ok::<_, Error>(None);
                    };

                    Ok::<_, Error>(
                        thread
                            .comments()
                            .iter()
                            .find(|loaded| *loaded.comment_id() == comment.comment_id)
                            .cloned()
                            .map(|comment| (form_id, comment)),
                    )
                }
            })
            .buffered(SEARCH_DETAIL_FETCH_CONCURRENCY)
            .try_filter_map(|visible| ready(Ok(visible)))
            .try_collect()
            .await?;
        let visible_comments = self
            .cross_search_comments_with_authors(account_user, visible_comments)
            .await?;

        Ok(CrossSearchOutput {
            forms: visible_forms,
            users: visible_users,
            label_for_forms: visible_label_for_forms,
            label_for_answers: visible_label_for_answers,
            answers: visible_answers,
            comments: visible_comments,
        })
    }

    pub async fn start_sync(
        &self,
        receiver: Receiver<SearchableFieldsWithOperation>,
        mut shutdown_status: watch::Receiver<bool>,
    ) -> Result<(), Error> {
        let mut receiver = receiver;

        loop {
            if *shutdown_status.borrow() {
                return Ok(());
            }

            let pending = tokio::select! {
                biased;
                _ = shutdown_status.changed() => return Ok(()),
                pending = receiver.recv() => pending,
            };
            let Some(pending) = pending else {
                return Ok(());
            };

            loop {
                if *shutdown_status.borrow() {
                    return Ok(());
                }

                let result = tokio::select! {
                    biased;
                    _ = shutdown_status.changed() => return Ok(()),
                    result = self.search_repository
                        .sync_search_engine(std::slice::from_ref(&pending))
                        // CDC consumer からチャンネル越しに受け取るため trace context はなく、
                        // 同期 1 件ごとに新しいルートスパンを作る
                        .instrument(tracing::info_span!(parent: None, "search_engine.sync")) => result,
                };

                match result {
                    Ok(()) => break,
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            retry_after_seconds = SEARCH_ENGINE_SYNC_RETRY_DELAY.as_secs(),
                            "failed to synchronize search engine; retrying",
                        );

                        tokio::select! {
                            biased;
                            _ = shutdown_status.changed() => return Ok(()),
                            _ = time::sleep(SEARCH_ENGINE_SYNC_RETRY_DELAY) => {}
                        }
                    }
                }
            }
        }
    }

    pub async fn start_watch_out_of_sync(
        &self,
        mut shutdown_status: watch::Receiver<bool>,
    ) -> Result<(), Error> {
        let mut interval = time::interval(OUT_OF_SYNC_CHECK_INTERVAL);
        // 1 回の再同期が interval より長引いても、溜まった tick が一斉に発火して
        // 再同期を多重に走らせないようにする
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        let mut pass = None;

        loop {
            if *shutdown_status.borrow() {
                break;
            }

            tokio::select! {
                biased;
                result = shutdown_status.changed() => {
                    if result.is_err() || *shutdown_status.borrow() {
                        break;
                    }
                },
                _ = interval.tick() => {
                    match self.resync_search_engine_step(pass.take()).await {
                        Ok(next) => pass = next,
                        Err(error) => {
                            tracing::error!(error = %error, "failed to check search engine synchronization");
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn repository_records(&self) -> Result<NumberOfRecordsPerAggregate, Error> {
        Ok(NumberOfRecordsPerAggregate {
            form_meta_data: NumberOfRecords(self.active_form_repository.size().await?),
            answers: NumberOfRecords(self.answer_entry_repository.size().await?),
            real_answers: NumberOfRecords(self.answer_entry_repository.content_size().await?),
            form_answer_comments: NumberOfRecords(self.comment_thread_repository.size().await?),
            label_for_form_answers: NumberOfRecords(
                self.form_answer_label_repository.size().await?,
            ),
            label_for_forms: NumberOfRecords(self.form_label_repository.size().await?),
            users: NumberOfRecords(self.user_repository.size().await?),
        })
    }

    async fn readable_forms_for_system(&self) -> Result<Vec<Allowed<ActiveForm, Read>>, Error> {
        let system = Actor::System;

        self.list_all_form_guards()
            .await?
            .into_iter()
            .map(|guard| guard.try_read(system.clone()).map_err(Into::into))
            .collect()
    }

    /// 検索エンジンとリポジトリの件数を比較し、乖離しているインデックスを少しずつ再同期する。
    ///
    /// 数万件規模で全件を一度に投入すると検索エンジンの CPU とファイルディスクリプタを使い切ってしまうため、
    /// 1 回の呼び出しで走査するドキュメント数を [`RESYNC_DOCUMENT_SCAN_BUDGET`] 件までに制限し、
    /// 途中経過を [`ResyncPass`] として返して次の呼び出しで再開する。
    ///
    /// 定期実行タスクのため、実行ごとに新しいルートスパンを作る。
    #[tracing::instrument(name = "search_engine.watch_out_of_sync", parent = None, skip_all)]
    async fn resync_search_engine_step(
        &self,
        pass: Option<ResyncPass>,
    ) -> Result<Option<ResyncPass>, Error> {
        let started = match pass {
            Some(pass) => Some(pass),
            None => self.start_resync_pass().await?,
        };
        let Some(mut pass) = started else {
            return Ok(None);
        };

        let mut budget = RESYNC_DOCUMENT_SCAN_BUDGET;

        if !pass.small_aggregates_done {
            self.resync_small_aggregates(&mut pass, &mut budget).await?;
            pass.small_aggregates_done = true;
        }

        self.resync_users(&mut pass, &mut budget).await?;
        self.resync_answers(&mut pass, &mut budget).await?;

        if !pass.is_complete() {
            return Ok(Some(pass));
        }

        self.delete_orphaned_documents(pass).await?;

        Ok(None)
    }

    /// 乖離しているインデックスを特定し、再同期を始める必要があれば [`ResyncPass`] を作る。
    async fn start_resync_pass(&self) -> Result<Option<ResyncPass>, Error> {
        let search_engine_records = self.search_repository.fetch_search_engine_stats().await?;
        let repository_records = self.repository_records().await?;
        let out_of_sync_indexes = search_engine_records.out_of_sync_indexes(&repository_records);

        if out_of_sync_indexes.is_empty() {
            return Ok(None);
        }

        tracing::info!(
            indexes = ?out_of_sync_indexes.iter().map(|index| index.as_str()).collect::<Vec<_>>(),
            "search engine is out of sync; starting incremental resync",
        );

        let indexed_ids = stream::iter(out_of_sync_indexes)
            .then(async |index| {
                Ok::<_, Error>((
                    index,
                    self.search_repository
                        .fetch_indexed_document_ids(index)
                        .await?,
                ))
            })
            .try_collect()
            .await?;

        Ok(Some(ResyncPass::new(indexed_ids, repository_records)))
    }

    /// フォームとラベルのように、件数が回答やユーザーほど増えないアグリゲートをまとめて走査する。
    async fn resync_small_aggregates(
        &self,
        pass: &mut ResyncPass,
        budget: &mut usize,
    ) -> Result<(), Error> {
        let forms = self
            .readable_forms_for_system()
            .await?
            .into_iter()
            .map(|form| {
                (
                    SearchableFields::FormMetaData(FormMetaData {
                        id: form.value().id().to_owned(),
                        title: form.value().title().to_owned(),
                        description: form.value().description().to_owned(),
                    }),
                    Operation::Update,
                )
            })
            .collect::<Vec<_>>();

        let labels_for_forms =
            system_readable_documents(self.form_label_repository.fetch_labels().await?, |label| {
                SearchableFields::LabelForForms(LabelForForms {
                    id: label.id().to_owned(),
                    name: label.name().to_owned().into_inner().into_inner(),
                })
            })?;

        let labels_for_answers = system_readable_documents(
            self.form_answer_label_repository
                .get_labels_for_answers()
                .await?,
            |label| {
                SearchableFields::LabelForFormAnswers(LabelForFormAnswers {
                    id: label.id().to_owned(),
                    name: label.name().to_owned().into_inner(),
                })
            },
        )?;

        self.sync_missing_documents(
            pass,
            forms
                .into_iter()
                .chain(labels_for_forms)
                .chain(labels_for_answers)
                .collect(),
            budget,
        )
        .await
    }

    /// ユーザーを 1 ページ分だけ検索ドキュメントへ投影し、次のページの要求と一緒に返す。
    async fn user_document_page(
        &self,
        request: PageRequest<UserPagePosition>,
    ) -> Result<DocumentPage<UserPagePosition>, Error> {
        let (users, next) = self
            .user_repository
            .fetch_users_page(request)
            .await?
            .into_parts();

        Ok((
            system_readable_documents(users, |user| {
                SearchableFields::Users(Users {
                    id: user.id().into_inner(),
                    name: user.name().to_owned(),
                })
            })?,
            next_page_request(next),
        ))
    }

    /// 回答を 1 ページ分だけ検索ドキュメントへ投影し、次のページの要求と一緒に返す。
    ///
    /// コメントは回答 1 件ごとにスレッドを引く必要があるため、`with_comments` のときだけ取得する。
    async fn answer_document_page(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        request: PageRequest<AnswerPagePosition>,
        with_comments: bool,
    ) -> Result<DocumentPage<AnswerPagePosition>, Error> {
        let (answers, next) = self
            .answer_entry_repository
            .list_all(forms, request, AnswerListFilter::default())
            .await?
            .into_parts();

        let comments = if with_comments {
            self.comment_search_documents(forms, &answers).await?
        } else {
            Vec::new()
        };

        Ok((
            answer_search_documents(&answers)
                .into_iter()
                .chain(comments)
                .collect(),
            next_page_request(next),
        ))
    }

    async fn resync_users(&self, pass: &mut ResyncPass, budget: &mut usize) -> Result<(), Error> {
        while let Some(request) = pass.users.clone().filter(|_| *budget > 0) {
            let (documents, next) = self.user_document_page(request).await?;

            self.sync_missing_documents(pass, documents, budget).await?;
            pass.users = next;
        }

        Ok(())
    }

    async fn resync_answers(&self, pass: &mut ResyncPass, budget: &mut usize) -> Result<(), Error> {
        if pass.answers.is_none() {
            return Ok(());
        }

        let forms = self.readable_forms_for_system().await?;
        let with_comments = pass.is_scanning(SearchIndex::FormAnswerComments);

        while let Some(request) = pass.answers.clone().filter(|_| *budget > 0) {
            let (documents, next) = self
                .answer_document_page(&forms, request, with_comments)
                .await?;

            self.sync_missing_documents(pass, documents, budget).await?;
            pass.answers = next;
        }

        Ok(())
    }

    /// 走査したドキュメントのうち、検索エンジンに存在しないものだけを投入する。
    async fn sync_missing_documents(
        &self,
        pass: &mut ResyncPass,
        documents: Vec<SearchableFieldsWithOperation>,
        budget: &mut usize,
    ) -> Result<(), Error> {
        *budget = budget.saturating_sub(documents.len());

        let missing = pass.take_missing(documents);

        if missing.is_empty() {
            return Ok(());
        }

        self.search_repository.sync_search_engine(&missing).await
    }

    /// 走査し終えても検索エンジンに残っていた、リポジトリ側に対応する行がないドキュメントを削除する。
    ///
    /// 走査した件数がリポジトリの件数に届いていないインデックスは、
    /// 走査漏れと区別できないため削除しない。
    async fn delete_orphaned_documents(&self, pass: ResyncPass) -> Result<(), Error> {
        let ResyncPass {
            indexed_ids,
            scanned,
            repository_records,
            ..
        } = pass;

        let deletions = indexed_ids
            .into_iter()
            .filter(|(_, orphaned_ids)| !orphaned_ids.is_empty())
            .filter(|(index, orphaned_ids)| {
                let scanned = scanned.get(index).copied().unwrap_or_default();
                let expected = repository_records.records_of(*index).0 as usize;
                let covered_whole_repository = scanned >= expected;

                if !covered_whole_repository {
                    tracing::warn!(
                        index = index.as_str(),
                        scanned,
                        expected,
                        orphaned = orphaned_ids.len(),
                        "skipped deleting search documents because the scan did not cover the whole repository",
                    );
                }

                covered_whole_repository
            })
            .collect::<Vec<_>>();

        stream::iter(deletions)
            .map(Ok)
            .try_for_each(async |(index, orphaned_ids): (_, HashSet<_>)| {
                tracing::info!(
                    index = index.as_str(),
                    count = orphaned_ids.len(),
                    "deleting search documents that no longer exist in the repository",
                );

                self.search_repository
                    .delete_search_documents(index, orphaned_ids.into_iter().collect())
                    .await
            })
            .await
    }

    pub async fn initialize_search_engine(&self) -> Result<(), Error> {
        if !self.search_repository.initialize_search_engine().await? {
            return Ok(());
        }

        let forms = self.readable_forms_for_system().await?;
        // 数万件規模の回答をすべてメモリに載せないよう、ページ単位で投影して投入する
        let mut request = Some(PageRequest::first(resync_page_limit()));

        while let Some(page_request) = request {
            let (documents, next) = self
                .answer_document_page(&forms, page_request, true)
                .await?;

            self.search_repository
                .sync_search_engine(&documents)
                .await?;

            request = next;
        }

        Ok(())
    }
}

fn answer_search_documents(
    answer_entries: &[Allowed<AnswerEntry, Read>],
) -> Vec<SearchableFieldsWithOperation> {
    answer_entries
        .iter()
        .flat_map(|entry| {
            let entry = entry.value();
            once((
                SearchableFields::AnswerTitle(AnswerTitleSearchDocument {
                    id: *entry.id(),
                    form_id: *entry.form_id(),
                    title: entry.title().clone(),
                    status: *entry.status(),
                }),
                Operation::Update,
            ))
            .chain(entry.contents().iter().map(|content| {
                (
                    SearchableFields::RealAnswers(RealAnswers {
                        id: content.id,
                        answer_id: *entry.id(),
                        question_id: content.question_id,
                        answer: content.answer.to_owned(),
                        status: *entry.status(),
                    }),
                    Operation::Update,
                )
            }))
        })
        .collect()
}

fn unique_answer_ids(answer_ids: impl IntoIterator<Item = AnswerId>) -> Vec<AnswerId> {
    let mut seen = HashSet::new();

    answer_ids
        .into_iter()
        .filter(|answer_id| seen.insert(*answer_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::repositories::{
        InMemoryActiveFormRepository, InMemoryAnswerEntryRepository, InMemoryFormLabelRepository,
        InMemoryUserRepository,
    };
    use chrono::Utc;
    use domain::{
        account::models::{Role, UserGroup, UserGroupName},
        form::{
            answer::{
                AnswerAuthor, AnswerAuthorPublicationPolicy, AnswerEntry, AnswerPublication,
                AnswerSettings, AnswerTitle, AnswerVisibility, FormAnswerContent,
                FormAnswerContentId, PostedAnswerContents,
            },
            comment::{Comment, CommentContent, CommentId},
            models::{AllowedUserGroups, FormDescription, FormSettings, FormTitle},
            question::{Question, QuestionSet},
        },
        repository::{
            form::{
                answer_entry_repository::MockAnswerEntryRepository,
                answer_label_repository::MockAnswerLabelRepository,
                comment_thread_repository::MockCommentThreadRepository,
            },
            search_repository::MockSearchRepository,
        },
        search::models::{
            AnswerSearchHit, CommentSearchHit, FormSearchHit, SearchableFields,
            SearchableFieldsWithOperation, Users,
        },
    };
    use std::sync::{Arc, Mutex};
    use tokio::sync::{Notify, mpsc, watch};
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn form_restricted_to(title: &str, group: &UserGroup) -> ActiveForm {
        let question = Question::new_text(
            "body".to_string().try_into().unwrap(),
            0,
            "Body".to_string().try_into().unwrap(),
            None,
            true,
        )
        .unwrap();
        let questions =
            QuestionSet::try_new(NonEmptyVec::try_new(vec![question]).unwrap()).unwrap();

        ActiveForm::new(
            FormTitle::new(title.to_string().try_into().unwrap()),
            FormDescription::default(),
            questions,
        )
        .change_settings(
            FormSettings::new()
                .change_allowed_user_groups(AllowedUserGroups::new(vec![*group.id()])),
        )
    }

    struct SearchSyncDependencies {
        active_form_repository: InMemoryActiveFormRepository,
        form_answer_label_repository: MockAnswerLabelRepository,
        form_label_repository: InMemoryFormLabelRepository,
        user_repository: InMemoryUserRepository,
        answer_entry_repository: InMemoryAnswerEntryRepository,
        comment_thread_repository: MockCommentThreadRepository,
    }

    impl SearchSyncDependencies {
        fn new() -> Self {
            Self {
                active_form_repository: InMemoryActiveFormRepository::default(),
                form_answer_label_repository: MockAnswerLabelRepository::new(),
                form_label_repository: InMemoryFormLabelRepository,
                user_repository: InMemoryUserRepository::default(),
                answer_entry_repository: InMemoryAnswerEntryRepository::default(),
                comment_thread_repository: MockCommentThreadRepository::new(),
            }
        }

        fn use_case<'a>(
            &'a self,
            search_repository: &'a MockSearchRepository,
        ) -> SearchUseCase<
            'a,
            MockSearchRepository,
            InMemoryActiveFormRepository,
            MockAnswerLabelRepository,
            InMemoryFormLabelRepository,
            InMemoryUserRepository,
            InMemoryAnswerEntryRepository,
            MockCommentThreadRepository,
        > {
            SearchUseCase {
                search_repository,
                active_form_repository: &self.active_form_repository,
                form_answer_label_repository: &self.form_answer_label_repository,
                form_label_repository: &self.form_label_repository,
                user_repository: &self.user_repository,
                answer_entry_repository: &self.answer_entry_repository,
                comment_thread_repository: &self.comment_thread_repository,
            }
        }
    }

    fn user_search_event(id: Uuid) -> SearchableFieldsWithOperation {
        (
            SearchableFields::Users(Users {
                id,
                name: id.to_string(),
            }),
            Operation::Update,
        )
    }

    fn user_id_from_search_event(data: &[SearchableFieldsWithOperation]) -> Uuid {
        match &data[0].0 {
            SearchableFields::Users(user) => user.id,
            _ => panic!("test event must be a user document"),
        }
    }

    fn temporary_search_error() -> Error {
        errors::infra::InfraError::Unexpected {
            cause: "temporary search engine failure".to_string(),
        }
        .into()
    }

    #[tokio::test]
    async fn start_sync_retries_pending_event_before_processing_the_next_event() {
        let first_id = Uuid::from_u128(1);
        let second_id = Uuid::from_u128(2);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let call_progress = Arc::new(Notify::new());
        let mut search_repository = MockSearchRepository::new();
        let calls_for_repository = Arc::clone(&calls);
        let progress_for_repository = Arc::clone(&call_progress);
        search_repository
            .expect_sync_search_engine()
            .times(3)
            .returning(move |data| {
                let mut calls = calls_for_repository.lock().unwrap();
                let attempt = calls.len();
                calls.push(user_id_from_search_event(data));
                progress_for_repository.notify_one();

                if attempt == 0 {
                    Err(temporary_search_error())
                } else {
                    Ok(())
                }
            });

        let dependencies = SearchSyncDependencies::new();
        let use_case = dependencies.use_case(&search_repository);
        let (sender, receiver) = mpsc::channel(2);
        let (_shutdown_sender, shutdown_status) = watch::channel(false);
        let sync = use_case.start_sync(receiver, shutdown_status);
        let calls_for_producer = Arc::clone(&calls);
        let producer = async move {
            sender.send(user_search_event(first_id)).await.unwrap();
            sender.send(user_search_event(second_id)).await.unwrap();

            call_progress.notified().await;
            assert_eq!(calls_for_producer.lock().unwrap().as_slice(), &[first_id]);

            call_progress.notified().await;
            call_progress.notified().await;
            drop(sender);
        };

        let (result, ()) = tokio::join!(sync, producer);

        assert!(result.is_ok());
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            &[first_id, first_id, second_id]
        );
    }

    /// 件数比較しか行わない依存を、再同期のテストで使えるようにまとめて用意する。
    fn empty_aggregate_dependencies() -> (MockAnswerLabelRepository, MockCommentThreadRepository) {
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository.expect_size().returning(|| Ok(0));
        answer_label_repository
            .expect_get_labels_for_answers()
            .returning(|| Ok(vec![]));

        let mut comment_thread_repository = MockCommentThreadRepository::new();
        comment_thread_repository.expect_size().returning(|| Ok(0));

        (answer_label_repository, comment_thread_repository)
    }

    #[tokio::test]
    async fn resync_pushes_only_documents_missing_from_the_search_engine() {
        let indexed_user = AccountUser::new(
            "indexed".to_string(),
            Uuid::from_u128(1).into(),
            Role::StandardUser,
        );
        let missing_user = AccountUser::new(
            "missing".to_string(),
            Uuid::from_u128(2).into(),
            Role::StandardUser,
        );
        let orphaned_id = Uuid::from_u128(3);
        let indexed_ids = HashSet::from([indexed_user.id().into_inner(), orphaned_id]);

        let synced = Arc::new(Mutex::new(Vec::new()));
        let deleted = Arc::new(Mutex::new(Vec::new()));
        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_fetch_search_engine_stats()
            .returning(|| {
                Ok(NumberOfRecordsPerAggregate {
                    users: NumberOfRecords(1),
                    ..Default::default()
                })
            });
        search_repository
            .expect_fetch_indexed_document_ids()
            .withf(|index| *index == SearchIndex::Users)
            .returning(move |_| Ok(indexed_ids.clone()));
        let synced_for_repository = Arc::clone(&synced);
        search_repository
            .expect_sync_search_engine()
            .returning(move |data| {
                synced_for_repository
                    .lock()
                    .unwrap()
                    .extend(data.iter().map(|(fields, _)| fields.document_id()));
                Ok(())
            });
        let deleted_for_repository = Arc::clone(&deleted);
        search_repository
            .expect_delete_search_documents()
            .returning(move |index, ids| {
                deleted_for_repository.lock().unwrap().push((index, ids));
                Ok(())
            });

        let (answer_label_repository, comment_thread_repository) = empty_aggregate_dependencies();
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(indexed_user.clone());
        user_repository.save_user(missing_user.clone());
        let active_form_repository = InMemoryActiveFormRepository::default();
        let form_label_repository = InMemoryFormLabelRepository;
        let answer_entry_repository = InMemoryAnswerEntryRepository::default();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_thread_repository,
        };

        let remaining = use_case.resync_search_engine_step(None).await.unwrap();

        assert!(
            remaining.is_none(),
            "走査しきれる件数なら 1 回の呼び出しで巡回が完了する"
        );
        assert_eq!(
            synced.lock().unwrap().as_slice(),
            &[missing_user.id().into_inner()],
            "検索エンジンに存在するドキュメントは投入し直さない"
        );
        assert_eq!(
            deleted.lock().unwrap().as_slice(),
            &[(SearchIndex::Users, vec![orphaned_id])],
            "リポジトリ側に対応する行がないドキュメントは削除する"
        );
    }

    #[tokio::test]
    async fn resync_does_not_scan_anything_while_record_counts_match() {
        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_fetch_search_engine_stats()
            .returning(|| Ok(NumberOfRecordsPerAggregate::default()));
        search_repository
            .expect_fetch_indexed_document_ids()
            .never();
        search_repository.expect_sync_search_engine().never();
        search_repository.expect_delete_search_documents().never();

        let (answer_label_repository, comment_thread_repository) = empty_aggregate_dependencies();
        let active_form_repository = InMemoryActiveFormRepository::default();
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        let answer_entry_repository = InMemoryAnswerEntryRepository::default();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_thread_repository,
        };

        assert!(
            use_case
                .resync_search_engine_step(None)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn start_sync_returns_when_receiver_is_closed() {
        let search_repository = MockSearchRepository::new();
        let dependencies = SearchSyncDependencies::new();
        let use_case = dependencies.use_case(&search_repository);
        let (sender, receiver) = mpsc::channel(1);
        drop(sender);
        let (_shutdown_sender, shutdown_status) = watch::channel(false);

        let result = tokio::time::timeout(
            Duration::from_secs(1),
            use_case.start_sync(receiver, shutdown_status),
        )
        .await
        .expect("start_sync must stop after its receiver is closed");

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn start_sync_returns_when_shutdown_was_already_requested() {
        let search_repository = MockSearchRepository::new();
        let dependencies = SearchSyncDependencies::new();
        let use_case = dependencies.use_case(&search_repository);
        let (_sender, receiver) = mpsc::channel(1);
        let (shutdown_sender, shutdown_status) = watch::channel(false);
        shutdown_sender.send(true).unwrap();

        let result = tokio::time::timeout(
            Duration::from_secs(1),
            use_case.start_sync(receiver, shutdown_status),
        )
        .await
        .expect("start_sync must stop after shutdown was requested");

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn cross_search_excludes_only_form_hits_the_actor_cannot_read() {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let other_group =
            UserGroup::new(UserGroupName::new("other".to_string().try_into().unwrap()));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(1).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let hidden_form = form_restricted_to("hidden", &other_group);
        let readable_form = form_restricted_to("readable", &member_group);
        let hidden_form_id = *hidden_form.id();
        let readable_form_id = *readable_form.id();

        let mut search_repository = MockSearchRepository::new();
        search_repository.expect_search_forms().returning(move |_| {
            Ok(vec![
                FormSearchHit {
                    form_id: hidden_form_id,
                },
                FormSearchHit {
                    form_id: readable_form_id,
                },
            ])
        });
        search_repository
            .expect_search_users()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_forms()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_answers()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_answers()
            .returning(|_, _, _| Ok(vec![]));
        search_repository
            .expect_search_comments()
            .returning(|_| Ok(vec![]));

        let active_form_repository =
            InMemoryActiveFormRepository::new(vec![hidden_form, readable_form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        let answer_entry_repository = InMemoryAnswerEntryRepository::default();
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "form".to_string())
            .await
            .unwrap();

        assert_eq!(output.forms.len(), 1);
        assert_eq!(*output.forms[0].form.id(), readable_form_id);
    }

    #[tokio::test]
    async fn search_answers_excludes_unreadable_hits_and_preserves_hit_order_and_duplicates() {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let other_group =
            UserGroup::new(UserGroupName::new("other".to_string().try_into().unwrap()));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(20).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let answer_author = AccountUser::new(
            "answer author".to_string(),
            Uuid::from_u128(21).into(),
            Role::StandardUser,
        );
        let readable_form = form_restricted_to("readable answers", &member_group)
            .change_answer_settings(
                AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
            );
        let hidden_form = form_restricted_to("hidden answers", &other_group)
            .change_answer_settings(
                AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
            );
        let readable_form_id = *readable_form.id();
        let answer_for = |form: &ActiveForm| {
            let question_id = *form.questions().as_slice()[0].id();
            AnswerEntry::new(
                *form.id(),
                AnswerAuthor::AuthenticatedUser(*answer_author.id()),
                AnswerTitle::default(),
                PostedAnswerContents::try_new(
                    form.questions().as_slice(),
                    vec![FormAnswerContent {
                        id: FormAnswerContentId::from(Uuid::new_v4()),
                        question_id: question_id.into(),
                        answer: "body".to_string(),
                    }],
                )
                .unwrap(),
            )
        };
        let visible_answer_a = answer_for(&readable_form);
        let visible_answer_b = answer_for(&readable_form);
        let private_answer =
            answer_for(&readable_form).change_publication(AnswerPublication::PRIVATE);
        let hidden_answer = answer_for(&hidden_form);
        let visible_answer_a_id = *visible_answer_a.id();
        let visible_answer_b_id = *visible_answer_b.id();
        let private_answer_id = *private_answer.id();
        let hidden_answer_id = *hidden_answer.id();

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_answers()
            .withf(move |_, form_id, _| *form_id == Some(readable_form_id))
            .returning(move |_, _, _| {
                Ok(vec![
                    AnswerSearchHit {
                        answer_id: visible_answer_b_id,
                    },
                    AnswerSearchHit {
                        answer_id: visible_answer_b_id,
                    },
                    AnswerSearchHit {
                        answer_id: private_answer_id,
                    },
                    AnswerSearchHit {
                        answer_id: hidden_answer_id,
                    },
                    AnswerSearchHit {
                        answer_id: visible_answer_a_id,
                    },
                ])
            });

        let active_form_repository =
            InMemoryActiveFormRepository::new(vec![readable_form, hidden_form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .times(3)
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(answer_author.clone());
        let mut answer_entry_repository = MockAnswerEntryRepository::new();
        answer_entry_repository
            .expect_find_by_ids()
            .withf(move |_, answer_ids| {
                answer_ids
                    == &vec![
                        visible_answer_b_id,
                        private_answer_id,
                        hidden_answer_id,
                        visible_answer_a_id,
                    ]
            })
            .return_once(move |forms, _| {
                let form = forms
                    .iter()
                    .find(|form| form.id() == visible_answer_a.form_id())
                    .unwrap();

                Ok(vec![visible_answer_a, visible_answer_b, private_answer]
                    .into_iter()
                    .filter_map(|answer| form.read_entry(answer).ok())
                    .collect())
            });
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let answers = use_case
            .search_answers(&actor, "answer".to_string(), Some(readable_form_id), None)
            .await
            .unwrap();

        let answer_ids = answers
            .iter()
            .map(|answer| answer.answer.id)
            .collect::<Vec<_>>();
        assert_eq!(
            answer_ids,
            vec![
                visible_answer_b_id,
                visible_answer_b_id,
                visible_answer_a_id
            ]
        );
    }

    #[tokio::test]
    async fn search_answers_keeps_an_answer_with_a_hidden_missing_author_as_anonymous() {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(20).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let form = form_restricted_to("hidden author", &member_group).change_answer_settings(
            AnswerSettings::default()
                .change_visibility(AnswerVisibility::PUBLIC)
                .change_author_publication_policy(AnswerAuthorPublicationPolicy::Hide),
        );
        let form_id = *form.id();
        let question_id = *form.questions().as_slice()[0].id();
        let answer = AnswerEntry::new(
            form_id,
            AnswerAuthor::AuthenticatedUser(Uuid::from_u128(999).into()),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(
                form.questions().as_slice(),
                vec![FormAnswerContent {
                    id: FormAnswerContentId::from(Uuid::new_v4()),
                    question_id: question_id.into(),
                    answer: "body".to_string(),
                }],
            )
            .unwrap(),
        );
        let answer_id = *answer.id();

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_answers()
            .return_once(move |_, _, _| Ok(vec![AnswerSearchHit { answer_id }]));
        let active_form_repository = InMemoryActiveFormRepository::new(vec![form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        let answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let answers = use_case
            .search_answers(&actor, "answer".to_string(), Some(form_id), None)
            .await
            .unwrap();

        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].answer.id, answer_id);
        assert!(matches!(
            answers[0].answer.author,
            PublishedAnswerAuthor::Anonymous
        ));
    }

    #[tokio::test]
    async fn search_answers_returns_empty_without_searching_for_a_missing_form() {
        let actor = AccountUser::new(
            "viewer".to_string(),
            Uuid::from_u128(20).into(),
            Role::StandardUser,
        );
        let missing_form_id = Uuid::from_u128(21).into();
        let search_repository = MockSearchRepository::new();
        let active_form_repository = InMemoryActiveFormRepository::default();
        let answer_label_repository = MockAnswerLabelRepository::new();
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        let answer_entry_repository = InMemoryAnswerEntryRepository::default();
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let answers = use_case
            .search_answers(&actor, "answer".to_string(), Some(missing_form_id), None)
            .await
            .unwrap();

        assert!(answers.is_empty());
    }

    #[tokio::test]
    async fn search_answers_returns_empty_without_searching_for_an_unreadable_form() {
        let permitted_group = UserGroup::new(UserGroupName::new(
            "permitted".to_string().try_into().unwrap(),
        ));
        let actor = AccountUser::new(
            "viewer".to_string(),
            Uuid::from_u128(20).into(),
            Role::StandardUser,
        );
        let unreadable_form = form_restricted_to("unreadable", &permitted_group);
        let unreadable_form_id = *unreadable_form.id();
        let search_repository = MockSearchRepository::new();
        let active_form_repository = InMemoryActiveFormRepository::new(vec![unreadable_form]);
        let answer_label_repository = MockAnswerLabelRepository::new();
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        let answer_entry_repository = InMemoryAnswerEntryRepository::default();
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let answers = use_case
            .search_answers(&actor, "answer".to_string(), Some(unreadable_form_id), None)
            .await
            .unwrap();

        assert!(answers.is_empty());
    }

    #[tokio::test]
    async fn cross_search_excludes_only_answer_hits_whose_author_is_missing() {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(1).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let form = form_restricted_to("answers", &member_group).change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let form_id = *form.id();
        let question_id = *form.questions().as_slice()[0].id();
        let answer_contents = || {
            PostedAnswerContents::try_new(
                form.questions().as_slice(),
                vec![FormAnswerContent {
                    id: FormAnswerContentId::from(Uuid::new_v4()),
                    question_id: question_id.into(),
                    answer: "body".to_string(),
                }],
            )
            .unwrap()
        };
        let missing_author_answer = AnswerEntry::new(
            form_id,
            AnswerAuthor::AuthenticatedUser(Uuid::from_u128(4).into()),
            AnswerTitle::default(),
            answer_contents(),
        );
        let visible_answer = AnswerEntry::new(
            form_id,
            AnswerAuthor::AuthenticatedUser(*actor.id()),
            AnswerTitle::default(),
            answer_contents(),
        );
        let missing_author_answer_id = *missing_author_answer.id();
        let visible_answer_id = *visible_answer.id();

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_forms()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_users()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_forms()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_answers()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_answers()
            .returning(move |_, _, _| {
                Ok(vec![
                    AnswerSearchHit {
                        answer_id: missing_author_answer_id,
                    },
                    AnswerSearchHit {
                        answer_id: visible_answer_id,
                    },
                ])
            });
        search_repository
            .expect_search_comments()
            .returning(|_| Ok(vec![]));

        let active_form_repository = InMemoryActiveFormRepository::new(vec![form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(actor.clone());
        let answer_entry_repository =
            InMemoryAnswerEntryRepository::new(vec![missing_author_answer, visible_answer]);
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "answer".to_string())
            .await
            .unwrap();

        assert_eq!(output.answers.len(), 1);
        assert_eq!(output.answers[0].answer.id, visible_answer_id);
    }

    #[tokio::test]
    async fn cross_search_excludes_only_comment_hits_whose_author_is_missing() {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(10).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let form = form_restricted_to("comments", &member_group).change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let form_id = *form.id();
        let question_id = *form.questions().as_slice()[0].id();
        let answer = AnswerEntry::new(
            *form.id(),
            AnswerAuthor::AuthenticatedUser(*actor.id()),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(
                form.questions().as_slice(),
                vec![FormAnswerContent {
                    id: FormAnswerContentId::from(Uuid::new_v4()),
                    question_id: question_id.into(),
                    answer: "body".to_string(),
                }],
            )
            .unwrap(),
        );
        let answer_id = *answer.id();
        let first_comment_id = CommentId::from(Uuid::from_u128(11));
        let missing_author_comment_id = CommentId::from(Uuid::from_u128(12));
        let second_comment_id = CommentId::from(Uuid::from_u128(13));
        let comment = |comment_id, commented_by, content: &str| unsafe {
            Comment::from_raw_parts(
                answer_id,
                comment_id,
                CommentContent::new(content.to_string().try_into().unwrap()),
                Utc::now(),
                commented_by,
            )
        };
        let first_comment = comment(first_comment_id, *actor.id(), "first");
        let missing_author_comment = comment(
            missing_author_comment_id,
            Uuid::from_u128(14).into(),
            "missing author",
        );
        let second_comment = comment(second_comment_id, *actor.id(), "second");

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_forms()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_users()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_forms()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_answers()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_answers()
            .returning(move |_, _, _| {
                Ok(vec![
                    AnswerSearchHit { answer_id },
                    AnswerSearchHit { answer_id },
                ])
            });
        search_repository
            .expect_search_comments()
            .returning(move |_| {
                Ok(vec![
                    CommentSearchHit {
                        comment_id: second_comment_id,
                        answer_id,
                    },
                    CommentSearchHit {
                        comment_id: missing_author_comment_id,
                        answer_id,
                    },
                    CommentSearchHit {
                        comment_id: first_comment_id,
                        answer_id,
                    },
                ])
            });

        let active_form_repository = InMemoryActiveFormRepository::new(vec![form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .times(2)
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(actor.clone());
        let mut answer_entry_repository = MockAnswerEntryRepository::new();
        answer_entry_repository
            .expect_find_by_ids()
            .withf(move |_, answer_ids| answer_ids == &vec![answer_id])
            .return_once(move |forms, _| {
                let form = forms
                    .iter()
                    .find(|form| form.id() == answer.form_id())
                    .unwrap();
                Ok(vec![form.read_entry(answer).unwrap()])
            });
        let stored_comments = vec![first_comment, missing_author_comment, second_comment];
        let mut comment_repository = MockCommentThreadRepository::new();
        comment_repository
            .expect_get_with_comments_for_answer()
            .returning(move |form, answer| {
                form.comment_thread_with_comments(answer, stored_comments.clone())
                    .map_err(Error::from)
            });
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "comment".to_string())
            .await
            .unwrap();

        let answer_ids = output
            .answers
            .iter()
            .map(|answer| answer.answer.id)
            .collect::<Vec<_>>();
        assert_eq!(answer_ids, vec![answer_id, answer_id]);

        let comment_ids = output
            .comments
            .iter()
            .map(|comment| *comment.comment.comment.comment_id())
            .collect::<Vec<_>>();
        assert_eq!(comment_ids, vec![second_comment_id, first_comment_id]);

        let form_ids = output
            .comments
            .iter()
            .map(|comment| comment.form_id)
            .collect::<Vec<_>>();
        assert_eq!(form_ids, vec![form_id, form_id]);
    }

    #[tokio::test]
    async fn cross_search_comments_keep_authorized_answers_parent_form_and_exclude_unavailable_or_unreadable_hits()
     {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let other_group =
            UserGroup::new(UserGroupName::new("other".to_string().try_into().unwrap()));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(20).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let form_a = form_restricted_to("comments a", &member_group).change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let form_b = form_restricted_to("comments b", &member_group).change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let unreadable_form = form_restricted_to("comments hidden", &other_group);
        let form_a_id = *form_a.id();
        let form_b_id = *form_b.id();
        let answer_author_id = Uuid::from_u128(28).into();
        let answer = |form: &ActiveForm| {
            AnswerEntry::new(
                *form.id(),
                AnswerAuthor::AuthenticatedUser(answer_author_id),
                AnswerTitle::default(),
                PostedAnswerContents::try_new(
                    form.questions().as_slice(),
                    vec![FormAnswerContent {
                        id: FormAnswerContentId::from(Uuid::new_v4()),
                        question_id: (*form.questions().as_slice()[0].id()).into(),
                        answer: "body".to_string(),
                    }],
                )
                .unwrap(),
            )
        };
        let answer_a = answer(&form_a);
        let answer_b = answer(&form_b);
        let private_answer = answer(&form_a).change_publication(AnswerPublication::PRIVATE);
        let unreadable_answer = answer(&unreadable_form);
        let answer_a_id = *answer_a.id();
        let answer_b_id = *answer_b.id();
        let private_answer_id = *private_answer.id();
        let unreadable_answer_id = *unreadable_answer.id();
        let unavailable_answer_id = AnswerId::from(Uuid::from_u128(21));
        let first_comment_id = CommentId::from(Uuid::from_u128(22));
        let missing_author_comment_id = CommentId::from(Uuid::from_u128(23));
        let second_comment_id = CommentId::from(Uuid::from_u128(24));
        let unavailable_comment_id = CommentId::from(Uuid::from_u128(25));
        let unreadable_comment_id = CommentId::from(Uuid::from_u128(27));
        let private_comment_id = CommentId::from(Uuid::from_u128(29));
        let comment = |answer_id, comment_id, commented_by, content: &str| unsafe {
            Comment::from_raw_parts(
                answer_id,
                comment_id,
                CommentContent::new(content.to_string().try_into().unwrap()),
                Utc::now(),
                commented_by,
            )
        };
        let first_comment = comment(answer_a_id, first_comment_id, *actor.id(), "first");
        let missing_author_comment = comment(
            answer_a_id,
            missing_author_comment_id,
            Uuid::from_u128(26).into(),
            "missing author",
        );
        let second_comment = comment(answer_b_id, second_comment_id, *actor.id(), "second");
        let unreadable_comment = comment(
            unreadable_answer_id,
            unreadable_comment_id,
            *actor.id(),
            "unreadable",
        );
        let private_comment = comment(
            private_answer_id,
            private_comment_id,
            *actor.id(),
            "private",
        );

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_forms()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_users()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_forms()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_answers()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_answers()
            .returning(|_, _, _| Ok(vec![]));
        search_repository
            .expect_search_comments()
            .returning(move |_| {
                Ok(vec![
                    CommentSearchHit {
                        comment_id: second_comment_id,
                        answer_id: answer_b_id,
                    },
                    CommentSearchHit {
                        comment_id: unavailable_comment_id,
                        answer_id: unavailable_answer_id,
                    },
                    CommentSearchHit {
                        comment_id: unreadable_comment_id,
                        answer_id: unreadable_answer_id,
                    },
                    CommentSearchHit {
                        comment_id: private_comment_id,
                        answer_id: private_answer_id,
                    },
                    CommentSearchHit {
                        comment_id: missing_author_comment_id,
                        answer_id: answer_a_id,
                    },
                    CommentSearchHit {
                        comment_id: first_comment_id,
                        answer_id: answer_a_id,
                    },
                ])
            });

        let active_form_repository =
            InMemoryActiveFormRepository::new(vec![form_a, form_b, unreadable_form]);
        let answer_label_repository = MockAnswerLabelRepository::new();
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(actor.clone());
        let answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![
            answer_a,
            answer_b,
            private_answer,
            unreadable_answer,
        ]);
        let mut comment_repository = MockCommentThreadRepository::new();
        comment_repository
            .expect_get_with_comments_for_answer()
            .returning(move |form, answer| {
                let comments = match *answer.id() {
                    id if id == answer_a_id => {
                        vec![first_comment.clone(), missing_author_comment.clone()]
                    }
                    id if id == answer_b_id => vec![second_comment.clone()],
                    id if id == private_answer_id => vec![private_comment.clone()],
                    id if id == unreadable_answer_id => vec![unreadable_comment.clone()],
                    _ => vec![],
                };

                form.comment_thread_with_comments(answer, comments)
                    .map_err(Error::from)
            });
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "comment".to_string())
            .await
            .unwrap();

        let comment_parent_pairs = output
            .comments
            .iter()
            .map(|comment| (*comment.comment.comment.comment_id(), comment.form_id))
            .collect::<Vec<_>>();
        assert_eq!(
            comment_parent_pairs,
            vec![
                (second_comment_id, form_b_id),
                (first_comment_id, form_a_id)
            ]
        );
    }

    #[tokio::test]
    async fn cross_search_excludes_comment_hits_for_private_answer_visible_to_its_author() {
        let group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let actor = AccountUser::with_groups(
            "author".to_string(),
            Uuid::new_v4().into(),
            Role::StandardUser,
            vec![group.clone()],
        );
        let form = form_restricted_to("form", &group).change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let answer = AnswerEntry::new(
            *form.id(),
            AnswerAuthor::AuthenticatedUser(*actor.id()),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(
                form.questions().as_slice(),
                vec![FormAnswerContent {
                    id: FormAnswerContentId::new(),
                    question_id: (*form.questions().as_slice()[0].id()).into(),
                    answer: "answer".to_string(),
                }],
            )
            .unwrap(),
        )
        .change_publication(AnswerPublication::PRIVATE);
        let answer_id = *answer.id();
        let comment_id = CommentId::new();

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_forms()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_users()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_forms()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_answers()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_answers()
            .returning(move |_, _, _| Ok(vec![AnswerSearchHit { answer_id }]));
        search_repository
            .expect_search_comments()
            .returning(move |_| {
                Ok(vec![CommentSearchHit {
                    comment_id,
                    answer_id,
                }])
            });

        let active_form_repository = InMemoryActiveFormRepository::new(vec![form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(actor.clone());
        let answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        let mut comment_repository = MockCommentThreadRepository::new();
        comment_repository
            .expect_get_with_comments_for_answer()
            .returning(|form, answer| {
                form.comment_thread_with_comments(answer, Vec::new())
                    .map_err(Error::from)
            });
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "private".to_string())
            .await
            .unwrap();

        assert_eq!(output.answers.len(), 1);
        assert!(output.comments.is_empty());
    }
}
