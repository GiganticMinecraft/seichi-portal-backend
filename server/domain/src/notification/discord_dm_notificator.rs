use crate::notification::discord_sender::DiscordSender;
use crate::notification::models::NotificationPreference;
use crate::user::models::DiscordUserId;
use async_trait::async_trait;
use errors::Error;

/// Discord の DM 通知を行うトリガーの種類を示す。
#[derive(Debug)]
pub enum DiscordDMNotificationType {
    Message,
}

/// Discord DM に送信するメッセージの内容を示す。
#[derive(Clone, Debug)]
pub struct DiscordDMSendContents(pub Vec<String>);

impl DiscordDMSendContents {
    pub fn new(contents: Vec<String>) -> Self {
        Self(contents)
    }
}

#[async_trait]
pub trait DiscordDMNotificator {
    async fn is_notification_preference_needed(
        &self,
        notification_type: DiscordDMNotificationType,
        notification_preference: &NotificationPreference,
    ) -> bool {
        match notification_type {
            DiscordDMNotificationType::Message => {
                *notification_preference.is_send_message_notification()
            }
        }
    }

    async fn send_message<Sender: DiscordSender>(
        &self,
        sender: &Sender,
        send_target: DiscordUserId,
        notification_type: DiscordDMNotificationType,
        notification_preference: &NotificationPreference,
        discord_dm_send_contents: &DiscordDMSendContents,
    ) -> Result<(), Error> {
        if self
            .is_notification_preference_needed(notification_type, notification_preference)
            .await
        {
            sender
                .send_direct_message(send_target, discord_dm_send_contents.0.join("\n"))
                .await?
        }

        Ok(())
    }
}
