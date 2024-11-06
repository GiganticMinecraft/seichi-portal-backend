use chrono::{DateTime, Utc};
use itertools::Itertools;
use serde::Serialize;
use uuid::Uuid;

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

impl From<domain::form::models::FormAnswerContent> for AnswerContent {
    fn from(val: domain::form::models::FormAnswerContent) -> Self {
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

impl From<domain::form::models::Comment> for AnswerComment {
    fn from(val: domain::form::models::Comment) -> Self {
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

impl From<domain::form::models::AnswerLabel> for AnswerLabels {
    fn from(val: domain::form::models::AnswerLabel) -> Self {
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
    form_id: i32,
    timestamp: DateTime<Utc>,
    title: Option<String>,
    answers: Vec<AnswerContent>,
    comments: Vec<AnswerComment>,
    labels: Vec<AnswerLabels>,
}

impl FormAnswer {
    pub fn new(
        answer: domain::form::models::FormAnswer,
        answer_contents: Vec<domain::form::models::FormAnswerContent>,
        comments: Vec<domain::form::models::Comment>,
        labels: Vec<domain::form::models::AnswerLabel>,
    ) -> Self {
        FormAnswer {
            id: answer.id.into(),
            user: answer.user.into(),
            form_id: answer.form_id.into(),
            timestamp: answer.timestamp,
            title: answer.title.default_answer_title,
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
