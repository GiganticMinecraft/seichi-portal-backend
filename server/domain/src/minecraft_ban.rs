use chrono::{DateTime, Utc};
use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
use serde::{Deserialize, Serialize};

use crate::{
    account::models::{Role, UserId},
    auth::Actor,
    types::authorization_guard::{AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded},
};

/// LiteBans から読み取った Minecraft のBAN履歴です。
///
/// LiteBans はこのサービスの所有する永続化先ではないため、外部データの値は
/// infra 層で復元します。`reason` には追加のドメイン不変条件を設けません。
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct MinecraftBan {
    uuid: UserId,
    reason: String,
    punished_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

/// 指定ユーザーのMinecraft BAN履歴を閲覧する操作の前提です。
///
/// これは集約や履歴データの入れ物ではなく、Repository に外部照会を許可するための
/// 認可対象だけを表します。
#[derive(Debug)]
pub struct MinecraftBanHistory {
    user_id: UserId,
}

impl MinecraftBanHistory {
    pub fn new(user_id: UserId) -> Self {
        Self { user_id }
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }
}

impl AuthorizationRole for MinecraftBanHistory {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for MinecraftBanHistory {
    fn can_create(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if self.user_id == *user.id() || user.role() == &Role::Administrator)
    }

    fn can_update(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}
