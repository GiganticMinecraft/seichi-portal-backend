use async_trait::async_trait;
use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::{arbitrary_date_time, arbitrary_opt_date_time, arbitrary_with_size};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use errors::Error;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use typed_builder::TypedBuilder;
use types::Resolver;

use crate::{
    repository::form_repository::FormRepository,
    types::authorization_guard::{AuthorizationGuard, AuthorizationGuardDefinitions, Create, Read},
    user::models::{Role::Administrator, User},
};

pub type FormId = types::IntegerId<Form>;

#[async_trait]
impl<Repo: FormRepository + Sized + Sync> Resolver<Form, Error, Repo> for FormId {
    async fn resolve(&self, repo: &Repo) -> Result<Option<Form>, Error> {
        repo.get(self.to_owned()).await.map(Some)
    }
}

#[derive(Deserialize, Debug)]
pub struct OffsetAndLimit {
    #[serde(default)]
    pub offset: Option<i32>,
    #[serde(default)]
    pub limit: Option<i32>,
}

#[derive(Serialize, Debug)]
pub struct SimpleForm {
    pub id: FormId,
    pub title: FormTitle,
    pub description: FormDescription,
    pub response_period: ResponsePeriod,
    pub labels: Vec<Label>,
    pub answer_visibility: Visibility,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(DerivingVia, TypedBuilder, Clone, Getters, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: String), Deserialize(via: String))]
pub struct FormTitle {
    #[builder(setter(into))]
    title: String,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(TypedBuilder, Serialize, Deserialize, Debug, PartialEq)]
pub struct Form {
    #[serde(default)]
    #[builder(setter(into))]
    pub id: FormId,
    #[builder(setter(into))]
    pub title: FormTitle,
    #[builder(setter(into))]
    pub description: FormDescription,
    #[cfg_attr(test, proptest(strategy = "arbitrary_with_size(1..100)"))]
    #[serde(default)]
    pub questions: Vec<Question>,
    #[serde(default)]
    #[builder(setter(into))]
    pub metadata: FormMeta,
    #[serde(default)]
    pub settings: FormSettings,
    #[cfg_attr(test, proptest(strategy = "arbitrary_with_size(1..100)"))]
    #[serde(default)]
    pub labels: Vec<Label>,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(DerivingVia, TypedBuilder, Getters, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<String>), Deserialize(via: Option::<String>
))]
pub struct FormDescription {
    description: Option<String>,
}

pub type QuestionId = types::IntegerId<Question>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(TypedBuilder, Serialize, Deserialize, Clone, Getters, Debug, PartialEq)]
pub struct Question {
    #[serde(default)]
    pub id: Option<QuestionId>,
    pub title: String,
    pub description: Option<String>,
    pub question_type: QuestionType,
    #[cfg_attr(test, proptest(strategy = "arbitrary_with_size(1..100)"))]
    #[serde(default)]
    pub choices: Vec<String>,
    pub is_required: bool,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, EnumString, PartialOrd, PartialEq, Display,
)]
#[strum(ascii_case_insensitive)]
pub enum QuestionType {
    TEXT,
    SINGLE,
    MULTIPLE,
}

impl TryFrom<String> for QuestionType {
    type Error = errors::domain::DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Default, TypedBuilder, Debug, PartialEq)]
pub struct FormMeta {
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    #[serde(default = "chrono::Utc::now")]
    created_at: DateTime<Utc>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    #[serde(default = "chrono::Utc::now")]
    updated_at: DateTime<Utc>,
}

