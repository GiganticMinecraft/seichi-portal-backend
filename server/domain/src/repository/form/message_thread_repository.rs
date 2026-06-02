use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{answer::models::AnswerId, message_thread::models::MessageThread},
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read, Update},
};

#[automock]
#[async_trait]
pub trait MessageThreadRepository: Send + Sync + 'static {
    async fn create(&self, message_thread: Allowed<MessageThread, Create>) -> Result<(), Error>;
    async fn get_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Option<AuthorizationGuard<MessageThread, Read>>, Error>;
    async fn update(&self, message_thread: Allowed<MessageThread, Update>) -> Result<(), Error>;
}
