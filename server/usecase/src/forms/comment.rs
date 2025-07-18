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
        answer_repository::AnswerRepository, comment_repository::CommentRepository,
        form_repository::FormRepository,
    },
    types::authorization_guard_with_context::AuthorizationGuardWithContext,
    user::models::User,
};
use errors::{
    Error,
    usecase::UseCaseError::{AnswerNotFound, CommentNotFound, FormNotFound},
};

pub struct CommentUseCase<
    'a,
    CommentRepo: CommentRepository,
    AnswerRepo: AnswerRepository,
    FormRepo: FormRepository,
> {
    pub comment_repository: &'a CommentRepo,
    pub answer_repository: &'a AnswerRepo,
    pub form_repository: &'a FormRepo,
}

impl<R1: CommentRepository, R2: AnswerRepository, R3: FormRepository>
    CommentUseCase<'_, R1, R2, R3>
{
    pub async fn get_comments(
        &self,
        actor: &User,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Vec<Comment>, Error> {
        let answer_guard = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?;

        let form_guard = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        let form = form_guard.try_read(actor)?;
        let form_settings = form.settings();

        let answer_entry_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
        };

        let comment_context = CommentAuthorizationContext {
            related_answer_entry_guard: answer_guard,
            related_answer_entry_guard_context: answer_entry_context,
        };

        let comments = self.comment_repository.get_comments(answer_id).await?;

        comments
            .into_iter()
            .map(|guard| guard.try_into_read(actor, &comment_context))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn post_comment(
        &self,
        actor: &User,
        form_id: FormId,
        answer_id: AnswerId,
        comment: Comment,
    ) -> Result<(), Error> {
        let answer_guard = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?;

        let form_guard = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        let form = form_guard.try_read(actor)?;
        let form_settings = form.settings();

        let answer_entry_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
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
        actor: &User,
        form_id: FormId,
        answer_id: AnswerId,
        comment_id: CommentId,
        content: Option<CommentContent>,
    ) -> Result<(), Error> {
        let answer_guard = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?;

        let form_guard = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        let form = form_guard.try_read(actor)?;
        let form_settings = form.settings();

        let answer_entry_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
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
        actor: &User,
        form_id: FormId,
        answer_id: AnswerId,
        comment_id: CommentId,
    ) -> Result<(), Error> {
        let comment_guard = self
            .comment_repository
            .get_comment(comment_id)
            .await?
            .ok_or(Error::from(CommentNotFound))?
            .into_delete();

        let form_guard = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        let form = form_guard.try_read(actor)?;
        let form_settings = form.settings();

        let answer_entry_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
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
