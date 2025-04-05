use crate::notification::discord_dm_notificator::{
    DiscordDMNotificationType, DiscordDMSendContents,
};
use crate::notification::models::NotificationPreference;
use crate::user::models::DiscordUserId;
use async_trait::async_trait;
use errors::Error;

#[async_trait]
pub trait NotificationAPI {
    async fn send_discord_dm_notification(
        &self,
        send_target: DiscordUserId,
        notification_type: DiscordDMNotificationType,
        notification_preference: &NotificationPreference,
        send_contents: &DiscordDMSendContents,
    ) -> Result<(), Error>;
}
