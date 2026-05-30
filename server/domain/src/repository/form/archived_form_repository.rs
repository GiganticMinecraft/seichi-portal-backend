use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::models::{ArchivedForm, FormId},
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read, Update},
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
        form: Allowed<ArchivedForm, Create>,
    ) -> Result<AuthorizationGuard<ArchivedForm, Read>, Error>;
    async fn restore(&self, form: Allowed<ArchivedForm, Update>) -> Result<(), Error>;
}
