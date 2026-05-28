use chrono::{DateTime, Utc};
use derive_getters::Getters;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};

use crate::user::models::UserId;

pub type MessageId = types::Id<Message>;

#[derive(Getters, Clone, PartialEq, Debug, Serialize, Deserialize)]
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

    /// [`Message`] の各フィールドの値を受け取り、[`Message`] を生成します。
    ///
    /// # Examples
    /// ```
    /// use chrono::Utc;
    /// use domain::{
    ///     form::message::models::{Message, MessageId},
    ///     user::models::UserId,
    /// };
    /// use uuid::Uuid;
    ///
    /// let user_id: UserId = Uuid::new_v4().into();
    ///
    /// unsafe {
    ///     let message = Message::from_raw_parts(
    ///         MessageId::new(),
    ///         user_id,
    ///         "test message".to_string(),
    ///         Utc::now(),
    ///     );
    /// }
    /// ```
    ///
    /// # Safety
    /// この関数は [`Message`] のバリデーションをスキップするため、
    /// データベースからすでにバリデーションされているデータを読み出すときなど、
    /// データの信頼性が保証されている場合にのみ使用してください。
    pub unsafe fn from_raw_parts(
        id: MessageId,
        sender_id: UserId,
        body: String,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            sender_id,
            body,
            timestamp,
        }
    }
}
