use async_trait::async_trait;
use domain::{
    form::{answer::models::AnswerId, message_thread::models::MessageThread},
    repository::form::message_thread_repository::MessageThreadRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard::{Create, Read, Update},
    },
    user::models::Actor,
};
use errors::{Error, infra::InfraError};
use std::collections::HashSet;
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    database::{
        components::{DatabaseComponents, FormMessageDatabase, FormMessageThreadDatabase},
        connection::DatabaseTransaction,
    },
    repository::Repository,
};

#[async_trait]
impl<Client> MessageThreadRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self))]
    async fn create(
        &self,
        message_thread: AuthorizationGuard<MessageThread, Create>,
        actor: &Actor,
    ) -> Result<(), Error> {
        let thread = message_thread.try_into_create(actor, |t| t)?;
        let answer_id = thread.answer_id().into_inner().to_string();
        let answer_author_id = thread.answer_author_id().to_string();

        self.client
            .form_message_thread()
            .create_message_thread(&answer_id, &answer_author_id)
            .await?;

        for message in thread.messages() {
            self.client
                .form_message()
                .post_message(message, *thread.answer_id())
                .await?;
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn get_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Option<AuthorizationGuard<MessageThread, Read>>, Error> {
        let answer_id_str = answer_id.into_inner().to_string();

        let Some(answer_author_id_str) = self
            .client
            .form_message_thread()
            .get_thread_author_by_answer_id(&answer_id_str)
            .await?
        else {
            return Ok(None);
        };

        let messages = self
            .client
            .form_message()
            .fetch_messages_by_answer_id(answer_id)
            .await?
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?;

        let thread = MessageThread::from_raw_parts(
            answer_id,
            Uuid::from_str(&answer_author_id_str)
                .map_err(InfraError::from)?
                .into(),
            messages,
        );

        Ok(Some(AuthorizationGuard::<MessageThread, Read>::from(
            thread,
        )))
    }

    #[tracing::instrument(skip(self))]
    async fn update(
        &self,
        message_thread: AuthorizationGuard<MessageThread, Update>,
        actor: &Actor,
    ) -> Result<(), Error> {
        let thread = message_thread.try_into_update(actor, |t| t)?;

        let existing = self
            .client
            .form_message()
            .fetch_messages_by_answer_id(*thread.answer_id())
            .await?;

        let current_ids: HashSet<String> = thread
            .messages()
            .iter()
            .map(|m| m.id().into_inner().to_string())
            .collect();

        for record in &existing {
            if !current_ids.contains(&record.id) {
                self.client
                    .form_message()
                    .delete_message(Uuid::from_str(&record.id).map_err(InfraError::from)?.into())
                    .await?;
            }
        }

        let existing_ids: HashSet<String> = existing.into_iter().map(|r| r.id).collect();

        for message in thread.messages() {
            let msg_id_str = message.id().into_inner().to_string();
            if existing_ids.contains(&msg_id_str) {
                self.client
                    .form_message()
                    .update_message_body(*message.id(), message.body().to_owned())
                    .await?;
            } else {
                self.client
                    .form_message()
                    .post_message(message, *thread.answer_id())
                    .await?;
            }
        }

        Ok(())
    }
}
