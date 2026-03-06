use async_trait::async_trait;

#[async_trait]
pub trait HealthCheckRepository: Send + Sync {
    async fn ping_db(&self) -> bool;
    async fn ping_meilisearch(&self) -> bool;
    async fn is_rabbitmq_connected(&self) -> bool;
    async fn is_discord_connected(&self) -> bool;
}
