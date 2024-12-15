use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::{arbitrary_date_time, arbitrary_opt_date_time, arbitrary_with_size};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

use crate::{
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Role::Administrator, User},
};

pub type FormId = types::Id<Form>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: String), Deserialize(via: String))]
pub struct FormTitle(String);

impl FormTitle {
    pub fn new(title: String) -> Self {
        Self(title)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Default, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<String>), Deserialize(via: Option::<String>
))]
pub struct FormDescription(Option<String>);

impl FormDescription {
    pub fn new(description: Option<String>) -> Self {
        Self(description)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Default, Debug, PartialEq)]
pub struct FormSettings {
    #[serde(default)]
    response_period: ResponsePeriod,
    #[serde(default)]
    webhook_url: WebhookUrl,
    #[serde(default)]
    default_answer_title: DefaultAnswerTitle,
    #[serde(default)]
    visibility: Visibility,
    #[serde(default)]
    answer_visibility: Visibility,
}

impl FormSettings {
    pub fn new() -> Self {
        Self {
            response_period: ResponsePeriod::new(None, None),
            webhook_url: WebhookUrl::new(None),
            default_answer_title: DefaultAnswerTitle::new(None),
            visibility: Visibility::PUBLIC,
            answer_visibility: Visibility::PRIVATE,
        }
    }

    pub fn from_raw_parts(
        response_period: ResponsePeriod,
        webhook_url: WebhookUrl,
        default_answer_title: DefaultAnswerTitle,
        visibility: Visibility,
        answer_visibility: Visibility,
    ) -> Self {
        Self {
            response_period,
            webhook_url,
            default_answer_title,
            visibility,
            answer_visibility,
        }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<String>), Deserialize(via: Option::<String>
))]
pub struct WebhookUrl(Option<String>);

impl WebhookUrl {
    pub fn new(url: Option<String>) -> Self {
        Self(url)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Default, Debug, PartialEq)]
pub struct ResponsePeriod {
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    start_at: Option<DateTime<Utc>>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    end_at: Option<DateTime<Utc>>,
}

impl ResponsePeriod {
    pub fn new(start_at: Option<DateTime<Utc>>, end_at: Option<DateTime<Utc>>) -> Self {
        Self { start_at, end_at }
    }

    pub fn from_raw_parts(start_at: Option<DateTime<Utc>>, end_at: Option<DateTime<Utc>>) -> Self {
        Self { start_at, end_at }
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
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<String>), Deserialize(via: Option::<String>
))]
pub struct DefaultAnswerTitle(Option<String>);

impl DefaultAnswerTitle {
    pub fn new(title: Option<String>) -> Self {
        Self(title)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct Form {
    #[serde(default)]
    id: FormId,
    title: FormTitle,
    #[serde(default)]
    description: FormDescription,
    #[serde(default)]
    metadata: FormMeta,
    #[serde(default)]
    settings: FormSettings,
}

impl Form {
    pub fn new(title: FormTitle, description: FormDescription) -> Self {
        Self {
            id: FormId::new(),
            title,
            description,
            metadata: FormMeta::new(),
            settings: FormSettings::new(),
        }
    }

    pub fn from_raw_parts(
        id: FormId,
        title: FormTitle,
        description: FormDescription,
        metadata: FormMeta,
        settings: FormSettings,
    ) -> Self {
        Self {
            id,
            title,
            description,
            metadata,
            settings,
        }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct FormMeta {
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    #[serde(default = "chrono::Utc::now")]
    created_at: DateTime<Utc>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    #[serde(default = "chrono::Utc::now")]
    updated_at: DateTime<Utc>,
}

impl FormMeta {
    pub fn new() -> Self {
        Self {
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn from_raw_parts(created_at: DateTime<Utc>, updated_at: DateTime<Utc>) -> Self {
        Self {
            created_at,
            updated_at,
        }
    }
}

pub type QuestionId = types::IntegerId<Question>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Getters, Debug, PartialEq)]
pub struct Question {
    #[serde(default)]
    pub id: Option<QuestionId>,
    pub form_id: FormId,
    pub title: String,
    pub description: Option<String>,
    pub question_type: QuestionType,
    #[cfg_attr(test, proptest(strategy = "arbitrary_with_size(1..100)"))]
    #[serde(default)]
    pub choices: Vec<String>,
    pub is_required: bool,
}

impl Question {
    pub fn new(
        id: Option<QuestionId>,
        form_id: FormId,
        title: String,
        description: Option<String>,
        question_type: QuestionType,
        choices: Vec<String>,
        is_required: bool,
    ) -> Self {
        Self {
            id,
            form_id,
            title,
            description,
            question_type,
            choices,
            is_required,
        }
    }

    pub fn from_raw_parts(
        id: Option<QuestionId>,
        form_id: FormId,
        title: String,
        description: Option<String>,
        question_type: QuestionType,
        choices: Vec<String>,
        is_required: bool,
    ) -> Self {
        Self {
            id,
            form_id,
            title,
            description,
            question_type,
            choices,
            is_required,
        }
    }
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
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

pub type AnswerId = types::IntegerId<FormAnswer>;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct FormAnswer {
    pub id: AnswerId,
    pub user: User,
    pub timestamp: DateTime<Utc>,
    pub form_id: FormId,
    pub title: Option<String>,
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

#[derive(Getters, PartialEq, Debug)]
pub struct Message {
    id: MessageId,
    related_answer: FormAnswer,
    sender: User,
    body: String,
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
    ///     form::models::{FormAnswer, Message},
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
    ///     timestamp: Default::default(),
    ///     form_id: Default::default(),
    ///     title: Default::default(),
    /// };
    ///
    /// let success_message =
    ///     Message::try_new(related_answer, user.to_owned(), "test message".to_string());
    ///
    /// let related_answer = FormAnswer {
    ///     id: 1.into(),
    ///     user: user.to_owned(),
    ///     timestamp: Default::default(),
    ///     form_id: Default::default(),
    ///     title: Default::default(),
    /// };
    /// let message_with_empty_body = Message::try_new(related_answer, user, "".to_string());
    ///
    /// assert!(success_message.is_ok());
    /// assert!(message_with_empty_body.is_err());
    /// ```
    pub fn try_new(
        related_answer: FormAnswer,
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
        related_answer: FormAnswer,
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

#[cfg(test)]
mod test {
    use proptest::{prop_assert_eq, proptest};
    use serde_json::json;
    use test_case::test_case;

    use super::*;

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
        fn serialize_from_id(id: i32) {
            let form_id: FormId = id.into();
            prop_assert_eq!(json!({"id":form_id}).to_string(), format!(r#"{{"id":{id}}}"#));
        }
    }
}
