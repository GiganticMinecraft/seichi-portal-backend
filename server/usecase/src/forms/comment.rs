use domain::form::comment::models::CommentContent;
use domain::form::models::FormId;
use domain::{
    form::{
        answer::{models::AnswerId, service::AnswerEntryAuthorizationContext},
        comment::{
            models::{Comment, CommentId},
            service::CommentAuthorizationContext,
        },
    },
    repository::form::{
        active_form_repository::ActiveFormRepository, answer_repository::AnswerRepository,
        comment_repository::CommentRepository,
    },
    repository::user_repository::UserRepository,
    types::authorization_guard_with_context::AuthorizationGuardWithContext,
    user::models::{ActiveUser, User},
};
use errors::{
    Error,
    usecase::UseCaseError::{AnswerNotFound, CommentNotFound, FormNotFound, UserNotFound},
};

use crate::{models::CommentWithAuthor, user_reference_resolver::resolve_user_references};

pub struct CommentUseCase<
    'a,
    CommentRepo: CommentRepository,
    AnswerRepo: AnswerRepository,
    FormRepo: ActiveFormRepository,
    UserRepo: UserRepository,
> {
    pub comment_repository: &'a CommentRepo,
    pub answer_repository: &'a AnswerRepo,
    pub active_form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
}

impl<R1: CommentRepository, R2: AnswerRepository, R3: ActiveFormRepository, R4: UserRepository>
    CommentUseCase<'_, R1, R2, R3, R4>
{
    async fn build_comments_with_authors(
        &self,
        actor: &ActiveUser,
        comments: Vec<Comment>,
    ) -> Result<Vec<CommentWithAuthor>, Error> {
        let user_ids = comments.iter().map(|c| *c.commented_by()).collect();
        let users = resolve_user_references(self.user_repository, actor, user_ids).await?;

        comments
            .into_iter()
            .map(|comment| {
                let commented_by = users
                    .get(comment.commented_by())
                    .cloned()
                    .ok_or(Error::from(UserNotFound))?;
                Ok(CommentWithAuthor {
                    comment,
                    commented_by,
                })
            })
            .collect()
    }

    pub async fn get_comments(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Vec<CommentWithAuthor>, Error> {
        let actor_user = User::from(actor.clone());
        let answer_guard = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?;

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        let form = form_guard.try_read(&actor_user)?;
        let form_settings = form.settings();

        let answer_entry_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
            allow_temporary_answers: form_settings.allow_temporary_answers(),
        };

        let comment_context = CommentAuthorizationContext {
            related_answer_entry_guard: answer_guard,
            related_answer_entry_guard_context: answer_entry_context,
        };

        let comments = self.comment_repository.get_comments(answer_id).await?;

        let comments = comments
            .into_iter()
            .map(|guard| guard.try_into_read(&actor_user, &comment_context))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Error::from)?;

        self.build_comments_with_authors(actor, comments).await
    }

    pub async fn post_comment(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        answer_id: AnswerId,
        comment: Comment,
    ) -> Result<(), Error> {
        let actor_user = User::from(actor.clone());
        let answer_guard = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?;

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        let form = form_guard.try_read(&actor_user)?;
        let form_settings = form.settings();

        let answer_entry_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
            allow_temporary_answers: form_settings.allow_temporary_answers(),
        };

        let comment_context = CommentAuthorizationContext {
            related_answer_entry_guard: answer_guard,
            related_answer_entry_guard_context: answer_entry_context,
        };

        let comment_guard = AuthorizationGuardWithContext::new(comment);

        self.comment_repository
            .create_comment(answer_id, &comment_context, actor, comment_guard)
            .await
    }

    pub async fn update_comment(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        answer_id: AnswerId,
        comment_id: CommentId,
        content: Option<CommentContent>,
    ) -> Result<(), Error> {
        let actor_user = User::from(actor.clone());
        let answer_guard = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?;

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        let form = form_guard.try_read(&actor_user)?;
        let form_settings = form.settings();

        let answer_entry_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
            allow_temporary_answers: form_settings.allow_temporary_answers(),
        };

        let comment_context = CommentAuthorizationContext {
            related_answer_entry_guard: answer_guard,
            related_answer_entry_guard_context: answer_entry_context,
        };

        let current_comment_guard = self
            .comment_repository
            .get_comment(comment_id)
            .await?
            .ok_or(CommentNotFound)?;

        if let Some(content) = content {
            let updated_comment = current_comment_guard
                .into_update()
                .map(|comment| comment.with_updated_content(content));

            self.comment_repository
                .update_comment(answer_id, &comment_context, actor, updated_comment)
                .await?;
        }

        Ok(())
    }

    pub async fn delete_comment(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        answer_id: AnswerId,
        comment_id: CommentId,
    ) -> Result<(), Error> {
        let actor_user = User::from(actor.clone());
        let comment_guard = self
            .comment_repository
            .get_comment(comment_id)
            .await?
            .ok_or(Error::from(CommentNotFound))?
            .into_delete();

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        let form = form_guard.try_read(&actor_user)?;
        let form_settings = form.settings();

        let answer_entry_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
            allow_temporary_answers: form_settings.allow_temporary_answers(),
        };

        let answer_guard = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?;

        let comment_context = CommentAuthorizationContext {
            related_answer_entry_guard: answer_guard,
            related_answer_entry_guard_context: answer_entry_context,
        };

        self.comment_repository
            .delete_comment(comment_context, actor, comment_guard)
            .await
    }
}
