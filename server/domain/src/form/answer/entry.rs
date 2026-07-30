use chrono::{DateTime, Utc};
use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

use crate::{
    account::models::Role,
    auth::Actor,
    form::{
        answer::{AnswerAuthor, AnswerTitle, FormAnswerContent, PostedAnswerContents},
        models::{ActiveForm, FormId},
    },
    types::authorization_guard::{
        AuthorizationRole, BelongsTo, Create, GuardedBy, ParentGuarded, Read, Update,
    },
};

pub type AnswerId = types::Id<AnswerEntry>;

/// 個別の回答を第三者へ公開するかどうかを表します。
#[derive(
    Serialize, Deserialize, Debug, EnumString, Display, Copy, Clone, Default, PartialEq, Eq,
)]
pub enum AnswerPublication {
    #[default]
    PUBLIC,
    PRIVATE,
}

impl TryFrom<String> for AnswerPublication {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnswerPagePosition {
    last_answer_id: AnswerId,
}

impl AnswerPagePosition {
    pub fn new(last_answer_id: AnswerId) -> Self {
        Self { last_answer_id }
    }

    pub fn last_answer_id(self) -> AnswerId {
        self.last_answer_id
    }
}

#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, PartialEq, Debug)]
pub struct AnswerEntry {
    id: AnswerId,
    form_id: FormId,
    author: AnswerAuthor,
    timestamp: DateTime<Utc>,
    title: AnswerTitle,
    publication: AnswerPublication,
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
            publication: AnswerPublication::PUBLIC,
            contents: contents.into_inner(),
        }
    }

    pub fn with_title(self, title: AnswerTitle) -> Self {
        Self { title, ..self }
    }

    pub fn change_publication(self, publication: AnswerPublication) -> Self {
        Self {
            publication,
            ..self
        }
    }

    pub(crate) fn publication_allows_read(&self, actor: &Actor) -> bool {
        match self.publication {
            AnswerPublication::PUBLIC => true,
            AnswerPublication::PRIVATE => {
                matches!(
                    actor,
                    Actor::AccountUser(user)
                        if user.role() == &Role::Administrator
                            || self.author.authenticated_user_id() == Some(*user.id())
                ) || matches!(actor, Actor::System)
            }
        }
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
