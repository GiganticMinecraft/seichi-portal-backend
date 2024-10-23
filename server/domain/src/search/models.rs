use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    form::models::{AnswerContent, AnswerId, CommentId, Form, Label},
    user::models::User,
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Comment {
    pub answer_id: AnswerId,
    pub id: CommentId,
    pub content: String,
    pub commented_by: Uuid,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct CrossSearchResult {
    pub forms: Vec<Form>,
    pub users: Vec<User>,
    pub answers: Vec<AnswerContent>,
    pub label_for_forms: Vec<Label>,
    pub label_for_answers: Vec<Label>,
    pub comments: Vec<Comment>,
}
