use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    account::models::{Role, UserId, UserSnapshot},
    auth::Actor,
    form::{answer::AnswerId, comment_thread::CommentThread},
    types::authorization_guard::{
        AuthorizationRole, BelongsTo, Create, Delete, DeleteTransition, GuardedBy, ParentGuarded,
        Read, Update,
    },
};

pub type CommentId = types::Id<Comment>;
pub type CommentHistoryId = types::Id<CommentHistoryEntry>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentHistoryPagePosition(CommentHistoryId);

impl CommentHistoryPagePosition {
    pub fn new(id: CommentHistoryId) -> Self {
        Self(id)
    }

    pub fn id(&self) -> CommentHistoryId {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CommentHistoryAction {
    Create,
    Update,
    Delete,
}

#[derive(UnsafeFromRawParts, Clone, Debug, PartialEq, Getters)]
pub struct CommentHistoryEntry {
    id: CommentHistoryId,
    #[getter(skip)]
    answer_id: AnswerId,
    comment_id: CommentId,
    original_author: UserSnapshot,
    original_timestamp: DateTime<Utc>,
    action: CommentHistoryAction,
    content: CommentContent,
    operated_by: UserSnapshot,
    operated_at: DateTime<Utc>,
}

impl AuthorizationRole for CommentHistoryEntry {
    type Role = ParentGuarded<CommentThread>;
}

impl BelongsTo<CommentThread> for CommentHistoryEntry {
    fn belongs_to(&self, parent: &CommentThread) -> bool {
        &self.answer_id == parent.answer_id()
    }
}

impl GuardedBy<CommentThread, Read> for CommentHistoryEntry {
    fn is_allowed_for(&self, _parent: &CommentThread, actor: &Actor) -> bool {
        !matches!(self.action, CommentHistoryAction::Delete)
            || matches!(actor, Actor::AccountUser(user) if user.role() == &Role::Administrator)
    }
}

#[derive(DerivingVia, Debug, PartialEq)]
#[deriving(Clone, From, Into, IntoInner, Serialize, Deserialize)]
pub struct CommentContent(NonEmptyString);

impl CommentContent {
    pub fn new(content: NonEmptyString) -> Self {
        Self(content)
    }
}

#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
pub struct Comment {
    answer_id: AnswerId,
    comment_id: CommentId,
    content: CommentContent,
    timestamp: DateTime<Utc>,
    commented_by: UserId,
}

impl Comment {
    pub(crate) fn new(answer_id: AnswerId, content: CommentContent, commented_by: UserId) -> Self {
        Self {
            answer_id,
            comment_id: CommentId::new(),
            content,
            timestamp: Utc::now(),
            commented_by,
        }
    }

    pub fn with_updated_content(self, content: CommentContent) -> Self {
        Self { content, ..self }
    }
}

/// 削除されたコメントと、削除時点の操作情報を表す。
#[derive(Getters, Debug, PartialEq)]
pub struct DeletedComment {
    comment: Comment,
    deleted_at: DateTime<Utc>,
    deleted_by: UserSnapshot,
}

impl AuthorizationRole for Comment {
    type Role = ParentGuarded<CommentThread>;
}

impl BelongsTo<CommentThread> for Comment {
    fn belongs_to(&self, parent: &CommentThread) -> bool {
        self.answer_id() == parent.answer_id()
    }
}

impl GuardedBy<CommentThread, Read> for Comment {
    fn is_allowed_for(&self, _parent: &CommentThread, _actor: &Actor) -> bool {
        true
    }
}

impl GuardedBy<CommentThread, Create> for Comment {
    fn is_allowed_for(&self, _parent: &CommentThread, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if user.id() == self.commented_by())
    }
}

impl GuardedBy<CommentThread, Update> for Comment {
    fn is_allowed_for(&self, _parent: &CommentThread, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::AccountUser(user)
                if user.id() == self.commented_by() || user.role() == &Role::Administrator
        )
    }
}

impl GuardedBy<CommentThread, Delete> for Comment {
    fn is_allowed_for(&self, parent: &CommentThread, actor: &Actor) -> bool {
        <Self as GuardedBy<CommentThread, Update>>::is_allowed_for(self, parent, actor)
    }
}

impl DeleteTransition for Comment {
    type Created = DeletedComment;
    type Context = DateTime<Utc>;

    fn transition(
        self,
        deleted_at: Self::Context,
        actor: &Actor,
    ) -> Result<Self::Created, DomainError> {
        let deleted_by = match actor {
            Actor::AccountUser(user) => UserSnapshot::from(user),
            _ => return Err(DomainError::Forbidden),
        };
        Ok(DeletedComment {
            comment: self,
            deleted_at,
            deleted_by,
        })
    }
}
