use serde::Serialize;

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct NotificationSettingsResponse {
    pub is_send_message_notification: bool,
}
