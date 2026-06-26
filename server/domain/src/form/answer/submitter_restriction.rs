use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    account::models::{Role, UserId},
    auth::Actor,
    types::authorization_guard::{AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded},
};

pub type AnswerSubmitterRestrictionId = types::Id<AnswerSubmitterRestriction>;

#[derive(Clone, DerivingVia, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: NonEmptyString), Deserialize(via: NonEmptyString))]
pub struct AnswerSubmitterRestrictionReason(NonEmptyString);

impl AnswerSubmitterRestrictionReason {
    pub fn new(reason: NonEmptyString) -> Self {
        Self(reason)
    }
}

#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
pub struct AnswerSubmitterRestriction {
    id: AnswerSubmitterRestrictionId,
    submitter_id: UserId,
    reason: AnswerSubmitterRestrictionReason,
    restricted_by: UserId,
    restricted_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

impl AnswerSubmitterRestriction {
    pub fn new(
        submitter_id: UserId,
        reason: AnswerSubmitterRestrictionReason,
        restricted_by: UserId,
        restricted_at: DateTime<Utc>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        if expires_at.is_some_and(|expires_at| expires_at <= restricted_at) {
            return Err(DomainError::InvalidEntity {
                message: "answer submitter restriction expires_at must be later than restricted_at"
                    .to_string(),
            });
        }

        Ok(Self {
            id: AnswerSubmitterRestrictionId::new(),
            submitter_id,
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

impl AuthorizationRole for AnswerSubmitterRestriction {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for AnswerSubmitterRestriction {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if user.role() == &Role::Administrator)
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if self.submitter_id == *user.id() || user.role() == &Role::Administrator)
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if user.role() == &Role::Administrator)
    }

    fn can_delete(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if user.role() == &Role::Administrator)
    }
}
