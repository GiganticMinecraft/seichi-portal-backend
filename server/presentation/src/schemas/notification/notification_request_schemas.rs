use serde::Deserialize;

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct NotificationSettingsUpdateSchema {
    pub is_send_message_notification: Option<bool>,
}
