use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::arbitrary_uuid_v4;
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

use crate::types::authorization_guard::{
    AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded,
};

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

pub type AnswerSubmissionRestrictionId = types::Id<AnswerSubmissionRestriction>;

#[derive(Clone, DerivingVia, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: NonEmptyString), Deserialize(via: NonEmptyString))]
pub struct AnswerSubmissionRestrictionReason(NonEmptyString);

impl AnswerSubmissionRestrictionReason {
    pub fn new(reason: NonEmptyString) -> Self {
        Self(reason)
    }
}

#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
pub struct AnswerSubmissionRestriction {
    id: AnswerSubmissionRestrictionId,
    user_id: UserId,
    reason: AnswerSubmissionRestrictionReason,
    restricted_by: UserId,
    restricted_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

impl AnswerSubmissionRestriction {
    pub fn new(
        user_id: UserId,
        reason: AnswerSubmissionRestrictionReason,
        restricted_by: UserId,
        restricted_at: DateTime<Utc>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        if expires_at.is_some_and(|expires_at| expires_at <= restricted_at) {
            return Err(DomainError::InvalidEntity {
                message:
                    "answer submission restriction expires_at must be later than restricted_at"
                        .to_string(),
            });
        }

        Ok(Self {
            id: AnswerSubmissionRestrictionId::new(),
            user_id,
            reason,
            restricted_by,
            restricted_at,
            expires_at,
        })
    }

    pub fn is_active_at(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_none_or(|expires_at| now < expires_at)
    }
}

impl AuthorizationRole for AnswerSubmissionRestriction {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for AnswerSubmissionRestriction {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(user)) if user.role() == &Role::Administrator)
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(user)) if self.user_id == *user.id() || user.role() == &Role::Administrator)
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(user)) if user.role() == &Role::Administrator)
    }

    fn can_delete(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(user)) if user.role() == &Role::Administrator)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnswerSubmitter {
    user: ActiveUser,
}

impl AnswerSubmitter {
    pub fn try_new(
        user: ActiveUser,
        restriction: Option<AnswerSubmissionRestriction>,
        now: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        if let Some(restriction) = restriction {
            if restriction.user_id != *user.id() {
                return Err(DomainError::InvalidEntity {
                    message: "answer submission restriction must belong to the submitter"
                        .to_string(),
                });
            }

            if !restriction.is_active_at(now) {
                return Ok(Self { user });
            }

            return Err(DomainError::AnswerSubmissionRestricted {
                reason: restriction.reason.into_inner().into_inner(),
                expires_at: restriction.expires_at,
            });
        }

        Ok(Self { user })
    }

    pub fn user(&self) -> &ActiveUser {
        &self.user
    }

    pub fn into_user(self) -> ActiveUser {
        self.user
    }
}

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
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Debug, Clone, PartialEq, Eq)]
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
}

impl AuthorizationRole for ActiveUser {
    type Role = SelfGuarded;
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

#[derive(DerivingVia, Debug, PartialEq, Eq)]
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

#[derive(DerivingVia, Debug, PartialEq, Eq)]
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

#[derive(Getters, Debug, Clone, PartialEq, Eq)]
pub struct DiscordUser {
    id: DiscordUserId,
    name: DiscordUserName,
}

impl DiscordUser {
    pub fn new(id: DiscordUserId, name: DiscordUserName) -> Self {
        Self { id, name }
    }
}

#[derive(Getters, Debug, Clone, PartialEq, Eq)]
pub struct DiscordAccountLink {
    user_id: UserId,
    discord_user: DiscordUser,
}

impl DiscordAccountLink {
    pub fn new(user_id: UserId, discord_user: DiscordUser) -> Self {
        Self {
            user_id,
            discord_user,
        }
    }
}

impl AuthorizationRole for DiscordAccountLink {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for DiscordAccountLink {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if *actor.id() == self.user_id)
    }

