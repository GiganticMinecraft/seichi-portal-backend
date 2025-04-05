use async_trait::async_trait;
use domain::notification::discord_dm_notificator::{
    DiscordDMNotificationType, DiscordDMNotificator, DiscordDMSendContents,
};
use domain::notification::discord_sender::DiscordSender;
use domain::notification::models::NotificationPreference;
use domain::notification::notification_api::NotificationAPI;
use domain::user::models::DiscordUserId;
use errors::Error;

pub struct NotificationAPIImpl<Sender: DiscordSender, Notificator: DiscordDMNotificator> {
    sender: Sender,
    notificator: Notificator,
}

impl<Sender: DiscordSender, Notificator: DiscordDMNotificator>
    NotificationAPIImpl<Sender, Notificator>
{
    pub const fn new(sender: Sender, notificator: Notificator) -> Self {
        Self {
            sender,
            notificator,
        }
    }
}

#[async_trait]
impl<Sender: DiscordSender, Notificator: DiscordDMNotificator + Sync + Send> NotificationAPI
    for NotificationAPIImpl<Sender, Notificator>
{
    async fn send_discord_dm_notification(
        &self,
        send_target: DiscordUserId,
        notification_type: DiscordDMNotificationType,
        notification_preference: &NotificationPreference,
        send_contents: &DiscordDMSendContents,
    ) -> Result<(), Error> {
        self.notificator
            .send_message(
                &self.sender,
                send_target,
                notification_type,
                notification_preference,
                send_contents,
            )
            .await
    }
}
