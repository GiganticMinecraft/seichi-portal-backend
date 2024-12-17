use domain::form::{
    answer::models::{AnswerLabel, FormAnswer, FormAnswerContent},
    comment::models::Comment,
};

pub struct AnswerDto {
    pub form_answer: FormAnswer,
    pub contents: Vec<FormAnswerContent>,
    pub labels: Vec<AnswerLabel>,
    pub comments: Vec<Comment>,
}
