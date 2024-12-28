use domain::form::{
    answer::models::{AnswerEntry, AnswerLabel, FormAnswerContent},
    comment::models::Comment,
};

pub struct AnswerDto {
    pub form_answer: AnswerEntry,
    pub contents: Vec<FormAnswerContent>,
    pub labels: Vec<AnswerLabel>,
    pub comments: Vec<Comment>,
}
