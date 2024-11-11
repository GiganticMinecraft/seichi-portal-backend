#[cfg(test)]
use common::test_utils::arbitrary_uuid_v4;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use uuid::Uuid;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct User {
    pub name: String,
    #[cfg_attr(test, proptest(strategy = "arbitrary_uuid_v4()"))]
    pub id: Uuid,
    #[serde(default)]
    pub role: Role,
}

#[cfg_attr(test, derive(Arbitrary))]
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

#[derive(Deserialize)]
pub struct UserSessionExpires {
    pub expires: i32,
}
