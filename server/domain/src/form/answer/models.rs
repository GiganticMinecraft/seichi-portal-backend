use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    form::{models::FormId, question::models::QuestionId},
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Role, User},
};

pub type AnswerId = types::Id<AnswerEntry>;

#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<NonEmptyString>), Deserialize(via: Option::<NonEmptyString>
))]
pub struct AnswerTitle(Option<NonEmptyString>);

impl AnswerTitle {
    pub fn new(title: Option<NonEmptyString>) -> Self {
        Self(title)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct FormAnswerContent {
    pub question_id: QuestionId,
    pub answer: String,
}

#[derive(Serialize, Deserialize, Getters, PartialEq, Debug)]
pub struct AnswerEntry {
    id: AnswerId,
    user: User,
    timestamp: DateTime<Utc>,
    form_id: FormId,
    title: AnswerTitle,
    contents: Vec<FormAnswerContent>,
}

impl AnswerEntry {
    /// [`AnswerEntry`] を新しく作成します。
    pub fn new(
        user: User,
        form_id: FormId,
        title: AnswerTitle,
        contents: Vec<FormAnswerContent>,
    ) -> Self {
        Self {
            id: AnswerId::new(),
            user,
            timestamp: Utc::now(),
            form_id,
            title,
            contents,
        }
    }

    /// [`AnswerEntry`] の各フィールドを指定して新しく作成します。
    ///
    /// # Safety
    /// この関数はオブジェクトを新しく作成しない場合
    /// (例えば、データベースから取得した場合)にのみ使用してください。
    pub unsafe fn from_raw_parts(
        id: AnswerId,
        user: User,
        timestamp: DateTime<Utc>,
        form_id: FormId,
        title: AnswerTitle,
        contents: Vec<FormAnswerContent>,
    ) -> Self {
        Self {
            id,
            user,
            timestamp,
            form_id,
            title,
            contents,
        }
    }

    pub fn with_title(self, title: AnswerTitle) -> Self {
        Self { title, ..self }
    }
}

pub type AnswerLabelId = types::Id<AnswerLabel>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct AnswerLabel {
    id: AnswerLabelId,
    name: String,
}

impl AnswerLabel {
    pub fn new(name: String) -> Self {
        Self {
            id: AnswerLabelId::new(),
            name,
        }
    }

    pub fn from_raw_parts(id: AnswerLabelId, name: String) -> Self {
        Self { id, name }
    }
}

impl AuthorizationGuardDefinitions for AnswerLabel {
    fn can_create(&self, actor: &User) -> bool {
        actor.role == Role::Administrator
    }

    fn can_read(&self, _actor: &User) -> bool {
        true
    }

    fn can_update(&self, actor: &User) -> bool {
        actor.role == Role::Administrator
    }

    fn can_delete(&self, actor: &User) -> bool {
        actor.role == Role::Administrator
    }
}
