use crate::form::handlers::domain_for_user_input::raw_question_type::QuestionType;

struct Question {
    title: String,
    description: String,
    question_type: QuestionType,
    choices: Option<Vec<String>>,
}
