use domain::account::models::UserGroupId;
use domain::form::question::{ChoiceId, QuestionId, QuestionType, TemplateKey};
use domain::form::{
    answer::{AnswerLabelId, AnswerTitle},
    models::{
        AnswerAcceptancePeriod, AnswerVisibility, DefaultAnswerTitle, DiscordWebhookUrl,
        FormLabelId, FormTitle, Visibility,
    },
};
use serde::{Deserialize, Deserializer};
use types::non_empty_string::NonEmptyString;
use types::non_empty_vec::NonEmptyVec;

#[derive(Deserialize, Debug, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FormListQuery {
    /// Maximum number of forms to return
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Cursor returned by the previous page
    pub cursor: Option<String>,
}

#[derive(Deserialize, Debug, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ArchivedFormListQuery {
    /// Maximum number of forms to return
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Cursor returned by the previous page
    pub cursor: Option<String>,
    pub query: Option<String>,
}

#[derive(Deserialize, Debug, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AnswerListQuery {
    /// Maximum number of answers to return
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Cursor returned by the previous page
    pub cursor: Option<String>,
}

#[derive(Deserialize, Debug, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct HistoryListQuery {
    /// Maximum number of history entries to return
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Cursor returned by the previous page
    pub cursor: Option<String>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormCreateSchema {
    #[schema(value_type = String)]
    pub title: FormTitle,
    pub description: String,
    #[serde(default)]
    pub settings: Option<FormSettingsSchema>,
    #[schema(value_type = Vec<QuestionSchema>, min_items = 1)]
    pub questions: NonEmptyVec<QuestionSchema>,
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
    #[schema(value_type = Option<AnswerAcceptancePeriodInput>)]
    pub acceptance_period: Option<AnswerAcceptancePeriod>,
    #[serde(default)]
    #[schema(value_type = Option<Vec<String>>)]
    pub answer_group_ids: Option<Vec<UserGroupId>>,
}

#[derive(utoipa::ToSchema)]
pub struct AnswerAcceptancePeriodInput {
    pub start_at: Option<String>,
    pub end_at: Option<String>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormSettingsSchema {
    #[serde(default)]
    #[schema(value_type = Option<Option<String>>)]
    pub discord_webhook_url: Option<DiscordWebhookUrlSchema>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub visibility: Option<Visibility>,
    #[serde(default)]
    #[schema(value_type = Option<Vec<String>>)]
    pub allowed_group_ids: Option<Vec<UserGroupId>>,
    #[serde(default)]
    pub allow_temporary_answers: Option<bool>,
    #[serde(default)]
    pub answer_settings: Option<AnswerSettingsSchema>,
}

#[derive(Clone, Debug)]
pub struct DiscordWebhookUrlSchema(pub(crate) Option<DiscordWebhookUrl>);

impl<'de> Deserialize<'de> for DiscordWebhookUrlSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let url: Option<String> = Option::deserialize(deserializer)?;
        match url {
            Some(url) => {
                let non_empty_url =
                    NonEmptyString::try_new(url).map_err(serde::de::Error::custom)?;
                let discord_webhook_url = DiscordWebhookUrl::try_new(Some(non_empty_url))
                    .map_err(serde::de::Error::custom)?;

                Ok(DiscordWebhookUrlSchema(Some(discord_webhook_url)))
            }
            None => Ok(DiscordWebhookUrlSchema(None)),
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
    /// When provided, replaces the full set of question definitions under the form.
    /// Omit this field to leave existing questions unchanged.
    #[serde(default)]
    pub questions: Option<Vec<QuestionSchema>>,
    /// When provided, replaces the full set of labels attached to the form.
    /// Omit this field to leave existing labels unchanged.
    #[serde(default)]
    #[schema(value_type = Option<Vec<String>>)]
    pub labels: Option<Vec<FormLabelId>>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerContentSchema {
    #[schema(value_type = String, format = "uuid")]
    pub question_id: QuestionId,
    pub answer: String,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerCreateSchema {
    pub contents: Vec<AnswerContentSchema>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct TemporaryUserCreateSchema {
    pub name: NonEmptyString,
    pub contact_text: NonEmptyString,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct TemporaryAnswerCreateSchema {
    pub temporary_user: TemporaryUserCreateSchema,
    pub contents: Vec<AnswerContentSchema>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerUpdateSchema {
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub title: Option<AnswerTitle>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct ChoiceSchema {
    #[schema(value_type = Option<i32>)]
    pub id: Option<ChoiceId>,
    pub position: u16,
    #[schema(value_type = String)]
    pub label: NonEmptyString,
}

impl From<ChoiceSchema> for domain::form::question::Choice {
    fn from(choice: ChoiceSchema) -> Self {
        Self::new(choice.id, choice.position, choice.label)
    }
}

#[derive(Clone, Deserialize, Debug, utoipa::ToSchema)]
pub struct QuestionDefinitionSchema {
    #[schema(value_type = Option<String>, format = "uuid")]
    pub id: Option<QuestionId>,
    #[schema(value_type = String)]
    pub template_key: TemplateKey,
    pub position: u16,
    #[schema(value_type = String)]
    pub title: NonEmptyString,
    #[schema(value_type = Option<String>)]
    pub description: Option<NonEmptyString>,
    pub is_required: bool,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TextQuestionSchema {
    #[serde(flatten)]
    pub definition: QuestionDefinitionSchema,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectQuestionSchema {
    #[serde(flatten)]
    pub definition: QuestionDefinitionSchema,
    pub choices: Vec<ChoiceSchema>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
#[serde(tag = "question_type")]
pub enum QuestionSchema {
    #[serde(rename = "Text")]
    Text(TextQuestionSchema),
    #[serde(rename = "SingleChoice")]
    SingleChoice(SelectQuestionSchema),
    #[serde(rename = "MultipleChoice")]
    MultipleChoice(SelectQuestionSchema),
}

impl QuestionSchema {
    pub fn definition(&self) -> &QuestionDefinitionSchema {
        match self {
            Self::Text(question) => &question.definition,
            Self::SingleChoice(question) | Self::MultipleChoice(question) => &question.definition,
        }
    }

    pub fn question_type(&self) -> QuestionType {
        match self {
            Self::Text(_) => QuestionType::Text,
            Self::SingleChoice(_) => QuestionType::SingleChoice,
            Self::MultipleChoice(_) => QuestionType::MultipleChoice,
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        QuestionType,
        QuestionDefinitionSchema,
        Option<Vec<ChoiceSchema>>,
    ) {
        match self {
            Self::Text(question) => (QuestionType::Text, question.definition, None),
            Self::SingleChoice(question) => (
                QuestionType::SingleChoice,
                question.definition,
                Some(question.choices),
            ),
            Self::MultipleChoice(question) => (
                QuestionType::MultipleChoice,
                question.definition,
                Some(question.choices),
            ),
        }
    }

    pub fn into_choices(self) -> Option<Vec<ChoiceSchema>> {
        match self {
            Self::Text(_) => None,
            Self::SingleChoice(question) | Self::MultipleChoice(question) => Some(question.choices),
        }
    }
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
pub struct PostedMessageSchema {
    #[schema(value_type = String, min_length = 1)]
    pub body: NonEmptyString,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct MessageUpdateSchema {
    #[schema(value_type = Option<String>, min_length = 1)]
    pub body: Option<NonEmptyString>,
}
