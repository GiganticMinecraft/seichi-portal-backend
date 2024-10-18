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
            return Err(DomainError::Forbidden);
        }

        Ok(Self {
            id: MessageId::new(),
            related_answer,
            posted_user,
            body,
            timestamp: Utc::now(),
        })
    }

    pub(crate) fn reconstruct(
        id: MessageId,
        related_answer: PostedAnswers,
        posted_user: User,
        body: String,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            related_answer,
            posted_user,
            body,
            timestamp,
        }
    }
}

#[cfg(test)]
mod test {
    use uuid::Uuid;

    use super::*;
    use crate::user::models::Role;

    #[test]
    fn should_reject_message_from_unrelated_user() {
        let message_posted_user = User {
            name: "message_posted_user".to_string(),
            id: Uuid::new_v4(),
            role: StandardUser,
        };

        let answer_posted_user = User {
            name: "answer_posted_user".to_string(),
            id: Uuid::new_v4(),
            role: StandardUser,
        };

        let answer = PostedAnswers {
            id: Default::default(),
            user: answer_posted_user,
            timestamp: Utc::now(),
            form_id: Default::default(),
            title: Default::default(),
            answers: vec![],
            comments: vec![],
            labels: vec![],
        };

        let message = Message::new(answer, message_posted_user, "test message".to_string());

        assert!(message.is_err());
    }

    #[test]
    fn should_accept_message_from_answer_posted_user() {
        let user = User {
            name: "user".to_string(),
            id: Uuid::new_v4(),
            role: StandardUser,
        };

        let answer = PostedAnswers {
            id: Default::default(),
            user: user.to_owned(),
            timestamp: Utc::now(),
            form_id: Default::default(),
            title: Default::default(),
            answers: vec![],
            comments: vec![],
            labels: vec![],
        };

        let message = Message::new(answer, user, "test message".to_string());

        assert!(message.is_ok());
    }

    #[test]
    fn should_accept_message_from_administrator() {
        let message_posted_user = User {
            name: "message_posted_user".to_string(),
            id: Uuid::new_v4(),
            role: Role::Administrator,
        };

        let answer_posted_user = User {
            name: "answer_posted_user".to_string(),
            id: Uuid::new_v4(),
            role: StandardUser,
        };

        let answer = PostedAnswers {
            id: Default::default(),
            user: answer_posted_user,
            timestamp: Utc::now(),
            form_id: Default::default(),
            title: Default::default(),
            answers: vec![],
            comments: vec![],
            labels: vec![],
        };

        let message = Message::new(answer, message_posted_user, "test message".to_string());

        assert!(message.is_ok());
    }
}
