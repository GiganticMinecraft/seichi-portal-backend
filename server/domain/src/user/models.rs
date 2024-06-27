use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct User {
    pub name: String,
    pub id: Uuid,
    #[serde(default)]
    pub role: Role,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Display, EnumString)]
pub enum Role {
    #[serde(rename = "ADMINISTRATOR")]
    #[strum(serialize = "ADMINISTRATOR")]
    Administrator,
    #[default]
    #[serde(rename = "STANDARD_USER")]
    #[strum(serialize = "STANDARD_USER")]
    StandardUser,
}

#[derive(Deserialize)]
pub struct RoleQuery {
    pub role: Role,
}
