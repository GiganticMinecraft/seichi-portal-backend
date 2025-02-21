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
    pub async fn post_comment(
        &self,
        actor: &User,
        comment: Comment,
        answer_id: AnswerId,
    ) -> Result<(), Error> {
        let answer_guard = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?;

        let answer_entry_context = answer_guard
            .create_context(move |entry| {
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
                        answer_visibility: form_settings.answer_settings().visibility().to_owned(),
                    })
                }
            })
            .await?;

        let comment_context = CommentAuthorizationContext {
            related_answer_entry_guard: answer_guard,
            related_answer_entry_guard_context: answer_entry_context,
        };

        let comment_guard = AuthorizationGuardWithContext::new(comment);

        self.comment_repository
            .post_comment(answer_id, &comment_context, actor, comment_guard)
            .await
    }

    pub async fn delete_comment(&self, actor: &User, comment_id: CommentId) -> Result<(), Error> {
        let comment_guard = self
            .comment_repository
            .get_comment(comment_id)
            .await?
            .ok_or(Error::from(CommentNotFound))?
            .into_delete();

        let comment_context = comment_guard
            .create_context(move |comment| {
                let answer_id = comment.answer_id().to_owned();
                async move {
                    let answer_guard = self
                        .answer_repository
                        .get_answer(answer_id)
                        .await?
                        .ok_or(Error::from(AnswerNotFound))?;

                    let answer_context = answer_guard
                        .create_context(move |entry| {
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

                    Ok(CommentAuthorizationContext {
                        related_answer_entry_guard: answer_guard,
                        related_answer_entry_guard_context: answer_context,
                    })
                }
            })
            .await?;

        self.comment_repository
            .delete_comment(comment_context, actor, comment_guard)
            .await
    }
}
