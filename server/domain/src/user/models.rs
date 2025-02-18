#[cfg(test)]
use common::test_utils::arbitrary_uuid_v4;
use derive_getters::Getters;
use deriving_via::DerivingVia;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use uuid::Uuid;

use crate::types::authorization_guard::AuthorizationGuardDefinitions;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub name: String,
    #[cfg_attr(test, proptest(strategy = "arbitrary_uuid_v4()"))]
    pub id: Uuid,
    #[serde(default)]
    pub role: Role,
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl AuthorizationGuardDefinitions for User {
    fn can_create(&self, actor: &User) -> bool {
        actor == self
    }

    fn can_read(&self, _actor: &User) -> bool {
        true
    }

    fn can_update(&self, actor: &User) -> bool {
        actor == self
    }

    fn can_delete(&self, actor: &User) -> bool {
        actor == self
    }
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

#[derive(DerivingVia, Debug)]
#[deriving(From, Into, IntoInner, Clone)]
pub struct DiscordUserId(String);

impl DiscordUserId {
    pub fn new(user_id: String) -> Self {
        // NOTE: Discord のユーザー id は 17桁または18桁である
        //  ref: https://support.discord.com/hc/ja/articles/4407571667351
        assert!(
            user_id.len() == 17 || user_id.len() == 18,
            "Discord user id must be 17 or 18 characters long"
        );

        Self(user_id)
    }
}

#[derive(DerivingVia, Debug)]
#[deriving(From, Into, IntoInner, Clone)]
pub struct DiscordUserName(String);

impl DiscordUserName {
    pub fn new(username: String) -> Self {
        // NOTE: Discord のユーザー名は 2文字以上32文字以下である
        //  ref: https://support.discord.com/hc/ja/articles/12620128861463
        assert!(
            username.len() >= 2 && username.len() <= 32,
            "Discord user name must be between 2 and 32 characters long"
        );

        Self(username)
    }
}

#[derive(Getters, Debug)]
pub struct DiscordUser {
    id: DiscordUserId,
    name: DiscordUserName,
}

impl DiscordUser {
    pub fn new(id: DiscordUserId, name: DiscordUserName) -> Self {
        Self { id, name }
    }
}
