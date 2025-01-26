use domain::{
    form::{
        answer::{models::AnswerId, service::AnswerEntryAuthorizationContext},
        comment::models::{Comment, CommentId},
        models::Visibility::PUBLIC,
    },
    repository::form::{
        answer_repository::AnswerRepository, comment_repository::CommentRepository,
        form_repository::FormRepository,
    },
    user::models::{
        Role::{Administrator, StandardUser},
        User,
    },
};
use errors::{
    usecase::UseCaseError::{AnswerNotFound, DoNotHavePermissionToPostFormComment, FormNotFound},
    Error,
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
        // TODO: ドメイン知識が UseCase に紛れ込んでいる。
        //      Comment に対して AuthorizationGuard を実装する必要がある
        let can_post_comment = match comment.commented_by().role {
            Administrator => true,
            StandardUser => {
                let answer = self
                    .answer_repository
                    .get_answer(answer_id)
                    .await?
                    .ok_or(Error::from(AnswerNotFound))?
                    .try_into_read_with_context_fn(actor, move |entry| {
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

                let form = self
                    .form_repository
                    .get(*answer.form_id())
                    .await?
                    .ok_or(FormNotFound)?
                    .try_into_read(actor)?;

                *form.settings().visibility() == PUBLIC
            }
        };

        if can_post_comment {
            self.comment_repository
                .post_comment(answer_id, &comment)
                .await
        } else {
            Err(Error::from(DoNotHavePermissionToPostFormComment))
        }
    }

    pub async fn delete_comment(&self, comment_id: CommentId) -> Result<(), Error> {
        self.comment_repository.delete_comment(comment_id).await
    }
}
