use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{answer::models::AnswerId, message_thread::models::MessageThread},
    types::authorization_guard::AuthorizationGuard,
    types::authorization_guard_with_context::{Create, Read, Update},
    user::models::Actor,
};

#[automock]
#[async_trait]
pub trait MessageThreadRepository: Send + Sync + 'static {
    async fn create(
        &self,
        message_thread: AuthorizationGuard<MessageThread, Create>,
        actor: &Actor,
    ) -> Result<(), Error>;
    async fn get_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Option<AuthorizationGuard<MessageThread, Read>>, Error>;
    async fn update(
        &self,
        message_thread: AuthorizationGuard<MessageThread, Update>,
        actor: &Actor,
    ) -> Result<(), Error>;
}
