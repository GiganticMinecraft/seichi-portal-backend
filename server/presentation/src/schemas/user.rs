use domain::user::models::Role;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct DiscordOAuthToken {
    pub token: String,
}

#[derive(Deserialize, Debug)]
pub struct UserUpdateSchema {
    pub name: Option<String>,
    pub id: Option<Uuid>,
    pub role: Option<Role>,
}
