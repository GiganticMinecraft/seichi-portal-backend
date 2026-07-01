use crate::models::CrossSearchOutput;
use domain::repository::form::answer_entry_repository::AnswerEntryRepository;
use domain::repository::form::answer_label_repository::AnswerLabelRepository;
use domain::repository::form::comment_repository::CommentRepository;
use domain::repository::form::form_label_repository::FormLabelRepository;
use domain::repository::user_repository::UserRepository;
use domain::search::models::NumberOfRecords;
use domain::search::models::{NumberOfRecordsPerAggregate, Operation, UserSearchHit};
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::{
        answer::{AnswerEntry, AnswerId},
        models::ActiveForm,
    },
    pagination::{PageLimit, PageRequest},
    repository::{
        form::active_form_repository::ActiveFormRepository, search_repository::SearchRepository,
    },
    search::models::SearchableFieldsWithOperation,
    types::authorization_guard::{Allowed, AuthorizationGuard, Read},
};
use errors::Error;
use futures::{StreamExt, TryStreamExt, stream, try_join};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, mpsc::Receiver};
use tokio::time;

pub struct SearchUseCase<
    'a,
    SearchRepo: SearchRepository,
    FormRepo: ActiveFormRepository,
    FormAnswerLabelRepo: AnswerLabelRepository,
    FormLabelRepo: FormLabelRepository,
    UserRepo: UserRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    CommentRepo: CommentRepository,
> {
    pub search_repository: &'a SearchRepo,
    pub active_form_repository: &'a FormRepo,
    pub form_answer_label_repository: &'a FormAnswerLabelRepo,
    pub form_label_repository: &'a FormLabelRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub comment_repository: &'a CommentRepo,
}

impl<
    R1: SearchRepository,
    R2: ActiveFormRepository,
    R3: AnswerLabelRepository,
    R4: FormLabelRepository,
    R5: UserRepository,
    R6: AnswerEntryRepository,
    R7: CommentRepository,
