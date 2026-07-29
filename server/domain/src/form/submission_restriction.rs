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

pub type FormSubmissionRestrictionId = types::Id<FormSubmissionRestriction>;

#[derive(Clone, DerivingVia, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: NonEmptyString), Deserialize(via: NonEmptyString))]
pub struct FormSubmissionRestrictionReason(NonEmptyString);

impl FormSubmissionRestrictionReason {
    pub fn new(reason: NonEmptyString) -> Self {
        Self(reason)
    }
}

#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
pub struct FormSubmissionRestriction {
    id: FormSubmissionRestrictionId,
    submitter_id: UserId,
    reason: FormSubmissionRestrictionReason,
    restricted_by: UserId,
    restricted_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    lifted_at: Option<DateTime<Utc>>,
    lifted_by: Option<UserId>,
}

impl FormSubmissionRestriction {
    pub fn new(
        submitter_id: UserId,
        reason: FormSubmissionRestrictionReason,
        restricted_by: UserId,
        restricted_at: DateTime<Utc>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        if expires_at.is_some_and(|expires_at| expires_at <= restricted_at) {
            return Err(DomainError::InvalidEntity {
                message: "form submission restriction expires_at must be later than restricted_at"
                    .to_string(),
            });
        }

        Ok(Self {
            id: FormSubmissionRestrictionId::new(),
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
pub struct FormSubmissionRestrictionHistory {
    submitter_id: UserId,
    restrictions: Vec<FormSubmissionRestriction>,
}

impl FormSubmissionRestrictionHistory {
    pub fn new(
        submitter_id: UserId,
        restrictions: Vec<FormSubmissionRestriction>,
    ) -> Result<Self, DomainError> {
        if restrictions
            .iter()
            .any(|restriction| restriction.submitter_id != submitter_id)
        {
            return Err(DomainError::InvalidEntity {
                message: "form submission restriction history must contain only restrictions for the submitter".to_string(),
            });
        }

        Ok(Self {
            submitter_id,
            restrictions,
        })
    }

    pub fn into_restrictions(self) -> Vec<FormSubmissionRestriction> {
        self.restrictions
    }
}

impl AuthorizationRole for FormSubmissionRestrictionHistory {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for FormSubmissionRestrictionHistory {
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

impl AuthorizationRole for FormSubmissionRestriction {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for FormSubmissionRestriction {
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
