use domain::form::comment::models::CommentContent;
use domain::form::models::FormId;
use domain::{
    form::{
        answer::models::{AnswerEntry, AnswerId},
        comment::models::{Comment, CommentId},
    },
    repository::form::{
        active_form_repository::ActiveFormRepository,
        answer_entry_repository::AnswerEntryRepository, comment_repository::CommentRepository,
    },
    repository::user_repository::UserRepository,
    types::authorization_guard::{Allowed, Read},
    user::models::{ActiveUser, Actor},
};
use errors::{
    Error,
    usecase::UseCaseError::{AnswerNotFound, CommentNotFound, FormNotFound, UserNotFound},
};

use crate::{models::CommentWithAuthor, user_reference_resolver::resolve_user_references};

pub struct CommentUseCase<
    'a,
    FormRepo: ActiveFormRepository,
    UserRepo: UserRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    CommentRepo: CommentRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub comment_repository: &'a CommentRepo,
}

impl<R1: ActiveFormRepository, R2: UserRepository, R3: AnswerEntryRepository, R4: CommentRepository>
    CommentUseCase<'_, R1, R2, R3, R4>
{
    /// フォームと回答の読み取り認可を通過した [`AnswerEntry`] のガードを取得する。
    async fn read_answer_entry(
        &self,
        actor: &Actor,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Allowed<AnswerEntry, Read>, Error> {
        let form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?
            .try_read(actor.clone())?;

        self.answer_entry_repository
            .get(&form, answer_id)
            .await?
            .ok_or(AnswerNotFound)
            .map_err(Into::into)
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
        let entry = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;

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
        let entry = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;

        let comment = entry.create_comment(content)?;

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
        let entry = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;
        let current_comment = self
            .comment_repository
            .find_by_answer(&entry)
            .await?
            .into_iter()
            .find(|comment| *comment.value().comment_id() == comment_id)
            .ok_or(Error::from(CommentNotFound))?;

        if let Some(content) = content {
            let updated = entry.update_comment(current_comment.into_inner(), content)?;
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
        let entry = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;
        let comment = self
            .comment_repository
            .find_by_answer(&entry)
            .await?
            .into_iter()
            .find(|comment| *comment.value().comment_id() == comment_id)
            .ok_or(Error::from(CommentNotFound))?;

        let comment = entry.delete_comment(comment.into_inner())?;

        self.comment_repository.delete(comment).await
    }
}
