use crate::{form::answer::models::AnswerId, user::models::User};
use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

pub type CommentId = types::Id<Comment>;

#[derive(DerivingVia, Debug, PartialEq)]
#[deriving(Clone, From, Into, IntoInner, Serialize, Deserialize)]
pub struct CommentContent(NonEmptyString);

impl CommentContent {
    pub fn new(content: NonEmptyString) -> Self {
        Self(content)
    }
}

#[derive(Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct Comment {
    answer_id: AnswerId,
    comment_id: CommentId,
    content: CommentContent,
    timestamp: DateTime<Utc>,
    commented_by: User,
}

impl Comment {
    pub fn new(answer_id: AnswerId, content: CommentContent, commented_by: User) -> Self {
        Self {
            answer_id,
            comment_id: CommentId::new(),
            content,
            timestamp: Utc::now(),
            commented_by,
        }
    }

    pub fn from_raw_parts(
        answer_id: AnswerId,
        comment_id: CommentId,
        content: CommentContent,
        timestamp: DateTime<Utc>,
        commented_by: User,
    ) -> Self {
        Self {
            answer_id,
            comment_id,
            content,
            timestamp,
            commented_by,
        }
    }
}