impl From<(DateTime<Utc>, DateTime<Utc>)> for FormMeta {
    fn from((created_at, updated_at): (DateTime<Utc>, DateTime<Utc>)) -> Self {
        Self {
            created_at,
            updated_at,
        }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct FormSettings {
    #[serde(default)]
    pub response_period: ResponsePeriod,
    #[serde(default)]
    pub webhook_url: WebhookUrl,
    #[serde(default)]
    pub default_answer_title: DefaultAnswerTitle,
    #[serde(default)]
    pub visibility: Visibility,
    #[serde(default)]
    pub answer_visibility: Visibility,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, Serialize(via: Option::<String>), Deserialize(via: Option::<String>))]
pub struct WebhookUrl {
    pub webhook_url: Option<String>,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(TypedBuilder, Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct ResponsePeriod {
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    pub start_at: Option<DateTime<Utc>>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    pub end_at: Option<DateTime<Utc>>,
}

impl ResponsePeriod {
    pub fn new(periods: Option<(DateTime<Utc>, DateTime<Utc>)>) -> Self {
        periods.map_or_else(ResponsePeriod::default, |(start_at, end_at)| {
            ResponsePeriod {
                start_at: Some(start_at),
                end_at: Some(end_at),
            }
        })
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, EnumString, Display, Default, PartialOrd, PartialEq)]
pub enum Visibility {
    PUBLIC,
    #[default]
    PRIVATE,
}

impl TryFrom<String> for Visibility {
    type Error = errors::domain::DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg_attr(test, derive(Arbitrary, Clone))]
#[derive(DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, Serialize(via: Option::<String>), Deserialize(via: Option::<String>))]
pub struct DefaultAnswerTitle {
    pub default_answer_title: Option<String>,
}

impl DefaultAnswerTitle {
    pub fn unwrap_or_default(&self) -> String {
        self.default_answer_title
            .to_owned()
            .unwrap_or("未設定".to_string())
    }
}

pub type AnswerId = types::IntegerId<FormAnswer>;

#[async_trait]
impl<Repo: FormRepository + Sized + Sync> Resolver<FormAnswer, Error, Repo> for AnswerId {
    async fn resolve(&self, repo: &Repo) -> Result<Option<FormAnswer>, Error> {
        repo.get_answers(self.to_owned()).await
    }
}

#[cfg_attr(test, derive(Arbitrary, Clone))]
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct FormAnswer {
    pub id: AnswerId,
    pub user: User,
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    pub timestamp: DateTime<Utc>,
    pub form_id: FormId,
    pub title: DefaultAnswerTitle,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct FormAnswerContent {
    pub answer_id: AnswerId,
    pub question_id: QuestionId,
    pub answer: String,
}

pub type CommentId = types::IntegerId<Comment>;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Comment {
    pub answer_id: AnswerId,
    pub comment_id: CommentId,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub commented_by: User,
}

pub type AnswerLabelId = types::IntegerId<AnswerLabel>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct AnswerLabel {
    pub id: AnswerLabelId,
    pub answer_id: AnswerId,
    pub name: String,
}

pub type LabelId = types::IntegerId<Label>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Label {
    pub id: LabelId,
    pub name: String,
}

pub type MessageId = types::Id<Message>;

#[cfg_attr(test, derive(Arbitrary, Clone))]
#[derive(Getters, Debug)]
pub struct Message {
    id: MessageId,
    related_answer: FormAnswer,
    sender: User,
    body: String,
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    timestamp: DateTime<Utc>,
}

impl AuthorizationGuardDefinitions<Message> for Message {
    /// [`Message`] の作成権限があるかどうかを判定します。
    ///
    /// 作成権限は以下の条件のどちらかを満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    /// - [`actor`] が関連する回答の回答者である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{FormAnswer, Message},
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
    /// let related_answer = FormAnswer {
    ///     id: Default::default(),
    ///     user: respondent.to_owned(),
    ///     timestamp: Default::default(),
    ///     form_id: Default::default(),
    ///     title: Default::default(),
    /// };
    ///
    /// let message = Message::new(
    ///     related_answer,
    ///     respondent.to_owned(),
    ///     "test message".to_string(),
    /// );
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
            || (actor.id == self.sender.id && self.related_answer.user.id == self.sender.id)
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
    ///     form::models::{FormAnswer, Message},
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
    /// let related_answer = FormAnswer {
    ///     id: Default::default(),
    ///     user: respondent.to_owned(),
    ///     timestamp: Default::default(),
    ///     form_id: Default::default(),
    ///     title: Default::default(),
    /// };
    ///
    /// let message = Message::new(
    ///     related_answer,
    ///     respondent.to_owned(),
    ///     "test message".to_string(),
    /// );
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
        actor.role == Administrator || self.related_answer.user.id == actor.id
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
    ///     form::models::{FormAnswer, Message},
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
    /// let related_answer = FormAnswer {
    ///     id: Default::default(),
    ///     user: respondent.to_owned(),
    ///     timestamp: Default::default(),
    ///     form_id: Default::default(),
    ///     title: Default::default(),
    /// };
    ///
    /// let message = Message::new(
    ///     related_answer,
    ///     respondent.to_owned(),
    ///     "test message".to_string(),
    /// );
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
    ///     form::models::{FormAnswer, Message},
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
    /// let related_answer = FormAnswer {
    ///     id: Default::default(),
    ///     user: respondent.to_owned(),
    ///     timestamp: Default::default(),
    ///     form_id: Default::default(),
    ///     title: Default::default(),
    /// };
    ///
    /// let message = Message::new(
    ///     related_answer,
    ///     respondent.to_owned(),
    ///     "test message".to_string(),
    /// );
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

impl From<Message> for AuthorizationGuard<Message, Create> {
    fn from(value: Message) -> Self {
        AuthorizationGuard::new(value)
    }
}

impl Message {
    pub fn new(related_answer: FormAnswer, sender: User, body: String) -> Self {
        Self {
            id: MessageId::new(),
            related_answer,
            sender,
            body,
            timestamp: Utc::now(),
        }
    }

    /// [`Message`] の各フィールドの値を受け取り、[`Message`] を生成します。
    ///
    /// # Examples
    /// ```
    /// use chrono::{DateTime, Utc};
    /// use domain::{
    ///     form::models::{AnswerId, FormAnswer, Message, MessageId},
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
    /// let related_answer = FormAnswer {
    ///     id: 1.into(),
    ///     user: user.to_owned(),
    ///     timestamp: Utc::now(),
    ///     form_id: Default::default(),
    ///     title: Default::default(),
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
        related_answer: FormAnswer,
        sender: User,
        body: String,
        timestamp: DateTime<Utc>,
    ) -> AuthorizationGuard<Self, Read> {
        AuthorizationGuard::new(Self {
            id,
            related_answer,
            sender,
            body,
            timestamp,
        })
        .into_read()
    }
}

#[cfg(test)]
mod test {
    use proptest::{prop_assert, prop_assert_eq, prop_assume, proptest};
    use serde_json::json;
    use test_case::test_case;

    use super::*;
    use crate::user::models::Role::StandardUser;

    #[test_case("TEXT"     => Ok(QuestionType::TEXT); "upper: TEXT")]
    #[test_case("text"     => Ok(QuestionType::TEXT); "lower: text")]
    #[test_case("SINGLE" => Ok(QuestionType::SINGLE); "upper: SINGLE")]
    #[test_case("single" => Ok(QuestionType::SINGLE); "lower: single")]
    #[test_case("MULTIPLE" => Ok(QuestionType::MULTIPLE); "upper: MULTIPLE")]
    #[test_case("multiple" => Ok(QuestionType::MULTIPLE); "lower: multiple")]
    fn string_to_question_type(input: &str) -> Result<QuestionType, errors::domain::DomainError> {
        input.to_owned().try_into()
    }

    proptest! {
        #[test]
        fn string_into_from_title(title: String) {
            let form_title: FormTitle = title.to_owned().into();
            prop_assert_eq!(form_title, FormTitle::builder().title(title).build());
        }
    }

    proptest! {
        #[test]
        fn serialize_from_id(id: i32) {
            let form_id: FormId = id.into();
            prop_assert_eq!(json!({"id":form_id}).to_string(), format!(r#"{{"id":{id}}}"#));
        }
    }

    proptest! {
        #[test]
        fn should_reject_message_from_unrelated_user(message_sender: User, form_answer: FormAnswer) {
            prop_assume!(message_sender.role == StandardUser);
            prop_assume!(form_answer.user.id != message_sender.id);

            let message: AuthorizationGuard<Message, Create> = Message::new(
                form_answer.to_owned(),
                message_sender.to_owned(),
                "test message".to_string(),
            ).into();

            let create_result = message.try_create(&message_sender, |_| {});

            prop_assert!(create_result.is_err());
        }
    }

    proptest! {
        #[test]
        fn should_accept_message_from_answer_posted_user(message: Message) {
            let message = Message {
                sender: User {
                    role: StandardUser,
                    ..message.related_answer.user.to_owned()
                },
                ..message
            };

            let message_guard: AuthorizationGuard<Message, Create> = message.to_owned().into();
            let create_result = message_guard.try_create(message.sender(), |_| {});

            prop_assert!(create_result.is_ok());
        }
    }

    proptest! {
        #[test]
        fn should_accept_message_from_administrator(message_sender: User, message: Message) {
            prop_assume!(message_sender.role == Administrator);

            let message_guard: AuthorizationGuard<Message, Create> = message.into();
            let create_result = message_guard.try_create(&message_sender, |_| {});

            prop_assert!(create_result.is_ok());
        }
    }
}
