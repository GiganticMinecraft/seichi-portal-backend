use chrono::{DateTime, Utc};
use domain::form::{
    answer::settings::models::DefaultAnswerTitle,
    models::{
        FormDescription, FormId, FormLabel, FormMeta, FormSettings, FormTitle, Visibility,
        WebhookUrl,
    },
    question::models::Question,
};
use itertools::Itertools;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, Debug)]
pub(crate) struct ResponsePeriodSchema {
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug)]
pub(crate) enum AnswerVisibility {
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

#[derive(Serialize, Debug)]
pub(crate) struct FormSettingsSchema {
    pub response_period: ResponsePeriodSchema,
    pub webhook_url: WebhookUrl,
    pub default_answer_title: DefaultAnswerTitle,
    pub visibility: Visibility,
    pub answer_visibility: AnswerVisibility,
}

impl FormSettingsSchema {
    pub fn from_settings_ref(settings: &FormSettings) -> Self {
        FormSettingsSchema {
            response_period: ResponsePeriodSchema {
                start_at: settings
                    .answer_settings()
                    .response_period()
                    .start_at()
                    .to_owned(),
                end_at: settings
                    .answer_settings()
                    .response_period()
                    .end_at()
                    .to_owned(),
            },
            webhook_url: settings.webhook_url().to_owned(),
            default_answer_title: settings.answer_settings().default_answer_title().to_owned(),
            visibility: settings.visibility().to_owned(),
            answer_visibility: settings.answer_settings().visibility().to_owned().into(),
        }
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct FormMetaSchema {
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

#[derive(Serialize, Debug)]
pub(crate) struct FormSchema {
    pub id: FormId,
    pub title: FormTitle,
    pub description: FormDescription,
    pub settings: FormSettingsSchema,
    pub metadata: FormMetaSchema,
    pub questions: Vec<Question>,
    pub labels: Vec<FormLabel>,
}

#[derive(Serialize, Debug)]
pub(crate) struct FormListSchema {
    pub id: FormId,
    pub title: String,
    pub description: String,
    pub response_period: ResponsePeriodSchema,
    pub answer_visibility: AnswerVisibility,
    pub labels: Vec<FormLabel>,
}

#[derive(Serialize, Debug)]
pub(crate) enum Role {
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

#[derive(Serialize, Debug)]
pub(crate) struct User {
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

#[derive(Serialize, Debug)]
pub(crate) struct AnswerContent {
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

#[derive(Serialize, Debug)]
pub(crate) struct AnswerComment {
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

#[derive(Serialize, Debug)]
pub(crate) struct AnswerLabels {
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

#[derive(Serialize, Debug)]
pub(crate) struct FormAnswer {
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

#[derive(Serialize, Debug)]
pub struct MessageContentSchema {
    pub id: Uuid,
    pub body: String,
    pub sender: SenderSchema,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct SenderSchema {
    pub uuid: String,
    pub name: String,
    pub role: String,
}
