use chrono::{DateTime, Utc};
use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};

use crate::{
    types::authorization_guard::{AuthorizationRole, ParentGuarded},
    user::models::UserId,
};

pub type MessageId = types::Id<Message>;

impl AuthorizationRole for Message {
    type Role = ParentGuarded;
}

#[derive(UnsafeFromRawParts, Getters, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Message {
    id: MessageId,
    sender_id: UserId,
    body: String,
    timestamp: DateTime<Utc>,
}

impl Message {
    /// [`Message`] の生成を試みます。
    ///
    /// 以下の場合に失敗します。
    /// - [`body`] が空文字列の場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::message::models::Message,
    ///     user::models::UserId,
    /// };
    /// use uuid::Uuid;
    ///
    /// let user_id: UserId = Uuid::new_v4().into();
    ///
    /// let success_message = Message::try_new(user_id, "test message".to_string());
    /// let message_with_empty_body = Message::try_new(user_id, "".to_string());
    ///
    /// assert!(success_message.is_ok());
    /// assert!(message_with_empty_body.is_err());
    /// ```
    pub fn try_new(sender_id: UserId, body: String) -> Result<Self, DomainError> {
        if body.is_empty() {
            return Err(DomainError::EmptyMessageBody);
        }

        Ok(Self {
            id: MessageId::new(),
            sender_id,
            body,
            timestamp: Utc::now(),
        })
    }

    pub fn update_body(self, body: String) -> Result<Self, DomainError> {
        if body.is_empty() {
            return Err(DomainError::EmptyMessageBody);
        }

        Ok(Self { body, ..self })
    }
}
