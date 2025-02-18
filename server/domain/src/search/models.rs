use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::form::{answer::models::AnswerId, comment::models::CommentId};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Comment {
    pub answer_id: AnswerId,
    pub id: CommentId,
    pub content: String,
    pub commented_by: Uuid,
}
