use domain::form::models::{AnswerLabel, Comment, FormAnswer, FormAnswerContent};

pub struct AnswerDto {
    pub form_answer: FormAnswer,
    pub contents: Vec<FormAnswerContent>,
    pub labels: Vec<AnswerLabel>,
    pub comments: Vec<Comment>,
}
