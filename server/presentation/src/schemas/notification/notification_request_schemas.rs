use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct NotificationSettingsUpdateSchema {
    pub is_send_message_notification: Option<bool>,
}
