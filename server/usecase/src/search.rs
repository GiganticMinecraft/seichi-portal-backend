use crate::models::CrossSearchOutput;
use domain::repository::form::answer_entry_set_repository::AnswerEntrySetRepository;
use domain::repository::form::answer_label_repository::AnswerLabelRepository;
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
use futures::try_join;
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
> {
    pub search_repository: &'a SearchRepo,
    pub active_form_repository: &'a FormRepo,
    pub form_answer_label_repository: &'a FormAnswerLabelRepo,
    pub form_label_repository: &'a FormLabelRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_set_repository: &'a AnswerEntrySetRepo,
}

impl<
    R1: SearchRepository,
    R2: ActiveFormRepository,
    R3: AnswerLabelRepository,
    R4: FormLabelRepository,
    R5: UserRepository,
    R6: AnswerEntrySetRepository,
> SearchUseCase<'_, R1, R2, R3, R4, R5, R6>
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

        let forms = forms
            .into_iter()
            .flat_map(|guard| guard.try_into_read(&actor))
            .collect::<Vec<_>>();

        let users = users
            .into_iter()
            .flat_map(|guard| guard.try_into_read(&actor))
            .collect::<Vec<_>>();

        let label_for_forms = label_for_forms
            .into_iter()
            .flat_map(|guard| guard.try_into_read(&actor))
            .collect::<Vec<_>>();

        let label_for_answers = label_for_answers
            .into_iter()
            .flat_map(|guard| guard.try_into_read(&actor))
            .collect::<Vec<_>>();

        let all_sets = self.answer_entry_set_repository.list_all().await?;

        let mut visible_answers = Vec::new();
        'entries: for entry in answers {
            for set_guard in &all_sets {
                let answer_entry_set = match set_guard.try_read(&actor) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let Ok(answer) = answer_entry_set.read_entry(*entry.id(), &actor) else {
                    continue;
                };
                let form_id = *answer_entry_set.form_id();
                let form_guard = match self.active_form_repository.get(form_id).await? {
                    Some(g) => g,
                    None => continue,
                };
                if form_guard.try_read(&actor).is_ok() {
                    visible_answers.push(answer.clone());
                }
                continue 'entries;
            }
        }

        let mut visible_comments = Vec::new();
        for comment in comments {
            let answer_id = *comment.answer_id();
            let mut found_set: Option<_> = None;

            for set_guard in &all_sets {
                let answer_entry_set = match set_guard.try_read(&actor) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if answer_entry_set.read_entry(answer_id, &actor).is_ok() {
                    found_set = Some(answer_entry_set);
                    break;
                }
            }

            let answer_entry_set = match found_set {
                Some(s) => s,
                None => continue,
            };
            let form_id = *answer_entry_set.form_id();
            let form_guard = match self.active_form_repository.get(form_id).await? {
                Some(g) => g,
                None => continue,
            };
            match form_guard.try_read(&actor) {
                Ok(_) => {}
                Err(_) => continue,
            }
            visible_comments.push(comment);
        }

        Ok(CrossSearchOutput {
            forms,
            users,
            label_for_forms,
            label_for_answers,
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
                            self.answer_entry_set_repository.size_comments().await?,
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
                                let form = guard.try_into_read(&system)?;

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

                        let answers = self
                            .answer_entry_set_repository
                            .list_all()
                            .await?
                            .into_iter()
                            .map(|guard| guard.try_into_read(&system).map_err(Error::from))
                            .collect::<Result<Vec<_>, errors::Error>>()?
                            .into_iter()
                            .flat_map(|set| {
                                set.entries_as_system(&system)
                                    .into_iter()
                                    .flat_map(|entries| entries.iter())
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
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>();

                        let comments = self
                            .answer_entry_set_repository
                            .get_all_comments()
                            .await?
                            .into_iter()
                            .map(|comment| {
                                (
                                    domain::search::models::SearchableFields::FormAnswerComments(
                                        domain::search::models::FormAnswerComments {
                                            id: comment.comment_id().to_owned(),
                                            answer_id: comment.answer_id().to_owned(),
                                            content: comment.content().to_owned().into_inner().into_inner(),
                                        },
                                    ),
                                    Operation::Update,
                                )
                            })
                            .collect::<Vec<_>>();

                        let labels_for_forms = self
                            .form_label_repository
                            .fetch_labels()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let label = guard.try_into_read(&system)?;

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
                                let label = guard.try_into_read(&system)?;

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
                                let user = guard.try_into_read(&system)?;

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
