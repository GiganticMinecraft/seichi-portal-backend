use chrono::{DateTime, Utc};
use derive_getters::Getters;
use errors::domain::DomainError;

use crate::{
    form::answer::models::AnswerEntry,
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Role::Administrator, User},
};

pub type MessageId = types::Id<Message>;

#[derive(Getters, PartialEq, Debug)]
pub struct Message {
    id: MessageId,
    related_answer: AnswerEntry,
    sender: User,
    body: String,
    timestamp: DateTime<Utc>,
}

impl AuthorizationGuardDefinitions for Message {
    /// [`Message`] の作成権限があるかどうかを判定します。
    ///
    /// 作成権限は以下の条件のどちらかを満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    /// - [`actor`] が関連する回答の回答者である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::{answer::models::AnswerEntry, message::models::Message},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{Role, User},
    /// };
    /// use uuid::Uuid;
    ///
    /// let respondent = User {
    ///     name: "respondent".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// let related_answer = AnswerEntry::new(
    ///     respondent.to_owned(),
    ///     Default::default(),
    ///     Default::default(),
    /// );
    ///
    /// let message = Message::try_new(
    ///     related_answer,
    ///     respondent.to_owned(),
    ///     "test message".to_string(),
    /// )
    /// .unwrap();
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let unrelated_standard_user = User {
    ///     name: "unrelated_user".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// assert!(message.can_create(&respondent));
    /// assert!(message.can_create(&administrator));
    /// assert!(!message.can_create(&unrelated_standard_user));
    /// ```
    fn can_create(&self, actor: &User) -> bool {
        actor.role == Administrator
            || (actor.id == self.sender.id && self.related_answer.user().id == self.sender.id)
    }

    /// [`Message`] の読み取り権限があるかどうかを判定します。
    ///
    /// 読み取り権限は以下の条件のどちらかを満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    /// - [`actor`] が関連する回答の回答者である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::{answer::models::AnswerEntry, message::models::Message},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{Role, User},
    /// };
    /// use uuid::Uuid;
    ///
    /// let respondent = User {
    ///     name: "respondent".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// let related_answer = AnswerEntry::new(
    ///     respondent.to_owned(),
    ///     Default::default(),
    ///     Default::default(),
    /// );
    ///
    /// let message = Message::try_new(
    ///     related_answer,
    ///     respondent.to_owned(),
    ///     "test message".to_string(),
    /// )
    /// .unwrap();
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let unrelated_standard_user = User {
    ///     name: "unrelated_user".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// assert!(message.can_read(&respondent));
    /// assert!(message.can_read(&administrator));
    /// assert!(!message.can_read(&unrelated_standard_user));
    /// ```
    fn can_read(&self, actor: &User) -> bool {
        actor.role == Administrator || self.related_answer.user().id == actor.id
    }

    /// [`Message`] の更新権限があるかどうかを判定します。
    ///
    /// 更新権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] がメッセージの送信者の場合
    ///
    /// [`actor`] が [`Administrator`] である場合に更新権限が与えられないのは、
    /// メッセージの送信者が意図しない更新が行われることを防ぐためです。
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::{answer::models::AnswerEntry, message::models::Message},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{Role, User},
    /// };
    /// use uuid::Uuid;
    ///
    /// let respondent = User {
    ///     name: "respondent".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// let related_answer = AnswerEntry::new(
    ///     respondent.to_owned(),
    ///     Default::default(),
    ///     Default::default(),
    /// );
    ///
    /// let message = Message::try_new(
    ///     related_answer,
    ///     respondent.to_owned(),
    ///     "test message".to_string(),
    /// )
    /// .unwrap();
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let unrelated_standard_user = User {
    ///     name: "unrelated_user".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// assert!(message.can_update(&respondent));
    /// assert!(!message.can_update(&administrator));
    /// assert!(!message.can_update(&unrelated_standard_user));
    /// ```
    fn can_update(&self, actor: &User) -> bool {
        self.sender.id == actor.id
    }

    /// [`Message`] の削除権限があるかどうかを判定します。
    ///
    /// 削除権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] がメッセージの送信者の場合
    ///
    /// [`actor`] が [`Administrator`] である場合に更新権限が与えられないのは、
    /// メッセージの送信者が意図しない削除(メッセージ内容の改変)が行われることを防ぐためです。
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::{answer::models::AnswerEntry, message::models::Message},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{Role, User},
    /// };
    /// use uuid::Uuid;
    ///
    /// let respondent = User {
    ///     name: "respondent".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// let related_answer = AnswerEntry::new(
    ///     respondent.to_owned(),
    ///     Default::default(),
    ///     Default::default(),
    /// );
    ///
    /// let message = Message::try_new(
    ///     related_answer,
    ///     respondent.to_owned(),
    ///     "test message".to_string(),
    /// )
    /// .unwrap();
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let unrelated_standard_user = User {
    ///     name: "unrelated_user".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// assert!(message.can_delete(&respondent));
    /// assert!(!message.can_delete(&administrator));
    /// assert!(!message.can_delete(&unrelated_standard_user));
    /// ```
    fn can_delete(&self, actor: &User) -> bool {
        self.sender.id == actor.id
    }
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
    /// let related_answer = AnswerEntry::new(user.to_owned(), Default::default(), Default::default());
    ///
    /// let success_message =
    ///     Message::try_new(related_answer, user.to_owned(), "test message".to_string());
    ///
    /// let related_answer = AnswerEntry::new(user.to_owned(), Default::default(), Default::default());
    /// let message_with_empty_body = Message::try_new(related_answer, user, "".to_string());
    ///
    /// assert!(success_message.is_ok());
    /// assert!(message_with_empty_body.is_err());
    /// ```
    pub fn try_new(
        related_answer: AnswerEntry,
        sender: User,
        body: String,
    ) -> Result<Self, DomainError> {
        if body.is_empty() {
            return Err(DomainError::EmptyMessageBody);
        }

        Ok(Self {
            id: MessageId::new(),
            related_answer,
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
    /// let related_answer = AnswerEntry::new(user.to_owned(), Default::default(), Default::default());
    ///
    /// unsafe {
    ///     let message = Message::from_raw_parts(
    ///         MessageId::new(),
    ///         related_answer,
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
        related_answer: AnswerEntry,
        sender: User,
        body: String,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            related_answer,
            sender,
            body,
            timestamp,
        }
    }
}
