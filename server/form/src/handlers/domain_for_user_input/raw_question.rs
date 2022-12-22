use crate::handlers::domain_for_user_input::raw_question_type::RawQuestionType;
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters)]
pub struct RawQuestion {
    pub title: String,
    pub description: String,
    pub question_type: RawQuestionType,
    pub choices: Option<Vec<String>>,
}
