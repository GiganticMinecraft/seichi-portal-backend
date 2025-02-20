use crate::{
    form::{answer::models::AnswerEntry, message::models::Message},
    types::authorization_guard_with_context::AuthorizationGuardWithContextDefinitions,
    user::models::{Role::Administrator, User},
};

#[derive(Debug)]
pub struct MessageAuthorizationContext {
    pub related_answer_entry: AnswerEntry,
}

impl AuthorizationGuardWithContextDefinitions<MessageAuthorizationContext> for Message {
    /// [`Message`] の作成権限があるかどうかを判定します。
    ///
    /// 作成権限は以下の条件のどちらかを満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    /// - [`actor`] が関連する回答の回答者である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::{
    ///         answer::models::AnswerEntry,
    ///         message::{models::Message, service::MessageAuthorizationContext},
    ///     },
    ///     types::{
    ///         authorization_guard::AuthorizationGuardDefinitions,
    ///         authorization_guard_with_context::AuthorizationGuardWithContextDefinitions,
    ///     },
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
    ///     Default::default(),
    /// );
    ///
    /// let message = Message::try_new(
    ///     *related_answer.id(),
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
    /// let context = MessageAuthorizationContext {
    ///     related_answer_entry: related_answer,
    /// };
    ///
    /// assert!(message.can_create(&respondent, &context));
    /// assert!(message.can_create(&administrator, &context));
    /// assert!(!message.can_create(&unrelated_standard_user, &context));
    /// ```
    fn can_create(&self, actor: &User, context: &MessageAuthorizationContext) -> bool {
        if context.related_answer_entry.id() != self.related_answer_id() {
            tracing::error!("The related answer entry does not match the context.");

            return false;
        }

        actor.role == Administrator
            || (actor.id == self.sender().id
                && context.related_answer_entry.user().id == self.sender().id)
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
    ///     form::{
    ///         answer::models::AnswerEntry,
    ///         message::{models::Message, service::MessageAuthorizationContext},
    ///     },
    ///     types::{
    ///         authorization_guard::AuthorizationGuardDefinitions,
    ///         authorization_guard_with_context::AuthorizationGuardWithContextDefinitions,
    ///     },
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
    ///     Default::default(),
    /// );
    ///
    /// let message = Message::try_new(
    ///     *related_answer.id(),
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
    /// let context = MessageAuthorizationContext {
    ///     related_answer_entry: related_answer,
    /// };
    ///
    /// assert!(message.can_read(&respondent, &context));
    /// assert!(message.can_read(&administrator, &context));
    /// assert!(!message.can_read(&unrelated_standard_user, &context));
    /// ```
    fn can_read(&self, actor: &User, context: &MessageAuthorizationContext) -> bool {
        if context.related_answer_entry.id() != self.related_answer_id() {
            tracing::error!("The related answer entry does not match the context.");

            return false;
        }

        actor.role == Administrator || context.related_answer_entry.user().id == actor.id
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
    ///     form::{
    ///         answer::models::AnswerEntry,
    ///         message::{models::Message, service::MessageAuthorizationContext},
    ///     },
    ///     types::{
    ///         authorization_guard::AuthorizationGuardDefinitions,
    ///         authorization_guard_with_context::AuthorizationGuardWithContextDefinitions,
    ///     },
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
    ///     Default::default(),
    /// );
    ///
    /// let message = Message::try_new(
    ///     *related_answer.id(),
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
    /// let context = MessageAuthorizationContext {
    ///     related_answer_entry: related_answer,
    /// };
    ///
    /// assert!(message.can_update(&respondent, &context));
    /// assert!(!message.can_update(&administrator, &context));
    /// assert!(!message.can_update(&unrelated_standard_user, &context));
    /// ```
    fn can_update(&self, actor: &User, _context: &MessageAuthorizationContext) -> bool {
        self.sender().id == actor.id
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
    ///     form::{
    ///         answer::models::AnswerEntry,
    ///         message::{models::Message, service::MessageAuthorizationContext},
    ///     },
    ///     types::{
    ///         authorization_guard::AuthorizationGuardDefinitions,
    ///         authorization_guard_with_context::AuthorizationGuardWithContextDefinitions,
    ///     },
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
    ///     Default::default(),
    /// );
    ///
    /// let message = Message::try_new(
    ///     *related_answer.id(),
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
    /// let context = MessageAuthorizationContext {
    ///     related_answer_entry: related_answer,
    /// };
    ///
    /// assert!(message.can_delete(&respondent, &context));
    /// assert!(!message.can_delete(&administrator, &context));
    /// assert!(!message.can_delete(&unrelated_standard_user, &context));
    /// ```
    fn can_delete(&self, actor: &User, _context: &MessageAuthorizationContext) -> bool {
        self.sender().id == actor.id
    }
}
