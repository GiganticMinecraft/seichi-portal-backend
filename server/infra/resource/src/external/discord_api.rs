use async_trait::async_trait;
use errors::infra::InfraError;
use mockall::automock;

use crate::external::discord_api_schema::DiscordUserSchema;

#[automock]
#[async_trait]
pub trait DiscordAPI: Send + Sync {
    async fn fetch_user(&self, token: String) -> Result<DiscordUserSchema, InfraError>;
}
