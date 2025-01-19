use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    form::{models::FormId, question::models::QuestionId},
    user::models::User,
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

#[derive(Serialize, Deserialize, Getters, PartialEq, Debug)]
pub struct AnswerEntry {
    id: AnswerId,
    user: User,
    timestamp: DateTime<Utc>,
    form_id: FormId,
    title: AnswerTitle,
}

impl AnswerEntry {
    /// [`AnswerEntry`] を新しく作成します。
    ///
    /// この関数が pub(crate) になっているのは、
    /// [`AnswerEntry`] というドメインモデルは [`FormSettings`] の状態によって
    /// 作成できるか否かが変わるためです。
    /// このため、この関数が pub であると、Invalid な状態の [`AnswerEntry`] が作成される可能性あり、
    /// [`AnswerEntry`] を作成する処理は DomainService 側に委譲するために pub(crate) にしています。
    pub fn new(user: User, form_id: FormId, title: AnswerTitle) -> Self {
        Self {
            id: AnswerId::new(),
            user,
            timestamp: Utc::now(),
            form_id,
            title,
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
    ) -> Self {
        Self {
            id,
            user,
            timestamp,
            form_id,
            title,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct FormAnswerContent {
    pub answer_id: AnswerId,
    pub question_id: QuestionId,
    pub answer: String,
}

pub type AnswerLabelId = types::IntegerId<AnswerLabel>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct AnswerLabel {
    pub id: AnswerLabelId,
    pub name: String,
}
