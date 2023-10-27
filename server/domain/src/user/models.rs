use serde::{Deserialize, Serialize};
use strum_macros::Display;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct User {
    pub name: String,
    pub id: Uuid,
    #[serde(default)]
    pub role: Role,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Display)]
pub enum Role {
    Administrator,
    #[default]
    #[strum(serialize = "STANDARD_USER")]
    StandardUser,
}
