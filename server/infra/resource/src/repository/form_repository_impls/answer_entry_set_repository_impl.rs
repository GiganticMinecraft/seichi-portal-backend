use async_trait::async_trait;
use domain::{
    form::answer_entry_set::models::{AnswerEntrySet, AnswerEntrySetId},
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
        components::{DatabaseComponents, FormDatabase},
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
}
