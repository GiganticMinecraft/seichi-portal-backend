use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{form::answer::models::AnswerId, user::models::User};

pub type CommentId = types::IntegerId<Comment>;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Comment {
    pub answer_id: AnswerId,
    pub comment_id: CommentId,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub commented_by: User,
}
