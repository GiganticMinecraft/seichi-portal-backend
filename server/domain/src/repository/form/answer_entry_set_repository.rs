use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::answer_entry_set::models::{AnswerEntrySet, AnswerEntrySetId},
    types::authorization_guard::AuthorizationGuard,
    types::authorization_guard_with_context::{Create, Read, Update},
};

#[automock]
#[async_trait]
pub trait AnswerEntrySetRepository: Send + Sync + 'static {
    async fn create(
        &self,
        answer_entry_set: AuthorizationGuard<AnswerEntrySet, Create>,
    ) -> Result<(), Error>;
    async fn get(
        &self,
        id: AnswerEntrySetId,
    ) -> Result<Option<AuthorizationGuard<AnswerEntrySet, Read>>, Error>;
    async fn list_all(&self) -> Result<Vec<AuthorizationGuard<AnswerEntrySet, Read>>, Error>;
    async fn update(
        &self,
        answer_entry_set: AuthorizationGuard<AnswerEntrySet, Update>,
    ) -> Result<(), Error>;
}
