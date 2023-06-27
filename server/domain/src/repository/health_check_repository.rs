#[automock]
#[async_trait]
pub trait HealthCheckRepository: Send + Sync + 'static {
    async fn health_check(&self) -> bool;
}
