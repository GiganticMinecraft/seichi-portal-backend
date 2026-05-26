use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::answer::models::AnswerEntry;

#[automock]
#[async_trait]
pub trait AnswerRepository: Send + Sync + 'static {
    async fn post_answer(&self, answer: &AnswerEntry) -> Result<(), Error>;
    async fn update_answer_entry(&self, answer_entry: &AnswerEntry) -> Result<(), Error>;
    async fn size(&self) -> Result<u32, Error>;
}
