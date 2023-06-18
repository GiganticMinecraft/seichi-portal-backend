use async_trait::async_trait;
use mockall::automock;

use crate::form::models::{Form, FormId, FormTitle};

#[automock]
#[async_trait]
pub trait FormRepository: Send + Sync + 'static {
    async fn create(&self, title: FormTitle) -> anyhow::Result<FormId>;
    async fn list(&self, offset: i32, limit: i32) -> anyhow::Result<Vec<Form>>;
    async fn get(&self, id: FormId) -> anyhow::Result<Form>;
    async fn delete(&self, id: FormId) -> anyhow::Result<FormId>;
}
