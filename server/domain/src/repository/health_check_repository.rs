use async_trait::async_trait;
use mockall::automock;

#[automock]
#[async_trait]
pub trait HealthCheckRepository: Send + Sync + 'static {
    async fn health_check(&self) -> bool;
}
