use async_trait::async_trait;
use mockall::automock;

use crate::form::models::{Form, FormId, FormTitle};

#[automock]
#[async_trait]
pub trait FormRepository: Send + Sync + 'static {
    async fn create(&self, title: FormTitle) -> anyhow::Result<FormId>;
    async fn list(&self, offset: i64, limit: i64) -> anyhow::Result<Vec<Form>>;
}
