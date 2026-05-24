use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::models::{ActiveForm, FormId},
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
    },
    user::models::ActiveUser,
};

#[automock]
#[async_trait]
pub trait ActiveFormRepository: Send + Sync + 'static {
    async fn create(
        &self,
        actor: &ActiveUser,
        form: AuthorizationGuard<ActiveForm, Create>,
    ) -> Result<(), Error>;
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<AuthorizationGuard<ActiveForm, Read>>, Error>;
    async fn get(&self, id: FormId) -> Result<Option<AuthorizationGuard<ActiveForm, Read>>, Error>;
    async fn update_form(
        &self,
        actor: &ActiveUser,
        updated_form: AuthorizationGuard<ActiveForm, Update>,
    ) -> Result<(), Error>;
    async fn size(&self) -> Result<u32, Error>;
}
