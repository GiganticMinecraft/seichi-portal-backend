use domain::notification::models::NotificationId;
use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct NotificationResponse {
    pub id: NotificationId,
    pub source_type: String,
    pub source_id: String,
    pub is_read: bool,
}
