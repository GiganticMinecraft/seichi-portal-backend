use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::{
    answer::models::AnswerId,
    message::models::{Message, MessageId},
};

#[automock]
#[async_trait]
pub trait MessageRepository: Send + Sync + 'static {
    async fn post_message(&self, message: &Message) -> Result<(), Error>;
    async fn fetch_messages_by_answer_id(&self, answer_id: AnswerId)
    -> Result<Vec<Message>, Error>;
    async fn update_message_body(&self, message_id: MessageId, body: String) -> Result<(), Error>;
    async fn fetch_message(&self, message_id: &MessageId) -> Result<Option<Message>, Error>;
    async fn delete_message(&self, message_id: MessageId) -> Result<(), Error>;
}
