use chrono::{DateTime, Utc};
use domain::form::models::FormId;
use itertools::Itertools;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, Debug)]
pub(crate) struct ResponsePeriodSchema {
    start_at: Option<DateTime<Utc>>,
    end_at: Option<DateTime<Utc>>,
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
pub(crate) struct FormListSchema {
    id: FormId,
    title: String,
    description: Option<String>,
    response_period: ResponsePeriodSchema,
    answer_visibility: AnswerVisibility,
}

impl From<domain::form::models::Form> for FormListSchema {
    fn from(form: domain::form::models::Form) -> Self {
        FormListSchema {
            id: form.id().to_owned(),
            title: form.title().to_string(),
            description: form
                .description()
                .to_owned()
                .into_inner()
                .map(|desc| desc.to_string()),
            response_period: ResponsePeriodSchema {
                start_at: form
                    .settings()
                    .answer_settings()
                    .response_period()
                    .start_at()
                    .to_owned(),
                end_at: form
                    .settings()
                    .answer_settings()
                    .response_period()
                    .end_at()
                    .to_owned(),
            },
            answer_visibility: form
                .settings()
                .answer_settings()
                .visibility()
                .to_owned()
                .into(),
        }
    }
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

impl From<domain::form::answer::models::FormAnswerContent> for AnswerContent {
    fn from(val: domain::form::answer::models::FormAnswerContent) -> Self {
        AnswerContent {
            question_id: val.question_id.into(),
            answer: val.answer,
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
    id: i32,
    name: String,
}

impl From<domain::form::answer::models::AnswerLabel> for AnswerLabels {
    fn from(val: domain::form::answer::models::AnswerLabel) -> Self {
        AnswerLabels {
            id: val.id.into(),
            name: val.name,
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
        answer: domain::form::answer::models::FormAnswer,
        answer_contents: Vec<domain::form::answer::models::FormAnswerContent>,
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
            answers: answer_contents.into_iter().map(Into::into).collect_vec(),
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
