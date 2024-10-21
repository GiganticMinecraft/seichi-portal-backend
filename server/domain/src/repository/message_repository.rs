use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::models::PostedAnswers,
    message::models::Message,
    types::authorization_guard::{AuthorizationGuard, Read},
};

#[automock]
#[async_trait]
pub trait MessageRepository: Send + Sync + 'static {
    async fn post_message(&self, message: &Message) -> Result<(), Error>;
    async fn fetch_messages_by_answer_id(
        &self,
        answers: &PostedAnswers,
    ) -> Result<Vec<AuthorizationGuard<Message, Read>>, Error>;
}
