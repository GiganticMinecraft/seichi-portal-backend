use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct NotificationSettingsUpdateSchema {
    pub recipient_id: Uuid,
    pub is_send_message_notification: bool,
}
