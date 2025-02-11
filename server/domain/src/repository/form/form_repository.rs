use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::models::{Form, FormId},
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Delete, Read, Update},
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait FormRepository: Send + Sync + 'static {
    async fn create(
        &self,
        actor: &User,
        form: AuthorizationGuard<Form, Create>,
    ) -> Result<(), Error>;
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<AuthorizationGuard<Form, Read>>, Error>;
    async fn get(&self, id: FormId) -> Result<Option<AuthorizationGuard<Form, Read>>, Error>;
    async fn delete(
        &self,
        actor: &User,
        form: AuthorizationGuard<Form, Delete>,
    ) -> Result<(), Error>;
    async fn update_form(
        &self,
        actor: &User,
        updated_form: AuthorizationGuard<Form, Update>,
    ) -> Result<(), Error>;
}
