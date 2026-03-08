use chrono::{DateTime, Utc};
use domain::form::answer::settings::models::AnswerSettings;
use domain::form::{
    answer::settings::models::DefaultAnswerTitle,
    models::{FormDescription, FormId, FormLabel, FormMeta, FormSettings, FormTitle, Visibility},
    question::models::Question,
};
use itertools::Itertools;
use serde::Serialize;
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct ResponsePeriodSchema {
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub enum AnswerVisibility {
    #[serde(rename = "PUBLIC")]
    Public,
    #[serde(rename = "PRIVATE")]
    Private,
}

impl From<domain::form::answer::settings::models::AnswerVisibility> for AnswerVisibility {
    fn from(val: domain::form::answer::settings::models::AnswerVisibility) -> Self {
        match val {
            domain::form::answer::settings::models::AnswerVisibility::PUBLIC => {
                AnswerVisibility::Public
            }
            domain::form::answer::settings::models::AnswerVisibility::PRIVATE => {
                AnswerVisibility::Private
            }
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerSettingsSchema {
    #[schema(value_type = Option<String>)]
    pub default_answer_title: DefaultAnswerTitle,
    pub visibility: AnswerVisibility,
    pub response_period: ResponsePeriodSchema,
}

impl AnswerSettingsSchema {
    pub fn from_answer_settings_ref(answer_settings: &AnswerSettings) -> Self {
        Self {
            default_answer_title: answer_settings.default_answer_title().to_owned(),
            visibility: answer_settings.visibility().to_owned().into(),
            response_period: ResponsePeriodSchema {
                start_at: answer_settings.response_period().start_at().to_owned(),
                end_at: answer_settings.response_period().end_at().to_owned(),
            },
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormSettingsSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<Option<String>>,
    #[schema(value_type = String)]
    pub visibility: Visibility,
    pub answer_settings: AnswerSettingsSchema,
}

impl FormSettingsSchema {
    pub fn from_settings_ref(actor: &domain::user::models::User, settings: &FormSettings) -> Self {
        FormSettingsSchema {
            webhook_url: settings
                .webhook_url(actor)
                .ok()
                .map(|url| url.to_owned().into_inner().map(NonEmptyString::into_inner)),
            visibility: settings.visibility().to_owned(),
            answer_settings: AnswerSettingsSchema::from_answer_settings_ref(
                settings.answer_settings(),
            ),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormMetaSchema {
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl FormMetaSchema {
    pub fn from_meta_ref(meta: &FormMeta) -> Self {
        FormMetaSchema {
            created_at: meta.created_at,
            updated_at: meta.updated_at,
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormSchema {
    #[schema(value_type = String, format = "uuid")]
    pub id: FormId,
    #[schema(value_type = String)]
    pub title: FormTitle,
    #[schema(value_type = String)]
    pub description: FormDescription,
    pub settings: FormSettingsSchema,
    pub metadata: FormMetaSchema,
    #[schema(value_type = Vec<QuestionResponseSchema>)]
    pub questions: Vec<Question>,
    #[schema(value_type = Vec<FormLabelResponseSchema>)]
    pub labels: Vec<FormLabel>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct QuestionResponseSchema {
    pub id: Option<i32>,
    pub form_id: String,
    pub title: String,
    pub description: Option<String>,
    pub question_type: String,
    pub choices: Vec<String>,
    pub is_required: bool,
}

impl From<domain::form::question::models::Question> for QuestionResponseSchema {
    fn from(val: domain::form::question::models::Question) -> Self {
        QuestionResponseSchema {
            id: val.id.map(|id| id.into_inner()),
            form_id: val.form_id.into_inner().to_string(),
            title: val.title,
            description: val.description,
            question_type: val.question_type.to_string(),
            choices: val.choices,
            is_required: val.is_required,
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormLabelResponseSchema {
    pub id: String,
    pub name: String,
}

impl From<domain::form::models::FormLabel> for FormLabelResponseSchema {
    fn from(val: domain::form::models::FormLabel) -> Self {
        FormLabelResponseSchema {
            id: val.id().to_owned().into_inner().to_string(),
            name: val.name().to_string(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerLabelResponseSchema {
    pub id: String,
    pub name: String,
}

impl From<domain::form::answer::models::AnswerLabel> for AnswerLabelResponseSchema {
    fn from(val: domain::form::answer::models::AnswerLabel) -> Self {
        AnswerLabelResponseSchema {
            id: val.id().to_owned().into_inner().to_string(),
            name: val.name().to_string(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct PutQuestionsResponseSchema {
    pub questions: Vec<QuestionResponseSchema>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub enum Role {
    #[serde(rename = "STANDARD_USER")]
    StandardUser,
    #[serde(rename = "ADMINISTRATOR")]
    Administrator,
}

impl From<domain::user::models::Role> for Role {
    fn from(val: domain::user::models::Role) -> Self {
        match val {
            domain::user::models::Role::StandardUser => Role::StandardUser,
            domain::user::models::Role::Administrator => Role::Administrator,
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct User {
    uuid: String,
    name: String,
    role: Role,
}

impl From<domain::user::models::User> for User {
    fn from(val: domain::user::models::User) -> Self {
        User {
            uuid: val.id.to_string(),
            name: val.name,
            role: val.role.into(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerContent {
    question_id: i32,
    answer: String,
}

impl AnswerContent {
    pub fn from_ref(val: &domain::form::answer::models::FormAnswerContent) -> Self {
        AnswerContent {
            question_id: val.question_id.into_inner(),
            answer: val.answer.to_string(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerComment {
    content: String,
    timestamp: DateTime<Utc>,
    commented_by: User,
}

impl From<domain::form::comment::models::Comment> for AnswerComment {
    fn from(val: domain::form::comment::models::Comment) -> Self {
        AnswerComment {
            content: val.content().to_string(),
            timestamp: val.timestamp().to_owned(),
            commented_by: val.commented_by().to_owned().into(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerLabels {
    id: Uuid,
    name: String,
}

impl From<domain::form::answer::models::AnswerLabel> for AnswerLabels {
    fn from(val: domain::form::answer::models::AnswerLabel) -> Self {
        AnswerLabels {
            id: val.id().to_owned().into(),
            name: val.name().to_string(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormAnswer {
    id: Uuid,
    user: User,
    form_id: Uuid,
    timestamp: DateTime<Utc>,
    title: Option<String>,
    answers: Vec<AnswerContent>,
    comments: Vec<AnswerComment>,
    labels: Vec<AnswerLabels>,
}

impl FormAnswer {
    pub fn new(
        answer: domain::form::answer::models::AnswerEntry,
        comments: Vec<domain::form::comment::models::Comment>,
        labels: Vec<domain::form::answer::models::AnswerLabel>,
    ) -> Self {
        FormAnswer {
            id: answer.id().to_owned().into(),
            user: answer.user().to_owned().into(),
            form_id: answer.form_id().into_inner(),
            timestamp: answer.timestamp().to_owned(),
            title: answer
                .title()
                .to_owned()
                .into_inner()
                .map(|title| title.to_string()),
            answers: answer
                .contents()
                .iter()
                .map(AnswerContent::from_ref)
                .collect_vec(),
            comments: comments.into_iter().map(Into::into).collect_vec(),
            labels: labels.into_iter().map(Into::into).collect_vec(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct MessageContentSchema {
    pub id: Uuid,
    pub body: String,
    pub sender: SenderSchema,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct SenderSchema {
    pub uuid: String,
    pub name: String,
    pub role: String,
}
