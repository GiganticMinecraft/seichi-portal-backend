use domain::notification::models::NotificationId;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct NotificationUpdateReadStateSchema {
    pub notification_id: NotificationId,
    pub is_read: bool,
}

#[derive(Deserialize, Debug)]
pub struct NotificationSettingsUpdateSchema {
    pub recipient_id: Uuid,
    pub is_send_message_notification: bool,
}
