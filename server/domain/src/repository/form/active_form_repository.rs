use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    account::models::AccountUser,
    form::models::{ActiveForm, FormId, FormPagePosition},
    pagination::{Page, PageRequest},
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read, Update},
};

#[automock]
#[async_trait]
pub trait ActiveFormRepository: Send + Sync + 'static {
    async fn create(
        &self,
        actor: &AccountUser,
        form: Allowed<ActiveForm, Create>,
    ) -> Result<(), Error>;
    async fn list(
        &self,
        request: PageRequest<FormPagePosition>,
    ) -> Result<Page<AuthorizationGuard<ActiveForm, Read>, FormPagePosition>, Error>;
    async fn list_all(&self) -> Result<Vec<AuthorizationGuard<ActiveForm, Read>>, Error>;
    async fn get(&self, id: FormId) -> Result<Option<AuthorizationGuard<ActiveForm, Read>>, Error>;
    async fn update_form(
        &self,
        actor: &AccountUser,
        updated_form: Allowed<ActiveForm, Update>,
    ) -> Result<(), Error>;
    async fn size(&self) -> Result<u32, Error>;
}
