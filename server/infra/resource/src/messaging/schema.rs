use std::str::FromStr;

use domain::search::models::SearchableFields;
use errors::infra::InfraError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

// Debezium から送られてくる Payload の op フィールドには CRUD 操作が入っている。
// Read に該当する R は存在しない理由は、op フィールドに R が含まれるのは
// Debezium の Snapshot mode が有効のときのみ存在するからである。
// 詳細は https://debezium.io/documentation/reference/stable/connectors/mariadb.html#mariadb-events の
// Table 11. Descriptions of create event value fields を参照
#[derive(Deserialize, Copy, Clone, Debug)]
pub enum Operation {
    #[serde(rename = "c")]
    Create,
    #[serde(rename = "u")]
    Update,
    #[serde(rename = "d")]
    Delete,
}

#[derive(Deserialize, Debug)]
pub struct Source {
    pub table: String,
}

#[derive(Deserialize, Debug)]
pub struct Payload {
    pub op: Operation,
    pub source: Source,
    // before, after は source によってテーブル名が判別するまで型が不定
    pub before: Value,
    pub after: Value,
}

impl Payload {
    fn try_into_actual_data_fields(
        table_name: &str,
        value: Value,
    ) -> Result<Option<ActualDataFields>, InfraError> {
        match table_name {
            "form_meta_data" => {
                let form_meta_data: FormMetaData = serde_json::from_value(value)?;
                Ok(Some(ActualDataFields::FormMetaData(form_meta_data)))
            }
            "real_answers" => {
                let real_answers: RealAnswers = serde_json::from_value(value)?;
                Ok(Some(ActualDataFields::RealAnswers(real_answers)))
            }
            "form_answer_comments" => {
                let form_answer_comments: FormAnswerComments = serde_json::from_value(value)?;
                Ok(Some(ActualDataFields::FormAnswerComments(
                    form_answer_comments,
                )))
            }
            "label_for_form_answers" => {
                let label_for_form_answers: LabelForFormAnswers = serde_json::from_value(value)?;
                Ok(Some(ActualDataFields::LabelForFormAnswers(
                    label_for_form_answers,
                )))
            }
            "label_for_forms" => {
                let label_for_forms: LabelForForms = serde_json::from_value(value)?;
                Ok(Some(ActualDataFields::LabelForForms(label_for_forms)))
            }
            "users" => {
                let users: Users = serde_json::from_value(value)?;
                Ok(Some(ActualDataFields::Users(users)))
            }
            _ => Ok(None),
        }
    }

    pub fn try_into_after(self) -> Result<Option<ActualDataFields>, InfraError> {
        let table_name = self.source.table.as_str();
        let after = self.after;

        Self::try_into_actual_data_fields(table_name, after)
    }

    pub fn try_into_before(self) -> Result<Option<ActualDataFields>, InfraError> {
        let table_name = self.source.table.as_str();
        let before = self.before;

        Self::try_into_actual_data_fields(table_name, before)
    }
}

// RabbitMQ の message には、Debezium から送られてくる JSON が入っている
// Debezium の MariaDB スキーマは以下を参照
// ref: https://debezium.io/documentation/reference/stable/connectors/mariadb.html#mariadb-events
#[derive(Deserialize, Debug)]
pub struct RabbitMQSchema {
    pub payload: Payload,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FormMetaData {
    pub id: String,
    pub title: NonEmptyString,
    pub description: Option<NonEmptyString>,
}

impl From<domain::search::models::FormMetaData> for FormMetaData {
    fn from(form_meta_data: domain::search::models::FormMetaData) -> Self {
        Self {
            id: form_meta_data.id.to_string(),
            title: form_meta_data.title.into(),
            description: form_meta_data.description.into_inner(),
        }
    }
}

impl TryFrom<FormMetaData> for domain::search::models::FormMetaData {
    type Error = InfraError;

    fn try_from(form_meta_data: FormMetaData) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::from_str(&form_meta_data.id)?.into(),
            title: form_meta_data.title.into(),
            description: form_meta_data.description.into(),
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RealAnswers {
    pub id: String,
    pub answer_id: String,
    pub question_id: i32,
    pub answer: String,
}

impl From<domain::search::models::RealAnswers> for RealAnswers {
    fn from(real_answers: domain::search::models::RealAnswers) -> Self {
        Self {
            id: real_answers.id.to_string(),
            answer_id: real_answers.answer_id.to_string(),
            question_id: real_answers.question_id.into_inner(),
            answer: real_answers.answer,
        }
    }
}

impl TryFrom<RealAnswers> for domain::search::models::RealAnswers {
    type Error = InfraError;

