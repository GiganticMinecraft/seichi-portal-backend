use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::models::{ArchivedForm, FormId},
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard::{Create, Read, Update},
    },
    user::models::ActiveUser,
};

#[automock]
#[async_trait]
pub trait ArchivedFormRepository: Send + Sync + 'static {
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
        query: Option<String>,
    ) -> Result<Vec<AuthorizationGuard<ArchivedForm, Read>>, Error>;
    async fn get(
        &self,
        id: FormId,
    ) -> Result<Option<AuthorizationGuard<ArchivedForm, Read>>, Error>;
    async fn archive(
        &self,
        actor: &ActiveUser,
        form: AuthorizationGuard<ArchivedForm, Create>,
    ) -> Result<AuthorizationGuard<ArchivedForm, Read>, Error>;
    async fn restore(
        &self,
        actor: &ActiveUser,
        form: AuthorizationGuard<ArchivedForm, Update>,
    ) -> Result<(), Error>;
}
