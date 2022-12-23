pub enum QuestionType {
    TEXT,
    PULLDOWN,
    CHECKBOX,
}

pub fn from_string(value: &str) -> Option<QuestionType> {
    match value.to_lowercase().as_str() {
        "text" => Some(QuestionType::TEXT),
        "checkbox" => Some(QuestionType::CHECKBOX),
        "pulldown" => Some(QuestionType::PULLDOWN),
        _ => None,
    }
}
