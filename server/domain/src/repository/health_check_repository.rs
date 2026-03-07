use async_trait::async_trait;

pub struct ComponentHealth {
    pub name: String,
    pub healthy: bool,
}

#[async_trait]
pub trait HealthCheckRepository: Send + Sync {
    async fn check_components(&self) -> Vec<ComponentHealth>;
}
