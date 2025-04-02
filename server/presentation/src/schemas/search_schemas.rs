use domain::form::answer::models::AnswerEntry;
use domain::{
    form::{
        answer::models::{AnswerId, AnswerLabel},
        comment::models::{Comment, CommentId},
        models::{Form, FormLabel},
    },
    user::models::User,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use usecase::dto::CrossSearchDto;
use uuid::Uuid;

#[derive(Deserialize, Debug, PartialEq)]
pub struct SearchQuery {
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct CommentSchema {
    pub answer_id: AnswerId,
    pub id: CommentId,
    pub content: String,
    pub commented_by: Uuid,
}

impl From<Comment> for CommentSchema {
    fn from(value: Comment) -> Self {
        Self {
            answer_id: value.answer_id().to_owned(),
            id: value.comment_id().to_owned(),
            content: value.content().to_owned().into_inner().into_inner(),
            commented_by: value.commented_by().id,
        }
    }
}

#[derive(Serialize, Debug, PartialEq)]
pub struct CrossSearchResult {
    pub forms: Vec<Form>,
    pub users: Vec<User>,
    pub answers: Vec<AnswerEntry>,
    pub label_for_forms: Vec<FormLabel>,
    pub label_for_answers: Vec<AnswerLabel>,
    pub comments: Vec<CommentSchema>,
}

impl From<CrossSearchDto> for CrossSearchResult {
    fn from(dto: CrossSearchDto) -> Self {
        Self {
            forms: dto.forms,
            users: dto.users,
            answers: dto.answers,
            label_for_forms: dto.label_for_forms,
            label_for_answers: dto.label_for_answers,
            comments: dto.comments.into_iter().map(Into::into).collect_vec(),
        }
    }
}
