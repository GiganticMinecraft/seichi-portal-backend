use domain::form::comment::models::CommentContent;
use domain::form::models::{ActiveForm, FormId};
use domain::{
    form::{
        answer::models::AnswerId,
        answer_entry_set::models::AnswerEntrySet,
        comment::models::{Comment, CommentId},
    },
    repository::form::{
        active_form_repository::ActiveFormRepository,
        answer_entry_set_repository::AnswerEntrySetRepository,
        comment_repository::CommentRepository,
    },
    repository::user_repository::UserRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Delete, Read, Update},
    user::models::{ActiveUser, Actor},
};
use errors::{
    Error,
    domain::DomainError,
    usecase::UseCaseError::{AnswerNotFound, CommentNotFound, FormNotFound, UserNotFound},
};

use crate::{models::CommentWithAuthor, user_reference_resolver::resolve_user_references};

pub struct CommentUseCase<
    'a,
    FormRepo: ActiveFormRepository,
    UserRepo: UserRepository,
    AnswerEntrySetRepo: AnswerEntrySetRepository,
    CommentRepo: CommentRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_set_repository: &'a AnswerEntrySetRepo,
    pub comment_repository: &'a CommentRepo,
}

impl<
    R1: ActiveFormRepository,
    R2: UserRepository,
    R3: AnswerEntrySetRepository,
    R4: CommentRepository,
> CommentUseCase<'_, R1, R2, R3, R4>
{
    async fn read_form_and_entry_set(
        &self,
        actor: &Actor,
        form_id: FormId,
    ) -> Result<(Allowed<ActiveForm, Read>, Allowed<AnswerEntrySet, Read>), Error> {
        let form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?
            .try_read(actor.clone())?;

        let answer_entry_set = self
            .answer_entry_set_repository
            .get_read(&form)
            .await?
            .ok_or(FormNotFound)?;

        Ok((form, answer_entry_set))
    }

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
        let actor_user = Actor::from(actor.clone());
        let (form, answer_entry_set) = self.read_form_and_entry_set(&actor_user, form_id).await?;

        let entry = form
            .read_entry(&answer_entry_set, answer_id)
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?;

        let comments = self
            .comment_repository
            .find_by_answer(&entry)
            .await?
            .into_iter()
            .map(|comment| comment.into_inner())
            .collect::<Vec<_>>();

        self.build_comments_with_authors(actor, comments).await
    }

    pub async fn post_comment(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        answer_id: AnswerId,
        content: CommentContent,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        let (form, answer_entry_set) = self.read_form_and_entry_set(&actor_user, form_id).await?;

        let comment = form
            .create_comment(&answer_entry_set, answer_id, content)?
            .try_create(actor_user.clone())?;

        self.comment_repository.create(comment).await
    }

    pub async fn update_comment(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        answer_id: AnswerId,
        comment_id: CommentId,
        content: Option<CommentContent>,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        let (form, answer_entry_set) = self.read_form_and_entry_set(&actor_user, form_id).await?;
        let entry = form
            .read_entry(&answer_entry_set, answer_id)
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?;
        let current_comment = self
            .comment_repository
            .find_by_answer(&entry)
            .await?
            .into_iter()
            .find(|comment| *comment.value().comment_id() == comment_id)
            .ok_or(Error::from(CommentNotFound))?;

        if let Some(content) = content {
            let updated = AuthorizationGuard::<_, Update>::from(
                current_comment.into_inner().with_updated_content(content),
            )
            .try_update(actor_user.clone())?;
            self.comment_repository.update(updated).await?;
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
        let actor_user = Actor::from(actor.clone());
        let (form, answer_entry_set) = self.read_form_and_entry_set(&actor_user, form_id).await?;
        let entry = form
            .read_entry(&answer_entry_set, answer_id)
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?;
        let comment = self
            .comment_repository
            .find_by_answer(&entry)
            .await?
            .into_iter()
            .find(|comment| *comment.value().comment_id() == comment_id)
            .ok_or(Error::from(CommentNotFound))?;

        let comment =
            AuthorizationGuard::<_, Delete>::from(comment.into_inner()).try_delete(actor_user)?;

        self.comment_repository.delete(comment).await
    }
}
