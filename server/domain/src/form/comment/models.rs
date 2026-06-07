use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    form::answer::models::{AnswerEntry, AnswerId},
    types::authorization_guard::{
        AuthorizationRole, BelongsTo, Create, Delete, GuardedBy, ParentGuarded, Read, Update,
    },
    user::models::{Actor, Role, User, UserId},
};

pub type CommentId = types::Id<Comment>;

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

impl AuthorizationRole for Comment {
    type Role = ParentGuarded<AnswerEntry>;
}

impl BelongsTo<AnswerEntry> for Comment {
    fn belongs_to(&self, parent: &AnswerEntry) -> bool {
        self.answer_id() == parent.id()
    }
}

impl GuardedBy<AnswerEntry, Read> for Comment {
    fn is_allowed_for(&self, _parent: &AnswerEntry, _actor: &Actor) -> bool {
        true
    }
}

impl GuardedBy<AnswerEntry, Create> for Comment {
    fn is_allowed_for(&self, _parent: &AnswerEntry, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(user)) if user.id() == self.commented_by())
    }
}

impl GuardedBy<AnswerEntry, Update> for Comment {
    fn is_allowed_for(&self, _parent: &AnswerEntry, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(user)) if user.id() == self.commented_by())
    }
}

impl GuardedBy<AnswerEntry, Delete> for Comment {
    fn is_allowed_for(&self, _parent: &AnswerEntry, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::User(User::ActiveUser(user))
                if user.id() == self.commented_by() || user.role() == &Role::Administrator
        )
    }
}
