use domain::user::models::Role;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct UserInfoResponse {
    pub id: String,
    pub name: String,
    pub role: String,
    pub discord_user_id: Option<String>,
    pub discord_username: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct UserSchema {
    pub id: String,
    pub name: String,
    pub role: String,
}

impl From<domain::user::models::User> for UserSchema {
    fn from(val: domain::user::models::User) -> Self {
        UserSchema {
            id: val.id.to_string(),
            name: val.name,
            role: val.role.to_string(),
        }
    }
}

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
