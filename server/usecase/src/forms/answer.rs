use domain::{
    form::{
        answer::{
            models::{AnswerEntry, AnswerId, AnswerTitle, FormAnswerContent},
            service::{AnswerEntryAuthorizationContext, FormAnswerContentAuthorizationContext},
        },
        comment::service::CommentAuthorizationContext,
        models::FormId,
        service::DefaultAnswerTitleDomainService,
    },
    repository::form::{
        answer_label_repository::AnswerLabelRepository, answer_repository::AnswerRepository,
        comment_repository::CommentRepository, form_repository::FormRepository,
        question_repository::QuestionRepository,
    },
    types::authorization_guard_with_context::AuthorizationGuardWithContext,
    user::models::User,
};
use errors::{
    usecase::UseCaseError::{AnswerNotFound, FormNotFound},
    Error,
};
use futures::{stream, try_join, StreamExt};

use crate::dto::AnswerDto;

pub struct AnswerUseCase<
    'a,
    AnswerRepo: AnswerRepository,
    FormRepo: FormRepository,
    CommentRepo: CommentRepository,
    AnswerLabelRepo: AnswerLabelRepository,
    QuestionRepo: QuestionRepository,
> {
    pub answer_repository: &'a AnswerRepo,
    pub form_repository: &'a FormRepo,
    pub comment_repository: &'a CommentRepo,
    pub answer_label_repository: &'a AnswerLabelRepo,
    pub question_repository: &'a QuestionRepo,
}

