use chrono::{DateTime, Utc};
use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

use crate::{
    account::models::UserSnapshot,
    auth::Actor,
    form::answer::{AnswerEntry, AnswerId},
    types::authorization_guard::{AuthorizationRole, BelongsTo, GuardedBy, ParentGuarded, Read},
};

pub type AnswerStatusHistoryId = types::Id<AnswerStatusHistoryEntry>;

#[allow(non_camel_case_types)]
#[derive(
    Serialize, Deserialize, Debug, EnumString, Display, Copy, Clone, Default, PartialEq, Eq,
)]
pub enum AnswerStatus {
    #[default]
    UNADDRESSED,
    IN_PROGRESS,
    COMPLETED,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnswerStatusChange {
    from: AnswerStatus,
    to: AnswerStatus,
}

impl AnswerStatusChange {
    pub fn new(from: AnswerStatus, to: AnswerStatus) -> Option<Self> {
        (from != to).then_some(Self { from, to })
    }

    pub fn from(self) -> AnswerStatus {
        self.from
    }

    pub fn to(self) -> AnswerStatus {
        self.to
    }
}

impl TryFrom<String> for AnswerStatus {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnswerStatusHistoryPagePosition(AnswerStatusHistoryId);

impl AnswerStatusHistoryPagePosition {
    pub fn new(id: AnswerStatusHistoryId) -> Self {
        Self(id)
    }

    pub fn id(self) -> AnswerStatusHistoryId {
        self.0
    }
}

#[derive(UnsafeFromRawParts, Clone, Debug, PartialEq, Getters)]
pub struct AnswerStatusHistoryEntry {
    id: AnswerStatusHistoryId,
    #[getter(skip)]
    answer_id: AnswerId,
    from_status: AnswerStatus,
    to_status: AnswerStatus,
    changed_by: UserSnapshot,
    changed_at: DateTime<Utc>,
}

impl AuthorizationRole for AnswerStatusHistoryEntry {
    type Role = ParentGuarded<AnswerEntry>;
}

impl BelongsTo<AnswerEntry> for AnswerStatusHistoryEntry {
    fn belongs_to(&self, parent: &AnswerEntry) -> bool {
        &self.answer_id == parent.id()
    }
}

impl GuardedBy<AnswerEntry, Read> for AnswerStatusHistoryEntry {
    fn is_allowed_for(&self, _parent: &AnswerEntry, _actor: &Actor) -> bool {
        true
    }
}
