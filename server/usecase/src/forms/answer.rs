use domain::{
    form::{
        answer::{
            models::{
                AnswerAuthor, AnswerEntry, AnswerId, AnswerTitle, FormAnswerContent,
                PostedAnswerContents,
            },
            service::AnswerEntryAuthorizationContext,
        },
        comment::service::CommentAuthorizationContext,
        models::FormId,
        service::DefaultAnswerTitleDomainService,
    },
    repository::form::{
        active_form_repository::ActiveFormRepository,
        answer_label_repository::AnswerLabelRepository, answer_repository::AnswerRepository,
        comment_repository::CommentRepository,
    },
    repository::user_repository::UserRepository,
    types::authorization_guard_with_context::AuthorizationGuardWithContext,
    user::models::{TemporaryUser, User},
};
use errors::{
    Error,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound, UserNotFound},
};
use futures::{StreamExt, stream, try_join};

use crate::{
    models::{AnswerAuthorDetails, AnswerDetails, CommentWithAuthor},
    user_reference_resolver::resolve_user_references,
};

pub struct AnswerUseCase<
    'a,
    AnswerRepo: AnswerRepository,
    FormRepo: ActiveFormRepository,
    CommentRepo: CommentRepository,
    AnswerLabelRepo: AnswerLabelRepository,
    UserRepo: UserRepository,
> {
    pub answer_repository: &'a AnswerRepo,
    pub active_form_repository: &'a FormRepo,
    pub comment_repository: &'a CommentRepo,
    pub answer_label_repository: &'a AnswerLabelRepo,
    pub user_repository: &'a UserRepo,
}

impl<
    R1: AnswerRepository,
    R2: ActiveFormRepository,
    R3: CommentRepository,
    R4: AnswerLabelRepository,
    R5: UserRepository,
