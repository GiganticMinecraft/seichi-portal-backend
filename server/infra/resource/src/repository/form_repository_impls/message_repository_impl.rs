use async_trait::async_trait;
use domain::{
    form::{
        answer::models::AnswerEntry,
        message::{
            models::{Message, MessageId},
            service::MessageAuthorizationContext,
        },
    },
    repository::form::message_repository::MessageRepository,
    types::authorization_guard_with_context::{
        AuthorizationGuardWithContext, Create, Delete, Read, Update,
    },
    user::models::{ActiveUser, User},
};
use errors::Error;

use crate::{
    database::components::{DatabaseComponents, FormMessageDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> MessageRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn post_message(
        &self,
        actor: &ActiveUser,
        context: &MessageAuthorizationContext,
        message: AuthorizationGuardWithContext<Message, Create, MessageAuthorizationContext>,
    ) -> Result<(), Error> {
        let actor_user = User::from(actor.clone());
        Ok(message
            .try_create(
                &actor_user,
                |message: &Message| self.client.form_message().post_message(message),
                context,
            )?
            .await?)
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_messages_by_answer(
        &self,
        answers: &AnswerEntry,
    ) -> Result<Vec<AuthorizationGuardWithContext<Message, Read, MessageAuthorizationContext>>, Error>
    {
        self.client
            .form_message()
            .fetch_messages_by_form_answer(answers)
            .await?
            .into_iter()
            .map(|record| {
                Ok::<Message, Error>(record.try_into()?).map(|message| {
                    let guard = AuthorizationGuardWithContext::new(message);

                    guard.into_read()
                })
            })
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn update_message_body(
        &self,
        actor: &ActiveUser,
        context: &MessageAuthorizationContext,
        message: AuthorizationGuardWithContext<Message, Update, MessageAuthorizationContext>,
        content: String,
    ) -> Result<(), Error> {
        let actor_user = User::from(actor.clone());
        message
            .try_update(
                &actor_user,
                |message: &Message| {
                    let message_id = message.id().to_owned();

                    self.client
                        .form_message()
                        .update_message_body(message_id, content)
                },
                context,
            )?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_message(
        &self,
        message_id: &MessageId,
    ) -> Result<
        Option<AuthorizationGuardWithContext<Message, Read, MessageAuthorizationContext>>,
        Error,
    > {
        self.client
            .form_message()
            .fetch_message(message_id)
            .await?
            .map(|record| {
                Ok::<Message, Error>(record.try_into()?).map(|message| {
                    let guard = AuthorizationGuardWithContext::new(message);

                    guard.into_read()
                })
            })
            .transpose()
    }

    #[tracing::instrument(skip(self))]
    async fn delete_message(
        &self,
        actor: &ActiveUser,
        context: &MessageAuthorizationContext,
        message: AuthorizationGuardWithContext<Message, Delete, MessageAuthorizationContext>,
    ) -> Result<(), Error> {
        let actor_user = User::from(actor.clone());
        message
            .try_delete(
                &actor_user,
                |message: &Message| {
                    let message_id = message.id().to_owned();

                    self.client.form_message().delete_message(message_id)
                },
                context,
            )?
            .await
            .map_err(Into::into)
    }
}
