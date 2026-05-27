use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerEntry, AnswerId},
        answer_entry_set::models::{AnswerEntrySet, AnswerEntrySetId},
        comment::models::{Comment, CommentId},
    },
    repository::form::answer_entry_set_repository::AnswerEntrySetRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
    },
    user::models::Actor,
};
use errors::Error;

use crate::{
    database::{
        components::{DatabaseComponents, FormAnswerDatabase, FormCommentDatabase, FormDatabase},
        connection::DatabaseTransaction,
    },
    repository::Repository,
};

#[async_trait]
impl<Client> AnswerEntrySetRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self))]
    async fn create(
        &self,
        answer_entry_set: AuthorizationGuard<AnswerEntrySet, Create>,
    ) -> Result<(), Error> {
        let answer_entry_set = answer_entry_set.try_into_create(&Actor::System, |set| set)?;

        self.client
            .form()
            .create_answer_entry_set(&answer_entry_set)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn get(
        &self,
        id: AnswerEntrySetId,
    ) -> Result<Option<AuthorizationGuard<AnswerEntrySet, Read>>, Error> {
        let record = self.client.form().get_answer_entry_set(id).await?;

        Ok(record.map(|set| AuthorizationGuard::<AnswerEntrySet, Create>::from(set).into_read()))
    }

    #[tracing::instrument(skip(self))]
    async fn list_all(&self) -> Result<Vec<AuthorizationGuard<AnswerEntrySet, Read>>, Error> {
        Ok(self
            .client
            .form()
            .list_answer_entry_sets()
            .await?
            .into_iter()
            .map(|set| AuthorizationGuard::<AnswerEntrySet, Create>::from(set).into_read())
            .collect())
    }

    #[tracing::instrument(skip(self))]
    async fn update(
        &self,
        answer_entry_set: AuthorizationGuard<AnswerEntrySet, Update>,
    ) -> Result<(), Error> {
        let answer_entry_set = answer_entry_set.try_into_update(&Actor::System, |set| set)?;

        self.client
            .form()
            .update_answer_entry_set(&answer_entry_set)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, answer_entry_set))]
    async fn add_entry(
        &self,
        answer_entry_set: &AuthorizationGuard<AnswerEntrySet, Read>,
        answer_entry: &AnswerEntry,
        actor: &Actor,
    ) -> Result<(), Error> {
        let set = answer_entry_set.try_read(actor)?;

        self.client
            .form_answer()
            .post_answer(answer_entry, *set.form_id())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, answer_entry_set))]
    async fn update_entry(
        &self,
        answer_entry_set: &AuthorizationGuard<AnswerEntrySet, Read>,
        answer_entry: &AnswerEntry,
        actor: &Actor,
    ) -> Result<(), Error> {
        let set = answer_entry_set.try_read(actor)?;
        set.read_entry(*answer_entry.id(), actor)?;

        self.client
            .form_answer()
            .update_answer_entry(answer_entry, *set.form_id())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn size_entries(&self) -> Result<u32, Error> {
        self.client.form_answer().size().await.map_err(Into::into)
    }

    #[tracing::instrument(skip(self, answer_entry_set))]
    async fn add_comment(
        &self,
        answer_entry_set: &AuthorizationGuard<AnswerEntrySet, Read>,
        answer_id: AnswerId,
        comment: &Comment,
        actor: &Actor,
    ) -> Result<(), Error> {
        answer_entry_set
            .try_read(actor)?
            .read_entry(answer_id, actor)?;

        self.client
            .form_comment()
            .upsert_comment(answer_id, comment)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, answer_entry_set))]
    async fn update_comment(
        &self,
        answer_entry_set: &AuthorizationGuard<AnswerEntrySet, Read>,
        answer_id: AnswerId,
        comment: &Comment,
        actor: &Actor,
    ) -> Result<(), Error> {
        answer_entry_set
            .try_read(actor)?
            .read_entry(answer_id, actor)?;

        self.client
            .form_comment()
            .upsert_comment(answer_id, comment)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, answer_entry_set))]
    async fn delete_comment(
        &self,
        answer_entry_set: &AuthorizationGuard<AnswerEntrySet, Read>,
        answer_id: AnswerId,
        comment_id: CommentId,
        actor: &Actor,
    ) -> Result<(), Error> {
        answer_entry_set
            .try_read(actor)?
            .read_entry(answer_id, actor)?;

        self.client
            .form_comment()
            .delete_comment(comment_id)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn get_all_comments(&self) -> Result<Vec<Comment>, Error> {
        self.client
            .form_comment()
            .get_all_comments()
            .await?
            .into_iter()
            .map(TryInto::<Comment>::try_into)
            .collect()
    }

    #[tracing::instrument(skip(self))]
    async fn size_comments(&self) -> Result<u32, Error> {
        self.client.form_comment().size().await.map_err(Into::into)
    }
}
