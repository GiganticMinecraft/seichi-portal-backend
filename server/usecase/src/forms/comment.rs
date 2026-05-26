use domain::form::comment::models::CommentContent;
use domain::form::models::FormId;
use domain::{
    form::{
        answer::models::AnswerId,
        answer_entry_set::models::AnswerEntrySet,
        comment::models::{Comment, CommentId},
    },
    repository::form::{
        active_form_repository::ActiveFormRepository,
        answer_entry_set_repository::AnswerEntrySetRepository,
    },
    repository::user_repository::UserRepository,
    types::{authorization_guard::AuthorizationGuard, authorization_guard_with_context::Read},
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
> {
    pub active_form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_set_repository: &'a AnswerEntrySetRepo,
}

impl<R1: ActiveFormRepository, R2: UserRepository, R3: AnswerEntrySetRepository>
    CommentUseCase<'_, R1, R2, R3>
{
    async fn read_answer_entry_set_guard(
        &self,
        actor: &Actor,
        form_id: FormId,
    ) -> Result<AuthorizationGuard<AnswerEntrySet, Read>, Error> {
        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(actor)?;

        let set_guard = self
            .answer_entry_set_repository
            .get(*form.answer_entry_set_id())
            .await?
            .ok_or(FormNotFound)?;

        set_guard.try_read(actor)?;
        Ok(set_guard)
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
        let set_guard = self
            .read_answer_entry_set_guard(&actor_user, form_id)
            .await?;
        let answer_entry_set = set_guard.try_read(&actor_user)?;

        let entry = answer_entry_set
            .read_entry(answer_id, &actor_user)
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?;

        let comments = entry.comments().to_vec();

        self.build_comments_with_authors(actor, comments).await
    }

    pub async fn post_comment(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        answer_id: AnswerId,
        comment: Comment,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        let set_guard = self
            .read_answer_entry_set_guard(&actor_user, form_id)
            .await?;

        if !comment.can_create_on_entry(&actor_user) {
            return Err(Error::from(DomainError::Forbidden));
        }

        self.answer_entry_set_repository
            .add_comment(&set_guard, answer_id, &comment, &actor_user)
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
        let actor_user = Actor::from(actor.clone());
        let set_guard = self
            .read_answer_entry_set_guard(&actor_user, form_id)
            .await?;
        let answer_entry_set = set_guard.try_read(&actor_user)?;

        let entry = answer_entry_set
            .read_entry(answer_id, &actor_user)
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?;

        let current_comment = entry.find_comment(comment_id).ok_or(CommentNotFound)?;

        if !current_comment.can_update_on_entry(&actor_user) {
            return Err(Error::from(DomainError::Forbidden));
        }

        if let Some(content) = content {
            let updated = current_comment.clone().with_updated_content(content);
            self.answer_entry_set_repository
                .update_comment(&set_guard, answer_id, &updated, &actor_user)
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
        let actor_user = Actor::from(actor.clone());
        let set_guard = self
            .read_answer_entry_set_guard(&actor_user, form_id)
            .await?;
        let answer_entry_set = set_guard.try_read(&actor_user)?;

        let entry = answer_entry_set
            .read_entry(answer_id, &actor_user)
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?;

        let comment = entry.find_comment(comment_id).ok_or(CommentNotFound)?;

        if !comment.can_delete_on_entry(&actor_user) {
            return Err(Error::from(DomainError::Forbidden));
        }

        self.answer_entry_set_repository
            .delete_comment(&set_guard, answer_id, comment_id, &actor_user)
            .await
    }
}
