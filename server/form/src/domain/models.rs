use crate::domain::QuestionType::{CHECKBOX, PULLDOWN, TEXT};
use derive_getters::Getters;
use serde::Deserialize;
use typed_builder::TypedBuilder;

#[derive(Clone, Debug)]
pub struct FormId(pub i32);

#[derive(TypedBuilder, Deserialize, Clone, Getters, Debug)]
pub struct FormName {
    name: String,
}

#[derive(TypedBuilder, Getters, Debug)]
pub struct Form {
    id: FormId,
    name: FormName,
    questions: Vec<Question>,
}

#[derive(TypedBuilder, Getters, Debug)]
pub struct Question {
    title: String,
    description: String,
    question_type: QuestionType,
    choices: Vec<String>,
}

#[derive(Debug)]
pub enum QuestionType {
    TEXT,
    PULLDOWN,
    CHECKBOX,
}

pub fn question_type_from_string(question_type: &str) -> Option<QuestionType> {
    match question_type.to_lowercase().as_str() {
        "text" => Some(TEXT),
        "checkbox" => Some(CHECKBOX),
        "pulldown" => Some(PULLDOWN),
        _ => None,
    }
}
