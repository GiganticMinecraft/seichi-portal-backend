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

#[derive(Deserialize, Debug)]
pub struct OffsetAndLimit {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct FormCreateSchema {
    pub title: FormTitle,
    pub description: String,
}

#[derive(Deserialize, Debug)]
pub struct AnswerSettingsSchema {
    #[serde(default)]
    pub default_answer_title: Option<DefaultAnswerTitle>,
    #[serde(default)]
    pub visibility: Option<AnswerVisibility>,
    #[serde(default)]
    pub response_period: Option<ResponsePeriod>,
}

#[derive(Deserialize, Debug)]
pub struct FormSettingsSchema {
    #[serde(default)]
    pub webhook_url: Option<WebhookUrlSchema>,
    #[serde(default)]
    pub visibility: Option<Visibility>,
    #[serde(default)]
    pub answer_settings: Option<AnswerSettingsSchema>,
}

#[derive(Debug)]
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

#[derive(Deserialize, Debug)]
pub struct FormUpdateSchema {
    #[serde(default)]
    pub title: Option<FormTitle>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub settings: Option<FormSettingsSchema>,
}

#[derive(Deserialize, Debug)]
pub struct AnswerContentSchema {
    pub question_id: QuestionId,
    pub answer: String,
}

#[derive(Deserialize, Debug)]
pub struct AnswerCreateSchema {
    pub contents: Vec<AnswerContentSchema>,
}

#[derive(Deserialize, Debug)]
pub struct AnswerUpdateSchema {
    #[serde(default)]
    pub title: Option<AnswerTitle>,
}

#[derive(Deserialize, Debug)]
pub struct QuestionSchema {
    pub id: Option<QuestionId>,
    pub title: String,
    pub question_type: QuestionType,
    pub description: Option<String>,
    #[serde(default)]
    pub choices: Vec<String>,
    pub is_required: bool,
}

#[derive(Deserialize, Debug)]
pub struct FormQuestionPutSchema {
    #[serde(default)]
    pub questions: Vec<QuestionSchema>,
}

#[derive(Deserialize, Debug)]
pub struct CommentPostSchema {
    pub content: NonEmptyString,
}

#[derive(Deserialize, Debug)]
pub struct CommentUpdateSchema {
    pub content: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug)]
pub struct FormLabelCreateSchema {
    pub name: NonEmptyString,
}

#[derive(Deserialize, Debug)]
pub struct FormLabelUpdateSchema {
    pub name: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug)]
pub struct AnswerLabelSchema {
    pub name: NonEmptyString,
}

#[derive(Deserialize, Debug)]
pub struct AnswerLabelUpdateSchema {
    pub name: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug)]
pub struct ReplaceAnswerLabelSchema {
    pub labels: Vec<AnswerLabelId>,
}

#[derive(Deserialize, Debug)]
pub struct ReplaceFormLabelSchema {
    pub labels: Vec<FormLabelId>,
}

#[derive(Deserialize, Debug)]
pub struct PostedMessageSchema {
    pub body: String,
}

#[derive(Deserialize, Debug)]
pub struct MessageUpdateSchema {
    pub body: Option<String>,
}
