use chrono::{DateTime, Utc};
use derive_getters::Getters;
use errors::domain::DomainError;

use crate::{form::answer::models::AnswerId, user::models::User};

pub type MessageId = types::Id<Message>;

#[derive(Getters, PartialEq, Debug)]
pub struct Message {
    id: MessageId,
    related_answer_id: AnswerId,
    sender: User,
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
    ///     form::{answer::models::AnswerEntry, message::models::Message},
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
    /// let related_answer = AnswerEntry::new(
    ///     user.to_owned(),
    ///     Default::default(),
    ///     Default::default(),
    ///     Default::default(),
    /// );
    ///
    /// let success_message = Message::try_new(
    ///     *related_answer.id(),
    ///     user.to_owned(),
    ///     "test message".to_string(),
    /// );
    ///
    /// let related_answer = AnswerEntry::new(
    ///     user.to_owned(),
    ///     Default::default(),
    ///     Default::default(),
    ///     Default::default(),
    /// );
    /// let message_with_empty_body = Message::try_new(*related_answer.id(), user, "".to_string());
    ///
    /// assert!(success_message.is_ok());
    /// assert!(message_with_empty_body.is_err());
    /// ```
    pub fn try_new(
        related_answer_id: AnswerId,
        sender: User,
        body: String,
    ) -> Result<Self, DomainError> {
        if body.is_empty() {
            return Err(DomainError::EmptyMessageBody);
        }

        Ok(Self {
            id: MessageId::new(),
            related_answer_id,
            sender,
            body,
            timestamp: Utc::now(),
        })
    }

    /// [`Message`] の各フィールドの値を受け取り、[`Message`] を生成します。
    ///
    /// # Examples
    /// ```
    /// use chrono::{DateTime, Utc};
    /// use domain::{
    ///     form::{
    ///         answer::models::AnswerEntry,
    ///         message::models::{Message, MessageId},
    ///     },
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
    /// let related_answer = AnswerEntry::new(
    ///     user.to_owned(),
    ///     Default::default(),
    ///     Default::default(),
    ///     Default::default(),
    /// );
    ///
    /// unsafe {
    ///     let message = Message::from_raw_parts(
    ///         MessageId::new(),
    ///         *related_answer.id(),
    ///         user,
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
        related_answer_id: AnswerId,
        sender: User,
        body: String,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            related_answer_id,
            sender,
            body,
            timestamp,
        }
    }
}
