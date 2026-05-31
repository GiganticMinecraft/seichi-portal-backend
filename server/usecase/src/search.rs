use crate::models::CrossSearchOutput;
use domain::repository::form::answer_entry_set_repository::AnswerEntrySetRepository;
use domain::repository::form::answer_label_repository::AnswerLabelRepository;
use domain::repository::form::comment_repository::CommentRepository;
use domain::repository::form::form_label_repository::FormLabelRepository;
use domain::repository::user_repository::UserRepository;
use domain::search::models::NumberOfRecords;
use domain::search::models::{NumberOfRecordsPerAggregate, Operation};
use domain::{
    repository::{
        form::active_form_repository::ActiveFormRepository, search_repository::SearchRepository,
    },
    search::models::SearchableFieldsWithOperation,
    user::models::{ActiveUser, Actor},
};
use errors::Error;
use futures::{StreamExt, TryStreamExt, stream, try_join};
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
    AnswerEntrySetRepo: AnswerEntrySetRepository,
    CommentRepo: CommentRepository,
> {
    pub search_repository: &'a SearchRepo,
    pub active_form_repository: &'a FormRepo,
    pub form_answer_label_repository: &'a FormAnswerLabelRepo,
    pub form_label_repository: &'a FormLabelRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_set_repository: &'a AnswerEntrySetRepo,
    pub comment_repository: &'a CommentRepo,
}

impl<
    R1: SearchRepository,
    R2: ActiveFormRepository,
    R3: AnswerLabelRepository,
    R4: FormLabelRepository,
    R5: UserRepository,
    R6: AnswerEntrySetRepository,
    R7: CommentRepository,
> SearchUseCase<'_, R1, R2, R3, R4, R5, R6, R7>
{
    pub async fn cross_search(
        &self,
        actor: &ActiveUser,
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

        let visible_users = stream::iter(users)
            .then(|user| async move {
                self.user_repository
                    .find_by(user.user_id.into_inner())
                    .await
                    .map(|guard| {
                        guard.and_then(|guard| {
                            guard
                                .try_read(actor_ref.clone())
                                .ok()
                                .map(|user| user.into_inner())
                        })
                    })
            })
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;

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

        let all_sets = self.answer_entry_set_repository.list_all().await?;

        let visible_answers = stream::iter(answers)
            .then(|entry| {
                let all_sets = &all_sets;

                async move {
                    let Some((answer, form_id)) = all_sets.iter().find_map(|set_guard| {
                        let answer_entry_set =
                            set_guard.clone().try_read(actor_ref.clone()).ok()?;
                        let answer = answer_entry_set.read_entry(entry.answer_id).ok()?;

                        Some((answer, *answer_entry_set.form_id()))
                    }) else {
                        return Ok::<_, Error>(None);
                    };

                    let is_form_visible = self
                        .active_form_repository
                        .get(form_id)
                        .await?
                        .is_some_and(|form_guard| form_guard.try_read(actor_ref.clone()).is_ok());

                    Ok::<_, Error>(is_form_visible.then_some(answer.into_inner()))
                }
            })
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_comments = stream::iter(comments)
            .then(|comment| {
                let all_sets = &all_sets;

                async move {
                    let Some((answer, comment_id, form_id)) =
                        all_sets.iter().find_map(|set_guard| {
                            let answer_entry_set =
                                set_guard.clone().try_read(actor_ref.clone()).ok()?;
                            let answer = answer_entry_set.read_entry(comment.answer_id).ok()?;

                            Some((answer, comment.comment_id, *answer_entry_set.form_id()))
                        })
                    else {
                        return Ok::<_, Error>(None);
                    };

                    let Some(comment) = self
                        .comment_repository
                        .find_by_answer(&answer)
                        .await?
                        .into_iter()
                        .find(|loaded| *loaded.value().comment_id() == comment_id)
                        .map(|comment| comment.into_inner())
                    else {
                        return Ok::<_, Error>(None);
                    };

                    let is_form_visible = self
                        .active_form_repository
                        .get(form_id)
                        .await?
                        .is_some_and(|form_guard| form_guard.try_read(actor_ref.clone()).is_ok());

                    Ok::<_, Error>(is_form_visible.then_some(comment))
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
                            self.answer_entry_set_repository.size_entries().await?,
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

                        let forms = self
                            .active_form_repository
                            .list(None, None)
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let form = guard.try_read(system.clone())?.into_inner();

                                Ok((
                                    domain::search::models::SearchableFields::FormMetaData(
                                        domain::search::models::FormMetaData {
                                            id: form.id().to_owned(),
                                            title: form.title().to_owned(),
                                            description: form.description().to_owned(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let answer_entry_sets = self
                            .answer_entry_set_repository
                            .list_all()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                guard.try_read(system.clone()).map_err(Error::from)
                            })
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let answers = answer_entry_sets
                            .iter()
                            .flat_map(|set| set.readable_entries())
                            .flat_map(|entry| {
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

                        let comments = stream::iter(
                            self.answer_entry_set_repository
                                .list_all()
                                .await?
                                .into_iter()
                                .filter_map(|guard| guard.try_read(system.clone()).ok())
                                .flat_map(|set| set.readable_entries())
                                .collect::<Vec<_>>(),
                        )
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
