use crate::form::handlers::domain_for_user_input::raw_question_type::RawQuestionType;
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters)]
pub struct RawQuestion {
    title: String,
    description: String,
    question_type: RawQuestionType,
    choices: Option<Vec<String>>,
}
