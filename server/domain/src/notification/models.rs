use derive_getters::Getters;
use errors::Error;

use crate::{
    form::{answer::models::AnswerId, models::FormId},
    notification::discord_sender::DiscordSender,
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{DiscordUserId, Role, User},
};

#[derive(Getters, Debug)]
pub struct NotificationSettings {
    recipient: User,
    is_send_message_notification: bool,
}

impl NotificationSettings {
    pub fn new(recipient: User) -> Self {
        Self {
            recipient,
            is_send_message_notification: false,
        }
    }

    pub fn update_send_message_notification(self, is_send_message_notification: bool) -> Self {
        Self {
            is_send_message_notification,
            ..self
        }
    }

    pub fn from_raw_parts(recipient: User, is_send_message_notification: bool) -> Self {
        Self {
            recipient,
            is_send_message_notification,
        }
    }
}

impl AuthorizationGuardDefinitions<NotificationSettings> for NotificationSettings {
    fn can_create(&self, actor: &User) -> bool {
        self.recipient() == actor || self.recipient().role == Role::Administrator
    }

    fn can_read(&self, actor: &User) -> bool {
        self.recipient() == actor || self.recipient().role == Role::Administrator
    }

    fn can_update(&self, actor: &User) -> bool {
        self.recipient() == actor
    }

    fn can_delete(&self, _actor: &User) -> bool {
        // NOTE: 明示的に通知設定を削除することはない(削除されるのは User が削除されたときのみ)
        false
    }
}

/// Discord の DM に送信する通知の種類
///
/// - Message
///     自身が送信した回答に対してメッセージが送信されたときの通知
pub enum DiscordDMNotificationType {
    Message {
        form_id: FormId,
        answer_id: AnswerId,
    },
}

pub struct DiscordDMNotification<Sender: DiscordSender> {
    discord_sender: Sender,
}

impl<Sender: DiscordSender> DiscordDMNotification<Sender> {
    pub fn new(discord_sender: Sender) -> Self {
        Self { discord_sender }
    }

    pub async fn send_message_notification(
        &self,
        discord_id: DiscordUserId,
        settings: &NotificationSettings,
        notification_type: DiscordDMNotificationType,
    ) -> Result<(), Error> {
        match notification_type {
            DiscordDMNotificationType::Message { form_id, answer_id } => {
                // NOTE: ここでガード節を使っていないのは、
                //  notification_type へのパターンマッチの網羅性を保証するため
                //  (ガード節を使うとその他を示すパターンが必要になるが、それを使うと網羅性が保証されなくなる)
                if settings.is_send_message_notification {
                    self.discord_sender
                        .send_direct_message(
                            discord_id,
                            [
                                "あなたの回答にメッセージが送信されました。",
                                "メッセージを確認してください。",
                                &format!(
                                    "http://localhost:3000/forms/{form_id}/answers/{answer_id}/messages"
                                ),
                            ]
                                .join("\n"),
                        )
                        .await?;
                }
            }
        }

        Ok(())
    }
}
