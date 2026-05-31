use async_trait::async_trait;
use domain::{
    form::{answer::models::AnswerEntry, answer_entry_set::models::AnswerEntrySet, models::FormId},
    repository::form::answer_entry_set_repository::AnswerEntrySetRepository,
    types::{
        authorization_guard::{Allowed, AuthorizationGuard},
        authorization_guard::{Create, Read, Update},
    },
};
use errors::Error;

use crate::{
    database::{
        components::{DatabaseComponents, FormAnswerDatabase, FormDatabase},
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
    async fn create(&self, answer_entry_set: Allowed<AnswerEntrySet, Create>) -> Result<(), Error> {
        self.client
            .form()
            .create_answer_entry_set(answer_entry_set.value())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn get(
        &self,
        form_id: FormId,
    ) -> Result<Option<AuthorizationGuard<AnswerEntrySet, Read>>, Error> {
        let record = self.client.form().get_answer_entry_set(form_id).await?;

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
    async fn update(&self, answer_entry_set: Allowed<AnswerEntrySet, Update>) -> Result<(), Error> {
        self.client
            .form()
            .update_answer_entry_set(answer_entry_set.value())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, answer_entry_set))]
    async fn add_entry(
        &self,
        answer_entry_set: &Allowed<AnswerEntrySet, Read>,
        answer_entry: &AnswerEntry,
    ) -> Result<(), Error> {
        self.client
            .form_answer()
            .post_answer(answer_entry, *answer_entry_set.value().form_id())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, answer_entry_set))]
    async fn update_entry(
        &self,
        answer_entry_set: &Allowed<AnswerEntrySet, Update>,
        answer_entry: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error> {
        self.client
            .form_answer()
            .update_answer_entry(answer_entry.value(), *answer_entry_set.value().form_id())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn size_entries(&self) -> Result<u32, Error> {
        self.client.form_answer().size().await.map_err(Into::into)
    }
}
