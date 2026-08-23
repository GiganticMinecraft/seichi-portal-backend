use serde::Deserialize;

#[derive(Deserialize, Debug, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct NotificationListQuery {
    /// Maximum number of notifications to return
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Cursor returned by the previous page
    pub cursor: Option<String>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct NotificationSettingsUpdateSchema {
    pub is_send_message_notification: Option<bool>,
}