> AnswerUseCase<'_, R1, R2, R3, R4, R5>
{
    async fn build_answer_details(
        &self,
        actor: &User,
        form_answer: AnswerEntry,
        labels: Vec<domain::form::answer::models::AnswerLabel>,
        comments: Vec<domain::form::comment::models::Comment>,
    ) -> Result<AnswerDetails, Error> {
        let user_ids = form_answer
            .author()
            .authenticated_user_id()
            .into_iter()
            .chain(comments.iter().map(|comment| *comment.commented_by()))
            .collect();

        let users = resolve_user_references(self.user_repository, actor, user_ids).await?;

        let author = match form_answer.author() {
            AnswerAuthor::AuthenticatedUser(user_id) => AnswerAuthorDetails::AuthenticatedUser(
                users
                    .get(user_id)
                    .cloned()
                    .ok_or(Error::from(UserNotFound))?,
            ),
            AnswerAuthor::TemporaryUser(temporary_user) => {
                AnswerAuthorDetails::TemporaryUser(temporary_user.clone())
            }
        };

        let comments = comments
            .into_iter()
            .map(|comment| {
                let commented_by = users
                    .get(comment.commented_by())
                    .cloned()
                    .ok_or(Error::from(UserNotFound))?;
                Ok::<_, Error>(CommentWithAuthor {
                    comment,
                    commented_by,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(AnswerDetails {
            form_answer,
            author,
            labels,
            comments,
        })
    }

    pub async fn post_answers(
        &self,
        user: User,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let form = self.active_form_repository.get(form_id).await?;

        let form = form
            .ok_or(Error::from(FormNotFound))?
            .try_into_read(&user)?;
        let questions = form.questions().as_slice().to_vec();
        let posted_answers = PostedAnswerContents::try_new(&questions, answers)?;
        let title = DefaultAnswerTitleDomainService::<R2>::to_answer_title_from_questions(
            form.settings()
                .answer_settings()
                .default_answer_title()
                .to_owned(),
            &questions,
            &posted_answers,
            user.name.as_str(),
        )?;

        let form_settings = form.settings();

        let answer_entry = AnswerEntry::new(
            AnswerAuthor::AuthenticatedUser(user.id),
            form_id,
            title,
            posted_answers,
        );
        let context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
        };

        let guard = AuthorizationGuardWithContext::new(answer_entry);

        self.answer_repository
            .post_answer(&context, guard, &user)
            .await
    }

    pub async fn post_temporary_answers(
        &self,
        temporary_user: TemporaryUser,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let form = self.active_form_repository.get(form_id).await?;

        let form_guard = form.ok_or(Error::from(FormNotFound))?;
        let form = unsafe { form_guard.into_read_unchecked() };
        let questions = form.questions().as_slice().to_vec();
        let posted_answers = PostedAnswerContents::try_new(&questions, answers)?;
        let title = DefaultAnswerTitleDomainService::<R2>::to_answer_title_from_questions(
            form.settings()
                .answer_settings()
                .default_answer_title()
                .to_owned(),
            &questions,
            &posted_answers,
            temporary_user.name.as_str(),
        )?;

        let form_settings = form.settings();
        let context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
        };

        if !form_settings.allow_temporary_answers() || !context.can_create_temporary() {
            return Err(errors::domain::DomainError::Forbidden.into());
        }

        let answer_entry = AnswerEntry::new(
            AnswerAuthor::TemporaryUser(temporary_user),
            form_id,
            title,
            posted_answers,
        );

        self.answer_repository
            .post_answer_without_actor(answer_entry)
            .await
    }

    pub async fn get_answers(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
        user: &User,
    ) -> Result<AnswerDetails, Error> {
        if let Some(form_answer_guard) = self.answer_repository.get_answer(answer_id).await? {
            let form_guard = self
                .active_form_repository
                .get(form_id)
                .await?
                .ok_or(FormNotFound)?;

            let form = form_guard.try_read(user)?;
            let form_settings = form.settings();

            let context = AnswerEntryAuthorizationContext {
                form_visibility: form_settings.visibility().to_owned(),
                response_period: form_settings.answer_settings().response_period().to_owned(),
                answer_visibility: form_settings.answer_settings().visibility().to_owned(),
            };

            let fetch_labels = self
                .answer_label_repository
                .get_labels_for_answers_by_answer_id(answer_id);
            let fetch_comments = self.comment_repository.get_comments(answer_id);

            let (labels, comments) = try_join!(fetch_labels, fetch_comments)?;

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

            let labels = labels
                .into_iter()
                .map(|label| label.try_into_read(user))
                .collect::<Result<Vec<_>, _>>()?;

            self.build_answer_details(user, form_answer, labels, comments)
                .await
        } else {
            Err(Error::from(AnswerNotFound))
        }
    }

    pub async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
        actor: &User,
    ) -> Result<Vec<AnswerDetails>, Error> {
        let form = self
            .active_form_repository
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

            let fetch_labels = self
                .answer_label_repository
                .get_labels_for_answers_by_answer_id(*form_answer.id());
            let fetch_comments = self.comment_repository.get_comments(*form_answer.id());

            let (labels, comments) = try_join!(fetch_labels, fetch_comments)?;

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

            let labels = labels
                .into_iter()
                .map(|label| label.try_into_read(actor))
                .collect::<Result<Vec<_>, _>>()?;

            self.build_answer_details(actor, form_answer, labels, comments)
                .await
        })
        .collect::<Vec<Result<AnswerDetails, Error>>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
    }

    pub async fn get_all_answers(&self, user: &User) -> Result<Vec<AnswerDetails>, Error> {
        stream::iter(self.answer_repository.get_all_answers().await?)
            .then(|form_answer_guard| async move {
                let context = form_answer_guard
                    .create_context(move |entry| {
                        let form_id = entry.form_id().to_owned();

                        async move {
                            let guard = self
                                .active_form_repository
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

                let form_answer = form_answer_guard.try_read(user, &context)?;
                let fetch_labels = self
                    .answer_label_repository
                    .get_labels_for_answers_by_answer_id(*form_answer.id());
                let fetch_comments = self.comment_repository.get_comments(*form_answer.id());

                let (labels, comments) = try_join!(fetch_labels, fetch_comments)?;

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

                let labels = labels
                    .into_iter()
                    .map(|label| label.try_into_read(user))
                    .collect::<Result<Vec<_>, _>>()?;

                self.build_answer_details(user, form_answer, labels, comments)
                    .await
            })
            .collect::<Vec<Result<AnswerDetails, Error>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
    }

    pub async fn update_answer_meta(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
        actor: &User,
        title: Option<AnswerTitle>,
    ) -> Result<AnswerDetails, Error> {
        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;

        let form = form_guard.try_read(actor)?;
        let form_settings = form.settings();

        let context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
        };

        if let Some(title) = title {
            let answer_entry = self
                .answer_repository
                .get_answer(answer_id)
                .await?
                .ok_or(Error::from(AnswerNotFound))?
                .into_update()
                .map(|entry| entry.with_title(title));

            self.answer_repository
                .update_answer_entry(actor, &context, answer_entry)
                .await?;
        }

        let answer_guard = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?;

        let fetch_labels = self
            .answer_label_repository
            .get_labels_for_answers_by_answer_id(answer_id);
        let fetch_comments = self.comment_repository.get_comments(answer_id);

        let (labels, comments) = try_join!(fetch_labels, fetch_comments)?;

        let labels = labels
            .into_iter()
            .map(|label| label.try_into_read(actor))
            .collect::<Result<Vec<_>, _>>()?;

        let comment_authorization_context = CommentAuthorizationContext {
            related_answer_entry_guard: answer_guard,
            related_answer_entry_guard_context: context,
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

        self.build_answer_details(actor, form_answer, labels, comments)
            .await
    }
}
