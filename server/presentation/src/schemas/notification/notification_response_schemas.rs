use domain::notification::models::{Notification, NotificationId, NotificationSource};
use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct NotificationResponse {
    pub id: NotificationId,
    pub source_type: String,
    pub source_id: String,
    pub is_read: bool,
}

impl NotificationResponse {
    pub fn from_notification_ref(notification: &Notification) -> Self {
        let (source_type, source_id) = match notification.source() {
            NotificationSource::Message(message_id) => {
                ("MESSAGE".to_string(), message_id.to_string())
            }
        };

        Self {
            id: notification.id().to_owned(),
            source_type,
            source_id,
            is_read: notification.is_read().to_owned(),
        }
    }
}
