use async_trait::async_trait;
use domain::{
    form::{answer::models::AnswerId, comment::models::Comment},
    repository::form::comment_repository::CommentRepository,
    types::authorization_guard::{AuthorizationGuard, Create, Delete, Update},
    user::models::Actor,
};
use errors::Error;

use crate::{
    database::{
        components::{DatabaseComponents, FormCommentDatabase},
        connection::DatabaseTransaction,
    },
    repository::Repository,
};

#[async_trait]
impl<Client> CommentRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self, comment))]
    async fn create(
        &self,
        comment: AuthorizationGuard<Comment, Create>,
        actor: &Actor,
    ) -> Result<(), Error> {
        let comment = comment.try_into_create(actor, |comment| comment)?;

        self.client
            .form_comment()
            .upsert_comment(*comment.answer_id(), &comment)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn find_by_answer_id(&self, answer_id: AnswerId) -> Result<Vec<Comment>, Error> {
        self.client
            .form_comment()
            .get_comments(answer_id)
            .await?
            .into_iter()
            .map(TryInto::<Comment>::try_into)
            .collect()
    }

    #[tracing::instrument(skip(self, comment))]
    async fn update(
        &self,
        comment: AuthorizationGuard<Comment, Update>,
        actor: &Actor,
    ) -> Result<(), Error> {
        let comment = comment.try_into_update(actor, |comment| comment)?;

        self.client
            .form_comment()
            .upsert_comment(*comment.answer_id(), &comment)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, comment))]
    async fn delete(
        &self,
        comment: AuthorizationGuard<Comment, Delete>,
        actor: &Actor,
    ) -> Result<(), Error> {
        let comment = comment.try_into_delete(actor, |comment| comment)?;

        self.client
            .form_comment()
            .delete_comment(*comment.comment_id())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn get_all(&self) -> Result<Vec<Comment>, Error> {
        self.client
            .form_comment()
            .get_all_comments()
            .await?
            .into_iter()
            .map(TryInto::<Comment>::try_into)
            .collect()
    }

    #[tracing::instrument(skip(self))]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form_comment().size().await.map_err(Into::into)
    }
}
