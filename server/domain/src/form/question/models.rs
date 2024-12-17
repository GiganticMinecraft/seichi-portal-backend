#[cfg(test)]
use common::test_utils::arbitrary_with_size;
use derive_getters::Getters;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

use crate::form::models::FormId;

pub type QuestionId = types::IntegerId<Question>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Getters, Debug, PartialEq)]
pub struct Question {
    #[serde(default)]
    pub id: Option<QuestionId>,
    pub form_id: FormId,
    pub title: String,
    pub description: Option<String>,
    pub question_type: QuestionType,
    #[cfg_attr(test, proptest(strategy = "arbitrary_with_size(1..100)"))]
    #[serde(default)]
    pub choices: Vec<String>,
    pub is_required: bool,
}

impl Question {
    pub fn new(
        id: Option<QuestionId>,
        form_id: FormId,
        title: String,
        description: Option<String>,
        question_type: QuestionType,
        choices: Vec<String>,
        is_required: bool,
    ) -> Self {
        Self {
            id,
            form_id,
            title,
            description,
            question_type,
            choices,
            is_required,
        }
    }

    pub fn from_raw_parts(
        id: Option<QuestionId>,
        form_id: FormId,
        title: String,
        description: Option<String>,
        question_type: QuestionType,
        choices: Vec<String>,
        is_required: bool,
    ) -> Self {
        Self {
            id,
            form_id,
            title,
            description,
            question_type,
            choices,
            is_required,
        }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, EnumString, PartialOrd, PartialEq, Display,
)]
#[strum(ascii_case_insensitive)]
pub enum QuestionType {
    TEXT,
    SINGLE,
    MULTIPLE,
}

impl TryFrom<String> for QuestionType {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}
