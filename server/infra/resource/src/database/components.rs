use async_trait::async_trait;
use domain::form::models::{FormId, FormTitle};
use mockall::automock;

#[async_trait]
pub trait DatabaseComponents: Send + Sync {
    type ConcreteFormDatabase: FormDatabase;
    type TransactionAcrossComponents: Send + Sync;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents>;
    fn form(&self) -> &Self::ConcreteFormDatabase;
}

#[automock]
#[async_trait]
pub trait FormDatabase: Send + Sync {
    async fn create(&self, title: FormTitle) -> anyhow::Result<FormId>;
}
