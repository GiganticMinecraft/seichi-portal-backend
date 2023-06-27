use async_trait::async_trait;
use domain::form::models::{Form, FormDescription, FormId, FormTitle, FormUpdateTargets};
use mockall::automock;

#[async_trait]
pub trait DatabaseComponents: Send + Sync {
    type ConcreteFormDatabase: FormDatabase;
    type ConcreteHealthCheckDatabase: HealthCheckDataBase;
    type TransactionAcrossComponents: Send + Sync;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents>;
    fn form(&self) -> &Self::ConcreteFormDatabase;
    fn health_check(&self) -> &Self::ConcreteHealthCheckDatabase;
}

#[automock]
#[async_trait]
pub trait FormDatabase: Send + Sync {
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
    ) -> anyhow::Result<FormId>;
    async fn list(&self, offset: i32, limit: i32) -> anyhow::Result<Vec<Form>>;
    async fn get(&self, form_id: FormId) -> anyhow::Result<Form>;
    async fn delete(&self, form_id: FormId) -> anyhow::Result<FormId>;
    async fn update(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> anyhow::Result<Form>;
}

#[automock]
#[async_trait]
pub trait HealthCheckDataBase: Send + Sync {
    async fn health_check(&self) -> bool;
}
