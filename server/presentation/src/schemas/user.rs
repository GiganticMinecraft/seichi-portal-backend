use domain::user::models::Role;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct DiscordOAuthToken {
    pub token: String,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct UserUpdateSchema {
    pub name: Option<String>,
    pub id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub role: Option<Role>,
}
