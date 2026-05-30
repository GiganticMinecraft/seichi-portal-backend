use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::models::AnswerEntry,
        answer_entry_set::models::{AnswerEntrySet, AnswerEntrySetId},
    },
    types::authorization_guard::AuthorizationGuard,
    types::authorization_guard::{Create, Read, Update},
    user::models::Actor,
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
    async fn add_entry(
        &self,
        answer_entry_set: &AuthorizationGuard<AnswerEntrySet, Read>,
        answer_entry: &AnswerEntry,
        actor: &Actor,
    ) -> Result<(), Error>;
    async fn update_entry(
        &self,
        answer_entry_set: &AuthorizationGuard<AnswerEntrySet, Read>,
        answer_entry: &AnswerEntry,
        actor: &Actor,
    ) -> Result<(), Error>;
    async fn size_entries(&self) -> Result<u32, Error>;
}
