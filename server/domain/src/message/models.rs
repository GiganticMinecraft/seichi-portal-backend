use chrono::{DateTime, Utc};
use derive_getters::Getters;
use errors::domain::DomainError;

use crate::{
    form::models::PostedAnswers,
    user::models::{Role::StandardUser, User},
};

pub type MessageId = types::Id<Message>;

#[derive(Getters, Debug)]
pub struct Message {
    id: MessageId,
    related_answer: PostedAnswers,
    posted_user: User,
    body: String,
    timestamp: DateTime<Utc>,
}

impl Message {
    pub fn new(
        related_answer: PostedAnswers,
        posted_user: User,
        body: String,
    ) -> Result<Self, DomainError> {
        if posted_user.role == StandardUser && related_answer.user.id != posted_user.id {
            return Err(DomainError::Forbidden {
                reason: "You cannot access to this message.".to_string(),
            });
        }

        Ok(Self {
            id: MessageId::new(),
            related_answer,
            posted_user,
            body,
            timestamp: Utc::now(),
        })
    }
}
