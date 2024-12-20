use crate::{form::answer::models::AnswerId, user::models::User};
use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};

pub type CommentId = types::Id<Comment>;

#[derive(DerivingVia, Debug, PartialEq)]
#[deriving(Clone, From, Into, IntoInner, Serialize(via: String), Deserialize(via: String))]
pub struct CommentContent(String);

impl CommentContent {
    pub fn try_new(content: String) -> Result<Self, DomainError> {
        if content.is_empty() {
            return Err(DomainError::EmptyValue);
        }

        Ok(Self(content))
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
