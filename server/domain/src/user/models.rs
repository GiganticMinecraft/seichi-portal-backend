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

#[derive(DerivingVia, Debug, PartialOrd, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
#[deriving(
    From,
    Into,
    Copy,
    IntoInner(via: Uuid),
    Display(via: Uuid),
    Serialize(via: Uuid),
    Deserialize(via: Uuid)
)]
pub struct UserId(
    #[cfg_attr(test, proptest(strategy = "arbitrary_uuid_v4()"))]
    #[underlying]
    Uuid,
);

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Debug, Clone)]
pub struct ActiveUser {
    name: String,
    id: UserId,
    #[serde(default)]
    role: Role,
}

impl ActiveUser {
    pub fn new(name: String, id: UserId, role: Role) -> Self {
        Self { name, id, role }
    }
}

impl PartialEq for ActiveUser {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum User {
    ActiveUser(ActiveUser),
    TemporaryUser(TemporaryUser),
    Anonymous,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Actor {
    User(User),
    System,
}

impl From<ActiveUser> for User {
    fn from(user: ActiveUser) -> Self {
        Self::ActiveUser(user)
    }
}

impl From<TemporaryUser> for User {
    fn from(user: TemporaryUser) -> Self {
        Self::TemporaryUser(user)
    }
}

impl From<ActiveUser> for Actor {
    fn from(user: ActiveUser) -> Self {
        Self::User(User::ActiveUser(user))
    }
}

impl From<TemporaryUser> for Actor {
    fn from(user: TemporaryUser) -> Self {
        Self::User(User::TemporaryUser(user))
    }
}

impl From<User> for Actor {
    fn from(user: User) -> Self {
        Self::User(user)
    }
}

#[derive(DerivingVia, Debug, PartialOrd, PartialEq, Eq, Hash, Clone, Copy)]
#[deriving(
    From,
    Into,
    IntoInner(via: Uuid),
    Display(via: Uuid),
    Serialize(via: Uuid),
    Deserialize(via: Uuid)
)]
pub struct TemporaryUserId(#[underlying] Uuid);

/// 一時回答が許可されたフォームで、ログインせずに回答した人の著者情報。
///
/// `TemporaryUser` は永続的な認証主体ではなく、回答作成時に入力された情報を
/// 回答の著者として保持するためのスナップショットである。`id` は通常の
/// `UserId` やログインセッションとは別の、回答著者を一時ユーザーとして識別する
/// ローカルな UUID として扱う。
///
/// `name` と `contact_text` は、管理者や回答閲覧者が回答者を識別し、必要に応じて
/// 連絡するための入力値である。権限判定上は回答の作成主体としてだけ使われ、
/// 通常の `User` と同じ閲覧・更新権限は持たない。
#[derive(Serialize, Deserialize, Getters, Debug, Clone, PartialEq, Eq)]
pub struct TemporaryUser {
    id: TemporaryUserId,
    name: String,
    contact_text: String,
}

impl TemporaryUser {
    pub fn new(name: String, contact_text: String) -> Self {
        Self {
            id: TemporaryUserId::from(Uuid::new_v4()),
            name,
            contact_text,
        }
    }

    pub fn from_raw_parts(id: TemporaryUserId, name: String, contact_text: String) -> Self {
        Self {
            id,
            name,
            contact_text,
        }
    }
}

impl AuthorizationGuardDefinitions for ActiveUser {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if actor == self)
    }

    fn can_read(&self, _actor: &Actor) -> bool {
        true
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if actor.role == Role::Administrator)
    }

    fn can_delete(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if actor == self)
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
pub struct UserSessionExpires {
    pub expires: u32,
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
