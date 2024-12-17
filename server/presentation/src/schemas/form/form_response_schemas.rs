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

impl From<domain::form::models::Visibility> for AnswerVisibility {
    fn from(val: domain::form::models::Visibility) -> Self {
        match val {
            domain::form::models::Visibility::PUBLIC => AnswerVisibility::Public,
            domain::form::models::Visibility::PRIVATE => AnswerVisibility::Private,
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
            title: form.title().to_owned().into_inner(),
            description: form.description().to_owned().into_inner(),
            response_period: ResponsePeriodSchema {
                start_at: form.settings().response_period().start_at().to_owned(),
                end_at: form.settings().response_period().end_at().to_owned(),
            },
            answer_visibility: form.settings().answer_visibility().to_owned().into(),
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
            content: val.content,
            timestamp: val.timestamp,
            commented_by: val.commented_by.into(),
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
    id: i32,
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
            id: answer.id.into(),
            user: answer.user.into(),
            form_id: answer.form_id.into(),
            timestamp: answer.timestamp,
            title: answer.title,
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