impl<
        R1: AnswerRepository,
        R2: FormRepository,
        R3: CommentRepository,
        R4: AnswerLabelRepository,
        R5: QuestionRepository,
    > AnswerUseCase<'_, R1, R2, R3, R4, R5>
{
    pub async fn post_answers(
        &self,
        user: User,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let form_service = DefaultAnswerTitleDomainService {
            form_repo: self.form_repository,
            question_repo: self.question_repository,
            answer_repo: self.answer_repository,
        };

        let (form, title) = try_join!(
            self.form_repository.get(form_id),
            form_service.to_answer_title(&user, form_id, answers.as_slice())
        )?;

        let form = form.ok_or(Error::from(FormNotFound))?;

        let form_settings = form.try_read(&user)?.settings();

        let answer_entry = AnswerEntry::new(user.to_owned(), form_id, title);
        let context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
        };

        let guard = AuthorizationGuardWithContext::new(answer_entry);

        self.answer_repository
            .post_answer(&context, guard, answers, &user)
            .await
    }

    pub async fn get_answers(&self, answer_id: AnswerId, user: &User) -> Result<AnswerDto, Error> {
        if let Some(form_answer_guard) = self.answer_repository.get_answer(answer_id).await? {
            let context = form_answer_guard
                .create_context(move |entry| {
                    let form_id = entry.form_id().to_owned();

                    async move {
                        let guard = self
                            .form_repository
                            .get(form_id)
                            .await?
                            .ok_or(FormNotFound)?;

                        let form = guard.try_read(user)?;
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

            let form_answer_content_context = FormAnswerContentAuthorizationContext {
                answer_entry_authorization_context: &context,
                answer_entry: &form_answer_guard,
            };

            let fetch_contents = self.answer_repository.get_answer_contents(answer_id);
            let fetch_labels = self
                .answer_label_repository
                .get_labels_for_answers_by_answer_id(answer_id);
            let fetch_comments = self.comment_repository.get_comments(answer_id);

            let (contents, labels, comments) =
                try_join!(fetch_contents, fetch_labels, fetch_comments)?;

            let contents = contents
                .into_iter()
                .map(|content| content.try_into_read(user, &form_answer_content_context))
                .collect::<Result<Vec<_>, _>>()?;

            let comment_authorization_context = CommentAuthorizationContext {
                related_answer_entry_guard: form_answer_guard,
                related_answer_entry_guard_context: context,
            };

            let comments = comments
                .into_iter()
                .map(|comment| comment.try_into_read(user, &comment_authorization_context))
                .collect::<Result<Vec<_>, _>>()?;

            let form_answer = comment_authorization_context
                .related_answer_entry_guard
                .try_into_read(
                    user,
                    &comment_authorization_context.related_answer_entry_guard_context,
                )?;

            Ok(AnswerDto {
                form_answer,
                contents,
                labels,
                comments,
            })
        } else {
            Err(Error::from(AnswerNotFound))
        }
    }

    pub async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
        actor: &User,
    ) -> Result<Vec<AnswerDto>, Error> {
        let form = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;

        let form_settings = form.try_read(actor)?.settings();

        let context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
        };

        stream::iter(
            self.answer_repository
                .get_answers_by_form_id(form_id)
                .await?,
        )
        .then(|form_answer_guard| async {
            let form_answer = form_answer_guard.try_read(actor, &context)?;

            let fetch_contents = self
                .answer_repository
                .get_answer_contents(*form_answer.id());
            let fetch_labels = self
                .answer_label_repository
                .get_labels_for_answers_by_answer_id(*form_answer.id());
            let fetch_comments = self.comment_repository.get_comments(*form_answer.id());

            let (contents, labels, comments) =
                try_join!(fetch_contents, fetch_labels, fetch_comments)?;

            let form_answer_content_context = FormAnswerContentAuthorizationContext {
                answer_entry_authorization_context: &context,
                answer_entry: &form_answer_guard,
            };

            let contents = contents
                .into_iter()
                .map(|content| content.try_into_read(actor, &form_answer_content_context))
                .collect::<Result<Vec<_>, _>>()?;

            let comment_authorization_context = CommentAuthorizationContext {
                related_answer_entry_guard: form_answer_guard,
                related_answer_entry_guard_context: AnswerEntryAuthorizationContext {
                    form_visibility: form_settings.visibility().to_owned(),
                    response_period: form_settings.answer_settings().response_period().to_owned(),
                    answer_visibility: form_settings.answer_settings().visibility().to_owned(),
                },
            };

            let comments = comments
                .into_iter()
                .map(|comment| comment.try_into_read(actor, &comment_authorization_context))
                .collect::<Result<Vec<_>, _>>()?;

            let form_answer = comment_authorization_context
                .related_answer_entry_guard
                .try_into_read(
                    actor,
                    &comment_authorization_context.related_answer_entry_guard_context,
                )?;

            Ok(AnswerDto {
                form_answer,
                contents,
                labels,
                comments,
            })
        })
        .collect::<Vec<Result<AnswerDto, Error>>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
    }

    pub async fn get_all_answers(&self, user: &User) -> Result<Vec<AnswerDto>, Error> {
        stream::iter(self.answer_repository.get_all_answers().await?)
            .then(|form_answer_guard| async move {
                let context = form_answer_guard
                    .create_context(move |entry| {
                        let form_id = entry.form_id().to_owned();

                        async move {
                            let guard = self
                                .form_repository
                                .get(form_id)
                                .await?
                                .ok_or(FormNotFound)?;

                            let form = guard.try_read(user)?;
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

                let form_answer_content_context = FormAnswerContentAuthorizationContext {
                    answer_entry_authorization_context: &context,
                    answer_entry: &form_answer_guard,
                };

                let form_answer = form_answer_guard.try_read(user, &context)?;
                let fetch_contents = self
                    .answer_repository
                    .get_answer_contents(*form_answer.id());
                let fetch_labels = self
                    .answer_label_repository
                    .get_labels_for_answers_by_answer_id(*form_answer.id());
                let fetch_comments = self.comment_repository.get_comments(*form_answer.id());

                let (contents, labels, comments) =
                    try_join!(fetch_contents, fetch_labels, fetch_comments)?;

                let contents = contents
                    .into_iter()
                    .map(|content| content.try_into_read(user, &form_answer_content_context))
                    .collect::<Result<Vec<_>, _>>()?;

                let comment_authorization_context = CommentAuthorizationContext {
                    related_answer_entry_guard: form_answer_guard,
                    related_answer_entry_guard_context: context,
                };

                let comments = comments
                    .into_iter()
                    .map(|comment| comment.try_into_read(user, &comment_authorization_context))
                    .collect::<Result<Vec<_>, _>>()?;

                let form_answer = comment_authorization_context
                    .related_answer_entry_guard
                    .try_into_read(
                        user,
                        &comment_authorization_context.related_answer_entry_guard_context,
                    )?;

                Ok(AnswerDto {
                    form_answer,
                    contents,
                    labels,
                    comments,
                })
            })
            .collect::<Vec<Result<AnswerDto, Error>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
    }

    pub async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        actor: &User,
        title: Option<AnswerTitle>,
    ) -> Result<(), Error> {
        match title {
            Some(title) => {
                let answer_entry = self
                    .answer_repository
                    .get_answer(answer_id)
                    .await?
                    .ok_or(Error::from(AnswerNotFound))?
                    .into_update()
                    .map(|entry| entry.with_title(title));

                let context = answer_entry
                    .create_context(|entry| {
                        let form_id = entry.form_id().to_owned();

                        async move {
                            let guard = self
                                .form_repository
                                .get(form_id)
                                .await?
                                .ok_or(FormNotFound)?;

                            let form = guard.try_read(actor)?;
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

                self.answer_repository
                    .update_answer_entry(actor, &context, answer_entry)
                    .await
            }
            None => Ok(()),
        }
    }
}
