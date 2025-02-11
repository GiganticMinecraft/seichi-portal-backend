use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use serenity::all::ExecuteWebhook;

use crate::{form::models::WebhookUrl, user::models::DiscordUserId};

#[automock]
#[async_trait]
pub trait DiscordSender: Send + Sync {
    async fn send_direct_message(
        &self,
        user_id: DiscordUserId,
        message: String,
    ) -> Result<(), Error>;
    async fn send_webhook_message(
        &self,
        webhook_url: WebhookUrl,
        message: ExecuteWebhook,
    ) -> Result<(), Error>;
}
