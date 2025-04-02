use crate::dto::CrossSearchDto;
use domain::repository::form::answer_label_repository::AnswerLabelRepository;
use domain::repository::form::comment_repository::CommentRepository;
use domain::repository::form::form_label_repository::FormLabelRepository;
use domain::repository::user_repository::UserRepository;
use domain::search::models::NumberOfRecords;
use domain::search::models::{NumberOfRecordsPerAggregate, Operation};
use domain::{
    form::{
        answer::service::AnswerEntryAuthorizationContext,
        comment::service::CommentAuthorizationContext,
    },
    repository::{
        form::{answer_repository::AnswerRepository, form_repository::FormRepository},
        search_repository::SearchRepository,
    },
    search::models::SearchableFieldsWithOperation,
    user::models::User,
};
use errors::{
    Error,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound},
};
use futures::{future::try_join_all, try_join};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, mpsc::Receiver};
use tokio::time;

pub struct SearchUseCase<
    'a,
    SearchRepo: SearchRepository,
    AnswerRepo: AnswerRepository,
    FormRepo: FormRepository,
    CommentRepo: CommentRepository,
    FormAnswerLabelRepo: AnswerLabelRepository,
    FormLabelRepo: FormLabelRepository,
    UserRepo: UserRepository,
> {
    pub search_repository: &'a SearchRepo,
    pub answer_repository: &'a AnswerRepo,
    pub form_repository: &'a FormRepo,
    pub comment_repository: &'a CommentRepo,
    pub form_answer_label_repository: &'a FormAnswerLabelRepo,
    pub form_label_repository: &'a FormLabelRepo,
    pub user_repository: &'a UserRepo,
}

impl<
    R1: SearchRepository,
    R2: AnswerRepository,
    R3: FormRepository,
    R4: CommentRepository,
    R5: AnswerLabelRepository,
    R6: FormLabelRepository,
    R7: UserRepository,
