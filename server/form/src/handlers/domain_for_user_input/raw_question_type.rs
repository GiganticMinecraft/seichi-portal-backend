use crate::domain::QuestionType;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Serialize, Deserialize, Display)]
pub enum RawQuestionType {
    #[strum(serialize = "text")]
    TEXT,
    #[strum(serialize = "pulldown")]
    PULLDOWN,
    #[strum(serialize = "checkbox")]
    CHECKBOX,
}

impl RawQuestionType {
    pub fn to_question_type(&self) -> QuestionType {
        match self {
            RawQuestionType::TEXT => QuestionType::TEXT,
            RawQuestionType::CHECKBOX => QuestionType::CHECKBOX,
            RawQuestionType::PULLDOWN => QuestionType::PULLDOWN,
        }
    }
}
