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

use crate::{repository::form_repository::FormRepository, user::models::User};

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

#[cfg_attr(test, derive(Arbitrary))]
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

pub type AnswerId = types::Id<FormAnswer>;

#[async_trait]
impl<Repo: FormRepository + Sized + Sync> Resolver<FormAnswer, Error, Repo> for AnswerId {
    async fn resolve(&self, repo: &Repo) -> Result<Option<FormAnswer>, Error> {
        repo.get_answers(self.to_owned()).await
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct FormAnswer {
    pub id: AnswerId,
    pub user: User,
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
}
