use async_trait::async_trait;
use domain::form::models::{FormDescription, FormId, FormTitle, FormUpdateTargets, PostedAnswers};
use errors::infra::InfraError;
use mockall::automock;

use crate::dto::FormDto;

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
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
    ) -> Result<FormId, InfraError>;
    async fn list(&self, offset: i32, limit: i32) -> Result<Vec<FormDto>, InfraError>;
    async fn get(&self, form_id: FormId) -> Result<FormDto, InfraError>;
    async fn delete(&self, form_id: FormId) -> Result<FormId, InfraError>;
    async fn update(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> Result<(), InfraError>;
    async fn post_answer(&self, answer: PostedAnswers) -> Result<(), InfraError>;
}
