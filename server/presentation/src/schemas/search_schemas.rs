use domain::form::answer::models::{AnswerLabel, FormAnswerContent};
use domain::form::models::{Form, FormLabel};
use domain::search::models::Comment;
use domain::user::models::User;
use serde::{Deserialize, Serialize};
use usecase::dto::CrossSearchDto;

#[derive(Deserialize, Debug, PartialEq)]
pub struct SearchQuery {
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct CrossSearchResult {
    pub forms: Vec<Form>,
    pub users: Vec<User>,
    pub answers: Vec<FormAnswerContent>,
    pub label_for_forms: Vec<FormLabel>,
    pub label_for_answers: Vec<AnswerLabel>,
    pub comments: Vec<Comment>,
}

impl From<CrossSearchDto> for CrossSearchResult {
    fn from(dto: CrossSearchDto) -> Self {
        Self {
            forms: dto.forms,
            users: dto.users,
            answers: dto.answers,
            label_for_forms: dto.label_for_forms,
            label_for_answers: dto.label_for_answers,
            comments: dto.comments,
        }
    }
}
