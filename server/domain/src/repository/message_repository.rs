use async_trait::async_trait;
use chrono::{DateTime, Utc};
use errors::Error;
use mockall::automock;

use crate::{
    form::models::PostedAnswers,
    message::models::{Message, MessageId},
    user::models::User,
};

#[automock]
#[async_trait]
pub trait MessageRepository: Send + Sync + 'static {
    async fn post_message(&self, message: &Message) -> Result<(), Error>;
    fn reconstruct_message(
        &self,
        id: MessageId,
        related_answer: PostedAnswers,
        posted_user: User,
        body: String,
        timestamp: DateTime<Utc>,
    ) -> Message {
        Message::reconstruct(id, related_answer, posted_user, body, timestamp)
    }
}
