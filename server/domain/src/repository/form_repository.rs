use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::models::{Form, FormDescription, FormId, FormTitle, FormUpdateTargets};

#[automock]
#[async_trait]
pub trait FormRepository: Send + Sync + 'static {
    async fn create(&self, title: FormTitle, description: FormDescription)
        -> Result<FormId, Error>;
    async fn list(&self, offset: i32, limit: i32) -> Result<Vec<Form>, Error>;
    async fn get(&self, id: FormId) -> Result<Form, Error>;
    async fn delete(&self, id: FormId) -> Result<FormId, Error>;
    async fn update(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> Result<(), Error>;
}
