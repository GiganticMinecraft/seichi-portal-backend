use async_trait::async_trait;
use mockall::automock;

use crate::form::models::{FormId, FormTitle};

#[automock]
#[async_trait]
pub trait FormRepository: Send + Sync + 'static {
    async fn create(&self, title: FormTitle) -> anyhow::Result<FormId>;
}
