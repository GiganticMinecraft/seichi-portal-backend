use crate::{
    account::models::UserSnapshot,
    auth::Actor,
    form::answer::{AnswerEntry, AnswerId},
    types::authorization_guard::{AuthorizationRole, BelongsTo, GuardedBy, ParentGuarded, Read},
};
use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use types::non_empty_string::NonEmptyString;

#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<NonEmptyString>), Deserialize(via: Option::<NonEmptyString>
))]
pub struct AnswerTitle(Option<NonEmptyString>);

impl AnswerTitle {
    pub fn new(title: Option<NonEmptyString>) -> Self {
        Self(title)
    }
}

pub type AnswerTitleHistoryId = types::Id<AnswerTitleHistoryEntry>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnswerTitleHistoryPagePosition(AnswerTitleHistoryId);

impl AnswerTitleHistoryPagePosition {
    pub fn new(id: AnswerTitleHistoryId) -> Self {
        Self(id)
    }

    pub fn id(self) -> AnswerTitleHistoryId {
        self.0
    }
}

#[derive(UnsafeFromRawParts, Clone, Debug, PartialEq, Getters)]
pub struct AnswerTitleHistoryEntry {
    id: AnswerTitleHistoryId,
    #[getter(skip)]
    answer_id: AnswerId,
    from_title: AnswerTitle,
    to_title: AnswerTitle,
    changed_by: UserSnapshot,
    changed_at: DateTime<Utc>,
}

impl AuthorizationRole for AnswerTitleHistoryEntry {
    type Role = ParentGuarded<AnswerEntry>;
}

impl BelongsTo<AnswerEntry> for AnswerTitleHistoryEntry {
    fn belongs_to(&self, parent: &AnswerEntry) -> bool {
        &self.answer_id == parent.id()
    }
}

impl GuardedBy<AnswerEntry, Read> for AnswerTitleHistoryEntry {
    fn is_allowed_for(&self, _parent: &AnswerEntry, _actor: &Actor) -> bool {
        true
    }
}
