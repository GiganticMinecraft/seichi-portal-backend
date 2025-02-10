use crate::form::models::WebhookUrl;
use crate::user::models::DiscordUserId;
use async_trait::async_trait;
use errors::infra::InfraError;
use mockall::automock;
use serenity::all::ExecuteWebhook;

#[automock]
#[async_trait]
pub trait DiscordSender: Send + Sync {
    async fn send_direct_message(
        &self,
        user_id: DiscordUserId,
        message: String,
    ) -> Result<(), InfraError>;
    async fn send_webhook_message(
        &self,
        webhook_url: WebhookUrl,
        message: ExecuteWebhook,
    ) -> Result<(), InfraError>;
}
