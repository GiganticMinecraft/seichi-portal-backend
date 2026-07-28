use chrono::{DateTime, Utc};
use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};

use crate::{
    account::models::{Role, UserId},
    auth::Actor,
    types::authorization_guard::{AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded},
};

/// Minecraft サーバーで記録されたBANです。
///
/// このサービスが所有しない永続化先から読み込む値のため、外部データの値は
/// infra 層で復元します。`reason` には追加のドメイン不変条件を設けません。
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct MinecraftBan {
    user_id: UserId,
    reason: String,
    punished_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

/// 指定ユーザーのMinecraft BAN履歴です。
#[derive(Debug, PartialEq)]
pub struct MinecraftBanHistory {
    user_id: UserId,
    minecraft_bans: Vec<MinecraftBan>,
}

impl MinecraftBanHistory {
    pub fn new(user_id: UserId, minecraft_bans: Vec<MinecraftBan>) -> Result<Self, DomainError> {
        if minecraft_bans.iter().any(|ban| ban.user_id != user_id) {
            return Err(DomainError::InvalidEntity {
                message: "minecraft ban history must contain only bans for the user".to_string(),
            });
        }

        Ok(Self {
            user_id,
            minecraft_bans,
        })
    }

    pub fn into_minecraft_bans(self) -> Vec<MinecraftBan> {
        self.minecraft_bans
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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;

    fn ban(user_id: UserId) -> MinecraftBan {
        unsafe { MinecraftBan::from_raw_parts(user_id, "reason".to_string(), Utc::now(), None) }
    }

    #[test]
    fn history_rejects_bans_for_another_user() {
        let result = MinecraftBanHistory::new(
            UserId::from(Uuid::from_u128(1)),
            vec![ban(UserId::from(Uuid::from_u128(2)))],
        );

        assert!(matches!(result, Err(DomainError::InvalidEntity { .. })));
    }
}