    fn try_from(real_answers: RealAnswers) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::from_str(&real_answers.id)?.into(),
            answer_id: Uuid::from_str(&real_answers.answer_id)?.into(),
            question_id: real_answers.question_id.into(),
            answer: real_answers.answer,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FormAnswerComments {
    pub id: String,
    pub answer_id: String,
    pub content: String,
}

impl From<domain::search::models::FormAnswerComments> for FormAnswerComments {
    fn from(form_answer_comments: domain::search::models::FormAnswerComments) -> Self {
        Self {
            id: form_answer_comments.id.to_string(),
            answer_id: form_answer_comments.answer_id.to_string(),
            content: form_answer_comments.content,
        }
    }
}

impl TryFrom<FormAnswerComments> for domain::search::models::FormAnswerComments {
    type Error = InfraError;

    fn try_from(form_answer_comments: FormAnswerComments) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::from_str(&form_answer_comments.id)?.into(),
            answer_id: Uuid::from_str(&form_answer_comments.answer_id)?.into(),
            content: form_answer_comments.content,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LabelForFormAnswers {
    pub id: String,
    pub name: String,
}

impl From<domain::search::models::LabelForFormAnswers> for LabelForFormAnswers {
    fn from(label_for_form_answers: domain::search::models::LabelForFormAnswers) -> Self {
        Self {
            id: label_for_form_answers.id.to_string(),
            name: label_for_form_answers.name,
        }
    }
}

impl TryFrom<LabelForFormAnswers> for domain::search::models::LabelForFormAnswers {
    type Error = InfraError;

    fn try_from(label_for_form_answers: LabelForFormAnswers) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::from_str(&label_for_form_answers.id)?.into(),
            name: label_for_form_answers.name,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LabelForForms {
    pub id: String,
    pub name: String,
}

impl From<domain::search::models::LabelForForms> for LabelForForms {
    fn from(label_for_forms: domain::search::models::LabelForForms) -> Self {
        Self {
            id: label_for_forms.id.to_string(),
            name: label_for_forms.name,
        }
    }
}

impl TryFrom<LabelForForms> for domain::search::models::LabelForForms {
    type Error = InfraError;

    fn try_from(label_for_forms: LabelForForms) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::from_str(&label_for_forms.id)?.into(),
            name: label_for_forms.name,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Users {
    pub id: String,
    pub name: String,
}

impl From<domain::search::models::Users> for Users {
    fn from(users: domain::search::models::Users) -> Self {
        Self {
            id: users.id.to_string(),
            name: users.name,
        }
    }
}

impl TryFrom<Users> for domain::search::models::Users {
    type Error = InfraError;

    fn try_from(users: Users) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::from_str(&users.id)?,
            name: users.name,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ActualDataFields {
    FormMetaData(FormMetaData),
    RealAnswers(RealAnswers),
    FormAnswerComments(FormAnswerComments),
    LabelForFormAnswers(LabelForFormAnswers),
    LabelForForms(LabelForForms),
    Users(Users),
}

impl From<SearchableFields> for ActualDataFields {
    fn from(value: SearchableFields) -> Self {
        match value {
            SearchableFields::FormMetaData(data) => ActualDataFields::FormMetaData(data.into()),
            SearchableFields::RealAnswers(data) => ActualDataFields::RealAnswers(data.into()),
            SearchableFields::FormAnswerComments(data) => {
                ActualDataFields::FormAnswerComments(data.into())
            }
            SearchableFields::LabelForFormAnswers(data) => {
                ActualDataFields::LabelForFormAnswers(data.into())
            }
            SearchableFields::LabelForForms(data) => ActualDataFields::LabelForForms(data.into()),
            SearchableFields::Users(data) => ActualDataFields::Users(data.into()),
        }
    }
}

impl TryFrom<ActualDataFields> for SearchableFields {
    type Error = InfraError;

    fn try_from(value: ActualDataFields) -> Result<Self, Self::Error> {
        match value {
            ActualDataFields::FormMetaData(data) => {
                Ok(SearchableFields::FormMetaData(data.try_into()?))
            }
            ActualDataFields::RealAnswers(data) => {
                Ok(SearchableFields::RealAnswers(data.try_into()?))
            }
            ActualDataFields::FormAnswerComments(data) => {
                Ok(SearchableFields::FormAnswerComments(data.try_into()?))
            }
            ActualDataFields::LabelForFormAnswers(data) => {
                Ok(SearchableFields::LabelForFormAnswers(data.try_into()?))
            }
            ActualDataFields::LabelForForms(data) => {
                Ok(SearchableFields::LabelForForms(data.try_into()?))
            }
            ActualDataFields::Users(data) => Ok(SearchableFields::Users(data.try_into()?)),
        }
    }
}
