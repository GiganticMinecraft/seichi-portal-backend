use domain::notification::models::NotificationId;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct NotificationUpdateReadStateSchema {
    pub notification_id: NotificationId,
    pub is_read: bool,
}
