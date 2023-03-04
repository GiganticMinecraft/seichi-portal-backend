use derive_getters::Getters;
use serde::Deserialize;
use typed_builder::TypedBuilder;

#[derive(Clone, Copy, Debug)]
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
