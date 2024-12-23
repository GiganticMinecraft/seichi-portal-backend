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

pub type AnswerId = types::Id<FormAnswer>;

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
pub struct FormAnswer {
    id: AnswerId,
    user: User,
    timestamp: DateTime<Utc>,
    form_id: FormId,
    title: AnswerTitle,
}

impl FormAnswer {
    pub fn new(user: User, form_id: FormId, title: AnswerTitle) -> Self {
        Self {
            id: AnswerId::new(),
            user,
            timestamp: Utc::now(),
            form_id,
            title,
        }
    }

    pub fn from_raw_parts(
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
