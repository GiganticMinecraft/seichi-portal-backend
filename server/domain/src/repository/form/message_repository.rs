use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::models::AnswerEntry,
        message::{
            models::{Message, MessageId},
            service::MessageAuthorizationContext,
        },
    },
    types::authorization_guard_with_context::{
        AuthorizationGuardWithContext, Create, Delete, Read, Update,
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait MessageRepository: Send + Sync + 'static {
    async fn post_message(
        &self,
        actor: &User,
        context: &MessageAuthorizationContext,
        message: AuthorizationGuardWithContext<Message, Create, MessageAuthorizationContext>,
    ) -> Result<(), Error>;
    async fn fetch_messages_by_answer(
        &self,
        answers: &AnswerEntry,
    ) -> Result<Vec<AuthorizationGuardWithContext<Message, Read, MessageAuthorizationContext>>, Error>;
    async fn update_message_body(
        &self,
        actor: &User,
        context: &MessageAuthorizationContext,
        message: AuthorizationGuardWithContext<Message, Update, MessageAuthorizationContext>,
        body: String,
    ) -> Result<(), Error>;
    async fn fetch_message(
        &self,
        message_id: &MessageId,
    ) -> Result<
        Option<AuthorizationGuardWithContext<Message, Read, MessageAuthorizationContext>>,
        Error,
    >;
    async fn delete_message(
        &self,
        actor: &User,
        context: &MessageAuthorizationContext,
        message: AuthorizationGuardWithContext<Message, Delete, MessageAuthorizationContext>,
    ) -> Result<(), Error>;
}
