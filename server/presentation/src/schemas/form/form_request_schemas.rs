use domain::form::question::models::{QuestionId, QuestionType};
use domain::form::{
    answer::{
        models::{AnswerLabelId, AnswerTitle},
        settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod},
    },
    models::{FormLabelId, FormTitle, Visibility, WebhookUrl},
};
use serde::{Deserialize, Deserializer};
use types::non_empty_string::NonEmptyString;

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct OffsetAndLimit {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormCreateSchema {
    #[schema(value_type = String)]
    pub title: FormTitle,
    pub description: String,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerSettingsSchema {
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub default_answer_title: Option<DefaultAnswerTitle>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub visibility: Option<AnswerVisibility>,
    #[serde(default)]
    #[schema(value_type = Option<ResponsePeriodInput>)]
    pub response_period: Option<ResponsePeriod>,
}

#[derive(utoipa::ToSchema)]
pub struct ResponsePeriodInput {
    pub start_at: Option<String>,
    pub end_at: Option<String>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormSettingsSchema {
    #[serde(default)]
    #[schema(value_type = Option<Option<String>>)]
    pub webhook_url: Option<WebhookUrlSchema>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub visibility: Option<Visibility>,
    #[serde(default)]
    pub answer_settings: Option<AnswerSettingsSchema>,
}

#[derive(Clone, Debug)]
pub struct WebhookUrlSchema(pub(crate) Option<WebhookUrl>);

impl<'de> Deserialize<'de> for WebhookUrlSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let url: Option<String> = Option::deserialize(deserializer)?;
        match url {
            Some(url) => {
                let non_empty_url =
                    NonEmptyString::try_new(url).map_err(serde::de::Error::custom)?;
                let webhook_url =
                    WebhookUrl::try_new(Some(non_empty_url)).map_err(serde::de::Error::custom)?;

                Ok(WebhookUrlSchema(Some(webhook_url)))
            }
            None => Ok(WebhookUrlSchema(None)),
        }
    }
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormUpdateSchema {
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub title: Option<FormTitle>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub settings: Option<FormSettingsSchema>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerContentSchema {
    #[schema(value_type = i32)]
    pub question_id: QuestionId,
    pub answer: String,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerCreateSchema {
    pub contents: Vec<AnswerContentSchema>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerUpdateSchema {
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub title: Option<AnswerTitle>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct QuestionSchema {
    #[schema(value_type = Option<i32>)]
    pub id: Option<QuestionId>,
    pub title: String,
    #[schema(value_type = String)]
    pub question_type: QuestionType,
    pub description: Option<String>,
    #[serde(default)]
    pub choices: Vec<String>,
    pub is_required: bool,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormQuestionPutSchema {
    #[serde(default)]
    pub questions: Vec<QuestionSchema>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct CommentPostSchema {
    pub content: NonEmptyString,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct CommentUpdateSchema {
    pub content: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormLabelCreateSchema {
    pub name: NonEmptyString,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormLabelUpdateSchema {
    pub name: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerLabelSchema {
    pub name: NonEmptyString,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerLabelUpdateSchema {
    pub name: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct ReplaceAnswerLabelSchema {
    #[schema(value_type = Vec<String>)]
    pub labels: Vec<AnswerLabelId>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct ReplaceFormLabelSchema {
    #[schema(value_type = Vec<String>)]
    pub labels: Vec<FormLabelId>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct PostedMessageSchema {
    pub body: String,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct MessageUpdateSchema {
    pub body: Option<String>,
}
