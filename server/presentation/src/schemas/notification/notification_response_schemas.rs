use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct NotificationSettingsResponse {
    pub is_send_message_notification: bool,
}
