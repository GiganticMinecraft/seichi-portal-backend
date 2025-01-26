use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::models::AnswerEntry,
        message::models::{Message, MessageId},
    },
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Delete, Read, Update},
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait MessageRepository: Send + Sync + 'static {
    async fn post_message(
        &self,
        actor: &User,
        message: AuthorizationGuard<Message, Create>,
    ) -> Result<(), Error>;
    async fn fetch_messages_by_answer(
        &self,
        answers: &AnswerEntry,
    ) -> Result<Vec<AuthorizationGuard<Message, Read>>, Error>;
    async fn update_message_body(
        &self,
        actor: &User,
        message: AuthorizationGuard<Message, Update>,
        body: String,
    ) -> Result<(), Error>;
    async fn fetch_message(
        &self,
        message_id: &MessageId,
    ) -> Result<Option<AuthorizationGuard<Message, Read>>, Error>;
    async fn delete_message(
        &self,
        actor: &User,
        message: AuthorizationGuard<Message, Delete>,
    ) -> Result<(), Error>;
}