    fn can_read(&self, _actor: &Actor) -> bool {
        true
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if *actor.id() == self.user_id)
    }

    fn can_delete(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if *actor.id() == self.user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_id(seed: u128) -> UserId {
        UserId::from(Uuid::from_u128(seed))
    }

    fn active_user(name: &str, id: UserId, role: Role) -> ActiveUser {
        ActiveUser::new(name.to_string(), id, role)
    }

    fn temporary_actor() -> Actor {
        Actor::from(TemporaryUser::new(
            "temporary_user".to_string(),
            "contact".to_string(),
        ))
    }

    #[test]
    fn active_user_equality_depends_only_on_id() {
        let id = user_id(1);

        assert_eq!(
            active_user("user", id, Role::StandardUser),
            active_user("renamed_user", id, Role::Administrator)
        );
        assert_ne!(
            active_user("user", user_id(1), Role::StandardUser),
            active_user("user", user_id(2), Role::StandardUser)
        );
    }

    #[test]
    fn active_user_can_be_created_only_by_self() {
        let target = active_user("target", user_id(1), Role::StandardUser);
        let same_user = Actor::from(active_user("same_user", user_id(1), Role::Administrator));
        let other_user = Actor::from(active_user("other", user_id(2), Role::StandardUser));

        assert!(target.can_create(&same_user));
        assert!(!target.can_create(&other_user));
        assert!(!target.can_create(&temporary_actor()));
        assert!(!target.can_create(&Actor::from(User::Anonymous)));
        assert!(!target.can_create(&Actor::System));
    }

    #[test]
    fn active_user_can_be_read_by_any_actor() {
        let target = active_user("target", user_id(1), Role::StandardUser);
        let active_user_actor = Actor::from(active_user("reader", user_id(2), Role::StandardUser));

        assert!(target.can_read(&active_user_actor));
        assert!(target.can_read(&temporary_actor()));
        assert!(target.can_read(&Actor::from(User::Anonymous)));
        assert!(target.can_read(&Actor::System));
    }

    #[test]
    fn active_user_can_be_updated_only_by_administrator() {
        let target = active_user("target", user_id(1), Role::StandardUser);
        let administrator = Actor::from(active_user("admin", user_id(2), Role::Administrator));
        let standard_user = Actor::from(active_user("standard", user_id(1), Role::StandardUser));

        assert!(target.can_update(&administrator));
        assert!(!target.can_update(&standard_user));
        assert!(!target.can_update(&temporary_actor()));
        assert!(!target.can_update(&Actor::from(User::Anonymous)));
        assert!(!target.can_update(&Actor::System));
    }

    #[test]
    fn active_user_can_be_deleted_only_by_self() {
        let target = active_user("target", user_id(1), Role::StandardUser);
        let same_user = Actor::from(active_user("same_user", user_id(1), Role::Administrator));
        let other_user = Actor::from(active_user("other", user_id(2), Role::Administrator));

        assert!(target.can_delete(&same_user));
        assert!(!target.can_delete(&other_user));
        assert!(!target.can_delete(&temporary_actor()));
        assert!(!target.can_delete(&Actor::from(User::Anonymous)));
        assert!(!target.can_delete(&Actor::System));
    }

    #[test]
    fn discord_account_link_can_be_written_only_by_linked_user() {
        let linked_user_id = user_id(1);
        let link = DiscordAccountLink::new(
            linked_user_id,
            DiscordUser::new(
                DiscordUserId::new("12345678901234567".to_string()),
                DiscordUserName::new("discord_user".to_string()),
            ),
        );
        let linked_user = Actor::from(active_user("linked", linked_user_id, Role::StandardUser));
        let other_user = Actor::from(active_user("other", user_id(2), Role::Administrator));
        let non_active_actors = [
            temporary_actor(),
            Actor::from(User::Anonymous),
            Actor::System,
        ];

        assert!(link.can_create(&linked_user));
        assert!(link.can_update(&linked_user));
        assert!(link.can_delete(&linked_user));

        assert!(!link.can_create(&other_user));
        assert!(!link.can_update(&other_user));
        assert!(!link.can_delete(&other_user));

        for actor in non_active_actors {
            assert!(!link.can_create(&actor));
            assert!(!link.can_update(&actor));
            assert!(!link.can_delete(&actor));
        }
    }

    #[test]
    fn discord_account_link_can_be_read_by_any_actor() {
        let link = DiscordAccountLink::new(
            user_id(1),
            DiscordUser::new(
                DiscordUserId::new("12345678901234567".to_string()),
                DiscordUserName::new("discord_user".to_string()),
            ),
        );
        let active_user_actor = Actor::from(active_user("reader", user_id(2), Role::StandardUser));

        assert!(link.can_read(&active_user_actor));
        assert!(link.can_read(&temporary_actor()));
        assert!(link.can_read(&Actor::from(User::Anonymous)));
        assert!(link.can_read(&Actor::System));
    }

    #[test]
    fn answer_submitter_is_created_when_user_has_no_active_restriction() {
        let user = active_user("user", user_id(1), Role::StandardUser);

        assert!(AnswerSubmitter::try_new(user, None, Utc::now()).is_ok());
    }

    #[test]
    fn answer_submitter_rejects_active_restriction() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            *user.id(),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(2),
            now,
            None,
        )
        .unwrap();

        let result = AnswerSubmitter::try_new(user, Some(restriction), now);

        assert_eq!(
            result,
            Err(DomainError::AnswerSubmissionRestricted {
                reason: "spam".to_string(),
                expires_at: None,
            })
        );
    }

    #[test]
    fn answer_submitter_ignores_expired_restriction() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            *user.id(),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(2),
            now - chrono::Duration::hours(2),
            Some(now - chrono::Duration::hours(1)),
        )
        .unwrap();

        assert!(AnswerSubmitter::try_new(user, Some(restriction), now).is_ok());
    }

    #[test]
    fn answer_submitter_rejects_restriction_for_different_user() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            user_id(2),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(3),
            now,
            None,
        )
        .unwrap();

        let result = AnswerSubmitter::try_new(user, Some(restriction), now);

        assert_eq!(
            result,
            Err(DomainError::InvalidEntity {
                message: "answer submission restriction must belong to the submitter".to_string(),
            })
        );
    }

    #[test]
    fn discord_user_id_allows_17_or_18_characters() {
        assert_eq!(
            DiscordUserId::new("12345678901234567".to_string()),
            DiscordUserId("12345678901234567".to_string())
        );
        assert_eq!(
            DiscordUserId::new("123456789012345678".to_string()),
            DiscordUserId("123456789012345678".to_string())
        );
    }

    #[test]
    #[should_panic(expected = "Discord user id must be 17 or 18 characters long")]
    fn discord_user_id_rejects_16_characters() {
        DiscordUserId::new("1234567890123456".to_string());
    }

    #[test]
    #[should_panic(expected = "Discord user id must be 17 or 18 characters long")]
    fn discord_user_id_rejects_19_characters() {
        DiscordUserId::new("1234567890123456789".to_string());
    }
}
