use chrono::{DateTime, Utc};
use derive_getters::Getters;
use errors::domain::DomainError;

use crate::{
    form::models::PostedAnswers,
    types::authorization_guard::{AuthorizationGuard, AuthorizationGuardDefinitions, Create, Read},
    user::models::{Role::Administrator, User},
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

impl AuthorizationGuardDefinitions<Message> for Message {
    fn can_create(&self, actor: &User) -> bool {
        self.posted_user.role == Administrator || self.related_answer.user.id == actor.id
    }

    fn can_read(&self, actor: &User) -> bool {
        self.posted_user.role == Administrator || self.related_answer.user.id == actor.id
    }
}

impl Message {
    pub fn try_new(
        related_answer: PostedAnswers,
        posted_user: User,
        body: String,
    ) -> Result<AuthorizationGuard<Self, Create>, DomainError> {
        AuthorizationGuard::try_new(
            &posted_user.to_owned(),
            Self {
                id: MessageId::new(),
                related_answer,
                posted_user,
                body,
                timestamp: Utc::now(),
            },
        )
    }

    /// [`Message`] の各フィールドの値を受け取り、[`Message`] を生成します。
    ///
    /// # Examples
    /// ```
    /// use chrono::{DateTime, Utc};
    /// use domain::{
    ///     form::models::{AnswerId, PostedAnswers},
    ///     message::models::{Message, MessageId},
    ///     user::models::{Role, User},
    /// };
    /// use uuid::Uuid;
    ///
    /// let user = User {
    ///     name: "user".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// let related_answer = PostedAnswers {
    ///     id: 1.into(),
    ///     user: user.to_owned(),
    ///     timestamp: Utc::now(),
    ///     form_id: Default::default(),
    ///     title: Default::default(),
    ///     answers: vec![],
    ///     comments: vec![],
    ///     labels: vec![],
    /// };
    ///
    /// let message = unsafe {
    ///     Message::from_raw_parts(
    ///         MessageId::new(),
    ///         related_answer,
    ///         user,
    ///         "test message".to_string(),
    ///         Utc::now(),
    ///     )
    /// };
    /// ```
    ///
    /// # Safety
    /// この関数は [`Message`] のバリデーションをスキップするため、
    /// データベースからすでにバリデーションされているデータを読み出すときなど、
    /// データの信頼性が保証されている場合にのみ使用してください。
    pub unsafe fn from_raw_parts(
        id: MessageId,
        related_answer: PostedAnswers,
        posted_user: User,
        body: String,
        timestamp: DateTime<Utc>,
    ) -> AuthorizationGuard<Self, Read> {
        AuthorizationGuard::new_unchecked(Self {
            id,
            related_answer,
            posted_user,
            body,
            timestamp,
        })
        .into_read()
    }
}

#[cfg(test)]
mod test {
    use uuid::Uuid;

    use super::*;
    use crate::user::models::Role::StandardUser;

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

        let message = Message::try_new(answer, message_posted_user, "test message".to_string());

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

        let message = Message::try_new(answer, user, "test message".to_string());

        assert!(message.is_ok());
    }

    #[test]
    fn should_accept_message_from_administrator() {
        let message_posted_user = User {
            name: "message_posted_user".to_string(),
            id: Uuid::new_v4(),
            role: Administrator,
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

        let message = Message::try_new(answer, message_posted_user, "test message".to_string());

        assert!(message.is_ok());
    }
}
