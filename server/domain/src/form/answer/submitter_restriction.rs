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
    lifted_at: Option<DateTime<Utc>>,
    lifted_by: Option<UserId>,
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
            lifted_at: None,
            lifted_by: None,
        })
    }

    pub fn is_active_at(&self, now: DateTime<Utc>) -> bool {
        self.lifted_at.is_none() && self.expires_at.is_none_or(|expires_at| now < expires_at)
    }
}

#[derive(Debug, PartialEq)]
pub struct AnswerSubmitterRestrictionHistory {
    submitter_id: UserId,
    restrictions: Vec<AnswerSubmitterRestriction>,
}

impl AnswerSubmitterRestrictionHistory {
    pub fn new(
        submitter_id: UserId,
        restrictions: Vec<AnswerSubmitterRestriction>,
    ) -> Result<Self, DomainError> {
        if restrictions
            .iter()
            .any(|restriction| restriction.submitter_id != submitter_id)
        {
            return Err(DomainError::InvalidEntity {
                message: "answer submitter restriction history must contain only restrictions for the submitter".to_string(),
            });
        }

        Ok(Self {
            submitter_id,
            restrictions,
        })
    }

    pub fn into_restrictions(self) -> Vec<AnswerSubmitterRestriction> {
        self.restrictions
    }
}

impl AuthorizationRole for AnswerSubmitterRestrictionHistory {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for AnswerSubmitterRestrictionHistory {
    fn can_create(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if self.submitter_id == *user.id() || user.role() == &Role::Administrator)
    }

    fn can_update(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
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
