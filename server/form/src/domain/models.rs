use derive_getters::Getters;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder, Getters)]
pub struct Form {
    id: FormId,
    name: FormName,
    questions: Vec<Question>,
}

pub struct FormId(pub i32);

pub struct FormName(pub String);

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