> SearchUseCase<'_, R1, R2, R3, R4, R5, R6, R7>
{
    /// 認可済みの `answer_entries` から `answer_id` の回答を探して返す。回答の公開範囲は
    /// `answer_entries` を取得した時点で各フォームの読み取りガードが検証済みのため、ここでは
    /// 同一性の照合だけを行う。
    fn read_visible_answer(
        answer_entries: &[Allowed<AnswerEntry, Read>],
        answer_id: AnswerId,
    ) -> Option<Allowed<AnswerEntry, Read>> {
        answer_entries
            .iter()
            .find(|entry| *entry.value().id() == answer_id)
            .cloned()
    }

    async fn list_all_form_guards(
        &self,
    ) -> Result<Vec<AuthorizationGuard<ActiveForm, Read>>, Error> {
        self.active_form_repository.list_all().await
    }

    async fn list_all_answer_entries(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error> {
        let mut request = PageRequest::first(PageLimit::default_limit());
        let mut answers = Vec::new();

        loop {
            let page = self
                .answer_entry_repository
                .list_all(forms, request)
                .await?;
            let (items, next) = page.into_parts();
            answers.extend(items);

            let Some(next) = next else {
                break;
            };
            request = PageRequest::after(next, PageLimit::default_limit());
        }

        Ok(answers)
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

    pub async fn search_users(
        &self,
        actor: &AccountUser,
        query: String,
    ) -> Result<Vec<AccountUser>, Error> {
        let actor = Actor::from(actor.clone());
        let users = self.search_repository.search_users(&query).await?;

        self.visible_users(&actor, users).await
    }

    pub async fn cross_search(
        &self,
        actor: &AccountUser,
        query: String,
    ) -> Result<CrossSearchOutput, Error> {
        let actor = Actor::from(actor.clone());
        let (forms, users, label_for_forms, label_for_answers, answers, comments) = try_join!(
            self.search_repository.search_forms(&query),
            self.search_repository.search_users(&query),
            self.search_repository.search_labels_for_forms(&query),
            self.search_repository.search_labels_for_answers(&query),
            self.search_repository.search_answers(&query),
            self.search_repository.search_comments(&query)
        )?;

        let actor_ref = &actor;

        let visible_forms = stream::iter(forms)
            .then(|form| async move {
                self.active_form_repository
                    .get(form.form_id)
                    .await
                    .map(|guard| {
                        guard.and_then(|guard| {
                            guard
                                .try_read(actor_ref.clone())
                                .ok()
                                .map(|form| form.into_inner())
                        })
                    })
            })
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_users = self.visible_users(actor_ref, users).await?;

        let visible_label_for_forms = stream::iter(label_for_forms)
            .then(|label| async move {
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
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_label_for_answers = stream::iter(label_for_answers)
            .then(|label| async move {
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
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;

        let readable_forms = self
            .list_all_form_guards()
            .await?
            .into_iter()
            .filter_map(|form| form.try_read(actor_ref.clone()).ok())
            .collect::<Vec<_>>();
        let answer_entries = self.list_all_answer_entries(&readable_forms).await?;

        let visible_answers = stream::iter(answers)
            .then(|entry| {
                let answer_entries = &answer_entries;

                async move {
                    let Some(answer) = Self::read_visible_answer(answer_entries, entry.answer_id)
                    else {
                        return Ok::<_, Error>(None);
                    };

                    Ok::<_, Error>(Some(answer.into_inner()))
                }
            })
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_comments = stream::iter(comments)
            .then(|comment| {
                let answer_entries = &answer_entries;

                async move {
                    let Some(answer) = Self::read_visible_answer(answer_entries, comment.answer_id)
                    else {
                        return Ok::<_, Error>(None);
                    };

                    Ok::<_, Error>(
                        self.comment_repository
                            .find_by_answer(&answer)
                            .await?
                            .into_iter()
                            .find(|loaded| *loaded.value().comment_id() == comment.comment_id)
                            .map(|comment| comment.into_inner()),
                    )
                }
            })
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
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
        shutdown_notifier: Arc<Notify>,
    ) -> Result<(), Error> {
        let mut receiver = receiver;
        loop {
            tokio::select! {
                _ = shutdown_notifier.notified() => {
                    break;
                },
                _ = async {
                    if let Some(data) = receiver.recv().await {
                        self.search_repository.sync_search_engine(&[data]).await?
                    }

                    Ok::<_, Error>(())
                } => {}
            }
        }

        Ok(())
    }

    pub async fn start_watch_out_of_sync(
        &self,
        shutdown_notifier: Arc<Notify>,
    ) -> Result<(), Error> {
        let mut interval = time::interval(Duration::from_secs(60));

        loop {
            tokio::select! {
                _ = shutdown_notifier.notified() => {
                    break
                },
                _ = interval.tick() => {
                    let search_engine_records = self.search_repository.fetch_search_engine_stats().await?;

                    let repository_records = NumberOfRecordsPerAggregate {
                        form_meta_data: NumberOfRecords(self.active_form_repository.size().await?),
                        real_answers: NumberOfRecords(
                            self.answer_entry_repository.size().await?,
                        ),
                        form_answer_comments: NumberOfRecords(
                            self.comment_repository.size().await?,
                        ),
                        label_for_form_answers: NumberOfRecords(
                            self.form_answer_label_repository.size().await?,
                        ),
                        label_for_forms: NumberOfRecords(self.form_label_repository.size().await?),
                        users: NumberOfRecords(self.user_repository.size().await?),
                    };

                    let sync_rate = search_engine_records.try_into_sync_rate(&repository_records)?;

                    if sync_rate.is_out_of_sync() {
                        let system = Actor::System;

                        let form_guards = self
                            .list_all_form_guards()
                            .await?
                            .into_iter()
                            .map(|guard| guard.try_read(system.clone()).map_err(Into::into))
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let forms = form_guards
                            .iter()
                            .map(|form| {
                                Ok((
                                    domain::search::models::SearchableFields::FormMetaData(
                                        domain::search::models::FormMetaData {
                                            id: form.value().id().to_owned(),
                                            title: form.value().title().to_owned(),
                                            description: form.value().description().to_owned(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let answer_entries = self.list_all_answer_entries(&form_guards).await?;

                        let answers = answer_entries
                            .iter()
                            .flat_map(|entry| {
                                let entry = entry.value();
                                entry
                                    .contents()
                                    .iter()
                                    .map(|content| {
                                        (
                                            domain::search::models::SearchableFields::RealAnswers(
                                                domain::search::models::RealAnswers {
                                                    id: content.id,
                                                    answer_id: entry.id().to_owned(),
                                                    question_id: content.question_id,
                                                    answer: content.answer.to_owned(),
                                                },
                                            ),
                                            Operation::Update,
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>();

                        let comments = stream::iter(answer_entries.clone())
                        .then(|answer| async move {
                            self.comment_repository
                                .find_by_answer(&answer)
                                .await
                                .map(|comments| {
                                    comments
                                        .into_iter()
                                        .map(|comment| comment.into_inner())
                                        .map(|comment| {
                                            (
                                                domain::search::models::SearchableFields::FormAnswerComments(
                                                    domain::search::models::FormAnswerComments {
                                                        id: comment.comment_id().to_owned(),
                                                        answer_id: comment.answer_id().to_owned(),
                                                        content: comment
                                                            .content()
                                                            .to_owned()
                                                            .into_inner()
                                                            .into_inner(),
                                                    },
                                                ),
                                                Operation::Update,
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                })
                        })
                        .try_collect::<Vec<_>>()
                        .await?
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();

                        let labels_for_forms = self
                            .form_label_repository
                            .fetch_labels()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let label = guard.try_read(system.clone())?.into_inner();

                                Ok((
                                    domain::search::models::SearchableFields::LabelForForms(
                                        domain::search::models::LabelForForms {
                                            id: label.id().to_owned(),
                                            name: label.name().to_owned().into_inner().into_inner(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let labels_for_answers = self
                            .form_answer_label_repository
                            .get_labels_for_answers()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let label = guard.try_read(system.clone())?.into_inner();

                                Ok((
                                    domain::search::models::SearchableFields::LabelForFormAnswers(
                                        domain::search::models::LabelForFormAnswers {
                                            id: label.id().to_owned(),
                                            name: label.name().to_owned().into_inner(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let users = self
                            .user_repository
                            .fetch_all_users()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let user = guard.try_read(system.clone())?.into_inner();

                                Ok((
                                    domain::search::models::SearchableFields::Users(
                                        domain::search::models::Users {
                                            id: user.id().into_inner(),
                                            name: user.name().to_owned(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let data = forms
                            .into_iter()
                            .chain(answers)
                            .chain(comments)
                            .chain(labels_for_forms)
                            .chain(labels_for_answers)
                            .chain(users)
                            .collect::<Vec<_>>();

                        self.search_repository
                            .sync_search_engine(data.as_slice())
                            .await?;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn initialize_search_engine(&self) -> Result<(), Error> {
        self.search_repository.initialize_search_engine().await
    }
}
