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
