use domain::form::answer::AnswerEntry;
use domain::{
    account::models::AccountUser,
    form::{
        answer::{AnswerId, AnswerLabel},
        comment::{Comment, CommentId},
        models::{ActiveForm, FormLabel},
    },
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;
use usecase::models::CrossSearchOutput;
use uuid::Uuid;

#[derive(Deserialize, Debug, PartialEq, utoipa::ToSchema)]
pub struct SearchQuery {
    #[serde(default)]
    pub query: Option<NonEmptyString>,
}

#[derive(Serialize, Debug, PartialEq, utoipa::ToSchema)]
pub struct CommentSchema {
    #[schema(value_type = String, format = "uuid")]
    pub answer_id: AnswerId,
    #[schema(value_type = String, format = "uuid")]
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
            commented_by: value.commented_by().into_inner(),
        }
    }
}

#[derive(Serialize, Debug, PartialEq, utoipa::ToSchema)]
pub struct CrossSearchResult {
    #[schema(value_type = Vec<serde_json::Value>)]
    pub forms: Vec<ActiveForm>,
    #[schema(value_type = Vec<serde_json::Value>)]
    pub users: Vec<AccountUser>,
    #[schema(value_type = Vec<serde_json::Value>)]
    pub answers: Vec<AnswerEntry>,
    #[schema(value_type = Vec<serde_json::Value>)]
    pub label_for_forms: Vec<FormLabel>,
    #[schema(value_type = Vec<serde_json::Value>)]
    pub label_for_answers: Vec<AnswerLabel>,
    pub comments: Vec<CommentSchema>,
}

impl From<CrossSearchOutput> for CrossSearchResult {
    fn from(output: CrossSearchOutput) -> Self {
        Self {
            forms: output.forms,
            users: output.users,
            answers: output.answers,
            label_for_forms: output.label_for_forms,
            label_for_answers: output.label_for_answers,
            comments: output.comments.into_iter().map(Into::into).collect_vec(),
        }
    }
}
