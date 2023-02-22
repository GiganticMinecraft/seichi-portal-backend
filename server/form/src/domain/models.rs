use derive_getters::Getters;
use serde::Deserialize;
use typed_builder::TypedBuilder;

#[derive(Clone)]
pub struct FormId(pub i32);

#[derive(Deserialize, Clone)]
pub struct FormName(pub String);

#[derive(TypedBuilder, Getters)]
pub struct Form {
    id: FormId,
    name: FormName,
    questions: Vec<Question>,
}

#[derive(TypedBuilder)]
pub struct Question {
    title: String,
    description: String,
    question_type: QuestionType,
    choices: Option<Vec<String>>,
}

pub enum QuestionType {
    TEXT,
    PULLDOWN,
    CHECKBOX,
}

pub fn from_string(value: String) -> Option<QuestionType> {
    match value.to_lowercase().as_str() {
        "text" => Some(QuestionType::TEXT),
        "checkbox" => Some(QuestionType::CHECKBOX),
        "pulldown" => Some(QuestionType::PULLDOWN),
        _ => None,
    }
}
