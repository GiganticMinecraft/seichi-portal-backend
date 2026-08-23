use async_trait::async_trait;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentRequirement {
    Required,
    Optional,
}

pub struct ComponentHealth {
    pub name: String,
    pub requirement: ComponentRequirement,
    pub healthy: bool,
}

#[async_trait]
pub trait HealthCheckRepository: Send + Sync {
    async fn check_components(&self) -> Vec<ComponentHealth>;
}
