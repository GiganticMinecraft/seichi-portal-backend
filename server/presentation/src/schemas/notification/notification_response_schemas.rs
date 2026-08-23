use chrono::{DateTime, Utc};
use domain::notification::models::Notification;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct NotificationResponse {
    pub id: Uuid,
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub url: String,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

impl From<Notification> for NotificationResponse {
    fn from(notification: Notification) -> Self {
        Self {
            id: notification.id().into_inner(),
            notification_type: notification.notification_type().to_string(),
            title: notification.content().title().to_owned(),
            body: notification.content().body().to_owned(),
            url: notification.content().url().to_owned(),
            created_at: *notification.created_at(),
            read_at: *notification.read_at(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct NotificationPageResponse {
    pub items: Vec<NotificationResponse>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct NotificationSettingsResponse {
    pub is_send_message_notification: bool,
}
