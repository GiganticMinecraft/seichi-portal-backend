use chrono::{DateTime, Utc};
use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};

use crate::{
    account::models::Role,
    auth::Actor,
    form::{
        answer::{AnswerAuthor, AnswerTitle, FormAnswerContent, PostedAnswerContents},
        comment::{Comment, CommentContent},
        models::{ActiveForm, FormId},
    },
    types::authorization_guard::{
        Allowed, AuthorizationRole, BelongsTo, Create, Delete, GuardedBy, ParentGuarded, Read,
        Update,
    },
};

pub type AnswerId = types::Id<AnswerEntry>;

#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, PartialEq, Debug)]
pub struct AnswerEntry {
    id: AnswerId,
    form_id: FormId,
    author: AnswerAuthor,
    timestamp: DateTime<Utc>,
    title: AnswerTitle,
    contents: Vec<FormAnswerContent>,
}

impl AnswerEntry {
    /// [`AnswerEntry`] を新しく作成します。
    pub fn new(
        form_id: FormId,
        author: AnswerAuthor,
        title: AnswerTitle,
        contents: PostedAnswerContents,
    ) -> Self {
        Self {
            id: AnswerId::new(),
            form_id,
            author,
            timestamp: Utc::now(),
            title,
            contents: contents.into_inner(),
        }
    }

    pub fn with_title(self, title: AnswerTitle) -> Self {
        Self { title, ..self }
    }
}

impl AuthorizationRole for AnswerEntry {
    type Role = ParentGuarded<ActiveForm>;
}

impl BelongsTo<ActiveForm> for AnswerEntry {
    fn belongs_to(&self, parent: &ActiveForm) -> bool {
        self.form_id() == parent.id()
    }
}

impl GuardedBy<ActiveForm, Read> for AnswerEntry {
    fn is_allowed_for(&self, parent: &ActiveForm, actor: &Actor) -> bool {
        parent.answer_settings().can_read_entry(self, actor)
    }
}

impl GuardedBy<ActiveForm, Update> for AnswerEntry {
    fn is_allowed_for(&self, _parent: &ActiveForm, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if user.role() == &Role::Administrator)
    }
}

impl GuardedBy<ActiveForm, Create> for AnswerEntry {
    fn is_allowed_for(&self, parent: &ActiveForm, actor: &Actor) -> bool {
        parent
            .answer_settings()
            .can_accept_answer(self.author(), actor)
    }
}

impl Allowed<AnswerEntry, Read> {
    pub fn authorize_comment(
        &self,
        comment: Comment,
    ) -> Result<Allowed<Comment, Read>, DomainError> {
        self.authorize_read(comment)
    }

    pub fn create_comment(
        &self,
        content: CommentContent,
    ) -> Result<Allowed<Comment, Create>, DomainError> {
        let commented_by = match self.actor() {
            Actor::AccountUser(user) => *user.id(),
            _ => return Err(DomainError::Forbidden),
        };

        let comment = Comment::new(*self.value().id(), content, commented_by);

        self.authorize_create(comment)
    }

    pub fn update_comment(
        &self,
        comment: Comment,
        content: CommentContent,
    ) -> Result<Allowed<Comment, Update>, DomainError> {
        self.authorize_update(comment.with_updated_content(content))
    }

    pub fn delete_comment(
        &self,
        comment: Comment,
    ) -> Result<Allowed<Comment, Delete>, DomainError> {
        self.authorize_delete(comment)
    }
}
