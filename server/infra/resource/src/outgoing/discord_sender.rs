use domain::{form::models::WebhookUrl, user::models::DiscordUserId};
use errors::infra::InfraError;
use mockall::automock;
use serenity::{all::ExecuteWebhook, async_trait};

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
