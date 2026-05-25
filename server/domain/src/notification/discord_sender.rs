use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::user::models::DiscordUserId;

#[automock]
#[async_trait]
pub trait DiscordSender: Send + Sync {
    async fn send_direct_message(
        &self,
        user_id: DiscordUserId,
        message: String,
    ) -> Result<(), Error>;
}