> SearchUseCase<'_, R1, R2, R3, R4, R5, R6, R7>
{
    pub async fn cross_search(&self, actor: &User, query: String) -> Result<CrossSearchDto, Error> {
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
            .flat_map(|guard| guard.try_into_read(actor))
            .collect::<Vec<_>>();

        let users = users
            .into_iter()
            .flat_map(|guard| guard.try_into_read(actor))
            .collect::<Vec<_>>();

        let label_for_forms = label_for_forms
            .into_iter()
            .flat_map(|guard| guard.try_into_read(actor))
            .collect::<Vec<_>>();

        let label_for_answers = label_for_answers
            .into_iter()
            .flat_map(|guard| guard.try_into_read(actor))
            .collect::<Vec<_>>();

        let answers_futs = answers
            .into_iter()
            .map(|guard| async {
                let context = guard
                    .create_context(|entry| {
                        let form_id = entry.form_id().to_owned();

                        async move {
                            let form_guard = self
                                .form_repository
                                .get(form_id)
                                .await?
                                .ok_or(Error::from(FormNotFound))?;

                            let form = form_guard.try_read(actor)?;
                            let form_settings = form.settings();

                            Ok(AnswerEntryAuthorizationContext {
                                form_visibility: form_settings.visibility().to_owned(),
                                response_period: form_settings
                                    .answer_settings()
                                    .response_period()
                                    .to_owned(),
                                answer_visibility: form_settings
                                    .answer_settings()
                                    .visibility()
                                    .to_owned(),
                            })
                        }
                    })
                    .await?;

                guard
                    .try_into_read(actor, &context)
                    .map_err(Into::<Error>::into)
            })
            .collect::<Vec<_>>();

        let answers = try_join_all(answers_futs).await?;

        let comment_futs = comments
            .into_iter()
            .map(|guard| async {
                let context = guard
                    .create_context(|comment| {
                        let answer_id = comment.answer_id().to_owned();

                        async move {
                            let answer_entry_guard = self
                                .answer_repository
                                .get_answer(answer_id)
                                .await?
                                .ok_or(Error::from(AnswerNotFound))?;

                            let answer_entry_context = answer_entry_guard
                                .create_context(|entry| {
                                    let form_id = entry.form_id().to_owned();

                                    async move {
                                        let form_guard = self
                                            .form_repository
                                            .get(form_id)
                                            .await?
                                            .ok_or(Error::from(FormNotFound))?;

                                        let form = form_guard.try_read(actor)?;
                                        let form_settings = form.settings();

                                        Ok(AnswerEntryAuthorizationContext {
                                            form_visibility: form_settings.visibility().to_owned(),
                                            response_period: form_settings
                                                .answer_settings()
                                                .response_period()
                                                .to_owned(),
                                            answer_visibility: form_settings
                                                .answer_settings()
                                                .visibility()
                                                .to_owned(),
                                        })
                                    }
                                })
                                .await?;

                            Ok(CommentAuthorizationContext {
                                related_answer_entry_guard: answer_entry_guard,
                                related_answer_entry_guard_context: answer_entry_context,
                            })
                        }
                    })
                    .await?;

                guard
                    .try_into_read(actor, &context)
                    .map_err(Into::<Error>::into)
            })
            .collect::<Vec<_>>();

        let comments = try_join_all(comment_futs).await?;

        Ok(CrossSearchDto {
            forms,
            users,
            label_for_forms,
            label_for_answers,
            answers,
            comments,
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
                        form_meta_data: NumberOfRecords(self.form_repository.size().await?),
                        real_answers: NumberOfRecords(self.answer_repository.size().await?),
                        form_answer_comments: NumberOfRecords(self.comment_repository.size().await?),
                        label_for_form_answers: NumberOfRecords(
                            self.form_answer_label_repository.size().await?,
                        ),
                        label_for_forms: NumberOfRecords(self.form_label_repository.size().await?),
                        users: NumberOfRecords(self.user_repository.size().await?),
                    };

                    let sync_rate = search_engine_records.try_into_sync_rate(&repository_records)?;

                    if sync_rate.is_out_of_sync() {
                        let forms = self
                            .form_repository
                            .list(None, None)
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let form = unsafe { guard.into_read_unchecked() };

                                (
                                    domain::search::models::SearchableFields::FormMetaData(
                                        domain::search::models::FormMetaData {
                                            id: form.id().to_owned(),
                                            title: form.title().to_owned(),
                                            description: form.description().to_owned(),
                                        },
                                    ),
                                    Operation::Update,
                                )
                            })
                            .collect::<Vec<_>>();

                        let answers = self
                            .answer_repository
                            .get_all_answers()
                            .await?
                            .into_iter()
                            .flat_map(|guard| {
                                let entry = unsafe { guard.into_read_unchecked() };

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

                        let comments = self
                            .comment_repository
                            .get_all_comments()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let comment = unsafe { guard.into_read_unchecked() };

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
                                let label = unsafe { guard.into_read_unchecked() };

                                (
                                    domain::search::models::SearchableFields::LabelForForms(
                                        domain::search::models::LabelForForms {
                                            id: label.id().to_owned(),
                                            name: label.name().to_owned().into_inner().into_inner(),
                                        },
                                    ),
                                    Operation::Update,
                                )
                            })
                            .collect::<Vec<_>>();

                        let labels_for_answers = self
                            .form_answer_label_repository
                            .get_labels_for_answers()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let label = unsafe { guard.into_read_unchecked() };

                                (
                                    domain::search::models::SearchableFields::LabelForFormAnswers(
                                        domain::search::models::LabelForFormAnswers {
                                            id: label.id().to_owned(),
                                            name: label.name().to_owned(),
                                        },
                                    ),
                                    Operation::Update,
                                )
                            })
                            .collect::<Vec<_>>();

                        let users = self
                            .user_repository
                            .fetch_all_users()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let user = unsafe { guard.into_read_unchecked() };

                                (
                                    domain::search::models::SearchableFields::Users(
                                        domain::search::models::Users {
                                            id: user.id,
                                            name: user.name,
                                        },
                                    ),
                                    Operation::Update,
                                )
                            })
                            .collect::<Vec<_>>();

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
}
