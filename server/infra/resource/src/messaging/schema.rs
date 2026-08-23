use std::str::FromStr;

use domain::{
    form::answer::AnswerStatus,
    search::models::{
        Operation as SearchOperation, SearchableFields, SearchableFieldsWithOperation,
    },
};
use errors::infra::InfraError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

// Debezium から送られてくる CDC イベントに含まれる操作種別。
// `r` は Snapshot で取得した既存行を表すため、検索側では Create として扱う。
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
    #[serde(rename = "r")]
    Read,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
pub struct Source {
    #[serde(default)]
    pub table: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Payload {
    #[serde(default)]
    pub op: Option<Operation>,
    #[serde(default)]
    pub source: Option<Source>,
    // before, after は source によってテーブル名が判別するまで型が不定
    #[serde(default)]
    pub before: Value,
    #[serde(default)]
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
            "answers" => {
                let answer: AnswerTitleSearchDocument = serde_json::from_value(value)?;
                Ok(Some(ActualDataFields::AnswerTitle(answer)))
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
            "redmine_imported_comments" => {
                let redmine_imported_comment: FormAnswerComments = serde_json::from_value(value)?;
                Ok(Some(ActualDataFields::FormAnswerComments(
                    redmine_imported_comment,
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
        let Self { source, after, .. } = self;
        let Some(table_name) = source.as_ref().and_then(|source| source.table.as_deref()) else {
            return Ok(None);
        };

        Self::try_into_actual_data_fields(table_name, after)
    }

    pub fn try_into_before(self) -> Result<Option<ActualDataFields>, InfraError> {
        let Self { source, before, .. } = self;
        let Some(table_name) = source.as_ref().and_then(|source| source.table.as_deref()) else {
            return Ok(None);
        };

        Self::try_into_actual_data_fields(table_name, before)
    }

    pub fn try_into_searchable_fields(
        self,
    ) -> Result<Option<SearchableFieldsWithOperation>, InfraError> {
        let operation = match self.op {
            Some(Operation::Create) => SearchOperation::Create,
            Some(Operation::Update) => SearchOperation::Update,
            Some(Operation::Delete) => SearchOperation::Delete,
            Some(Operation::Read) => SearchOperation::Create,
            None | Some(Operation::Unknown) => return Ok(None),
        };

        let actual_data_fields = match self.op {
            Some(Operation::Delete) => self.try_into_before()?,
            Some(Operation::Create | Operation::Update | Operation::Read) => {
                self.try_into_after()?
            }
            None | Some(Operation::Unknown) => return Ok(None),
        };

        let data_fields = actual_data_fields
            .map(SearchableFields::try_from)
            .transpose()?;

        Ok(data_fields.map(|data_fields| (data_fields, operation)))
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
    pub description: String,
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
pub struct AnswerTitleSearchDocument {
    pub id: String,
    pub form_id: String,
    pub title: Option<NonEmptyString>,
    #[serde(default)]
    pub status: AnswerStatus,
}

impl From<domain::search::models::AnswerTitleSearchDocument> for AnswerTitleSearchDocument {
    fn from(answer: domain::search::models::AnswerTitleSearchDocument) -> Self {
        Self {
            id: answer.id.to_string(),
            form_id: answer.form_id.to_string(),
            title: answer.title.into_inner(),
            status: answer.status,
        }
    }
}

impl TryFrom<AnswerTitleSearchDocument> for domain::search::models::AnswerTitleSearchDocument {
    type Error = InfraError;

    fn try_from(answer: AnswerTitleSearchDocument) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::from_str(&answer.id)?.into(),
            form_id: Uuid::from_str(&answer.form_id)?.into(),
            title: domain::form::answer::AnswerTitle::new(answer.title),
            status: answer.status,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RealAnswers {
    pub id: String,
    pub answer_id: String,
    pub question_id: String,
    pub answer: String,
    #[serde(default)]
    pub status: AnswerStatus,
}

impl From<domain::search::models::RealAnswers> for RealAnswers {
    fn from(real_answers: domain::search::models::RealAnswers) -> Self {
        Self {
            id: real_answers.id.to_string(),
            answer_id: real_answers.answer_id.to_string(),
            question_id: real_answers.question_id.into_inner().to_string(),
            answer: real_answers.answer,
            status: real_answers.status,
        }
    }
}

impl TryFrom<RealAnswers> for domain::search::models::RealAnswers {
    type Error = InfraError;

    fn try_from(real_answers: RealAnswers) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::from_str(&real_answers.id)?.into(),
            answer_id: Uuid::from_str(&real_answers.answer_id)?.into(),
            question_id: Uuid::from_str(&real_answers.question_id)?.into(),
            answer: real_answers.answer,
            status: real_answers.status,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FormAnswerComments {
    #[serde(alias = "comment_id")]
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
    AnswerTitle(AnswerTitleSearchDocument),
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
            SearchableFields::AnswerTitle(data) => ActualDataFields::AnswerTitle(data.into()),
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
            ActualDataFields::AnswerTitle(data) => {
                Ok(SearchableFields::AnswerTitle(data.try_into()?))
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

#[cfg(test)]
mod tests {
    use super::{RabbitMQSchema, SearchableFields};
    use serde_json::json;
    use uuid::Uuid;

    fn answer_image(answer_id: Uuid, form_id: Uuid) -> serde_json::Value {
        json!({
            "id": answer_id.to_string(),
            "form_id": form_id.to_string(),
            "title": "検索できるタイトル"
        })
    }

    fn operation_from_answer_payload(
        operation: &str,
        before: serde_json::Value,
        after: serde_json::Value,
    ) -> domain::search::models::Operation {
        let schema: RabbitMQSchema = serde_json::from_value(json!({
            "payload": {
                "op": operation,
                "source": { "table": "answers" },
                "before": before,
                "after": after
            }
        }))
        .unwrap();

        schema
            .payload
            .try_into_searchable_fields()
            .unwrap()
            .unwrap()
            .1
    }

    #[test]
    fn non_row_payloads_are_ignored_by_search_conversion() {
        let payloads = [
            json!({
                "schema": { "type": "struct" },
                "payload": {
                    "source": {
                        "table": null,
                        "db": "seichi-portal",
                        "connector": "mariadb"
                    },
                    "databaseName": "seichi_portal",
                    "schemaName": null,
                    "ddl": "CREATE TABLE answers (...)"
                }
            }),
            json!({
                "payload": {
                    "source": {},
                    "tableChanges": []
                }
            }),
            json!({ "payload": { "source": null } }),
            json!({ "payload": {} }),
            json!({
                "payload": {
                    "op": null,
                    "source": { "table": "answers" }
                }
            }),
            json!({
                "payload": {
                    "op": "unknown",
                    "source": { "table": "answers" }
                }
            }),
        ];

        for payload in payloads {
            let schema: RabbitMQSchema = serde_json::from_value(payload).unwrap();
            assert!(
                schema
                    .payload
                    .try_into_searchable_fields()
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn snapshot_read_payload_uses_after_image_as_create() {
        let answer_id = Uuid::from_u128(1);
        let form_id = Uuid::from_u128(2);
        let schema: RabbitMQSchema = serde_json::from_value(json!({
            "payload": {
                "op": "r",
                "source": { "table": "answers" },
                "before": null,
                "after": answer_image(answer_id, form_id)
            }
        }))
        .unwrap();

        let Some((SearchableFields::AnswerTitle(document), operation)) =
            schema.payload.try_into_searchable_fields().unwrap()
        else {
            panic!("snapshot payload must become a search event");
        };

        assert!(matches!(
            operation,
            domain::search::models::Operation::Create
        ));
        assert_eq!(document.id.into_inner(), answer_id);
        assert_eq!(document.form_id.into_inner(), form_id);
    }

    #[test]
    fn c_u_d_payloads_keep_their_domain_operations() {
        let answer_id = Uuid::from_u128(1);
        let form_id = Uuid::from_u128(2);

        assert!(matches!(
            operation_from_answer_payload("c", json!(null), answer_image(answer_id, form_id)),
            domain::search::models::Operation::Create
        ));
        assert!(matches!(
            operation_from_answer_payload("u", json!(null), answer_image(answer_id, form_id)),
            domain::search::models::Operation::Update
        ));
        assert!(matches!(
            operation_from_answer_payload("d", answer_image(answer_id, form_id), json!(null)),
            domain::search::models::Operation::Delete
        ));
    }

    #[test]
    fn known_table_with_invalid_after_image_returns_conversion_error() {
        let schema: RabbitMQSchema = serde_json::from_value(json!({
            "payload": {
                "op": "u",
                "source": { "table": "answers" },
                "before": null,
                "after": { "id": "not-a-complete-answer-row" }
            }
        }))
        .unwrap();

        assert!(schema.payload.try_into_searchable_fields().is_err());
    }

    #[test]
    fn answers_create_or_update_payload_uses_after_image() {
        let answer_id = Uuid::from_u128(1);
        let form_id = Uuid::from_u128(2);
        let schema: RabbitMQSchema = serde_json::from_value(json!({
            "payload": {
                "op": "u",
                "source": { "table": "answers" },
                "before": null,
                "after": {
                    "id": answer_id.to_string(),
                    "form_id": form_id.to_string(),
                    "title": "検索できるタイトル"
                }
            }
        }))
        .unwrap();

        let actual = schema.payload.try_into_after().unwrap().unwrap();
        let SearchableFields::AnswerTitle(document) = SearchableFields::try_from(actual).unwrap()
        else {
            panic!("answers payload must become an answer title search document");
        };

        assert_eq!(document.id.into_inner(), answer_id);
        assert_eq!(document.form_id.into_inner(), form_id);
        assert_eq!(
            document.status,
            domain::form::answer::AnswerStatus::UNADDRESSED
        );
        assert_eq!(
            serde_json::to_value(document.title).unwrap(),
            json!("検索できるタイトル")
        );
    }

    #[test]
    fn answers_payload_preserves_status_and_old_payload_defaults_to_unaddressed() {
        let answer_id = Uuid::from_u128(1);
        let form_id = Uuid::from_u128(2);
        let schema: RabbitMQSchema = serde_json::from_value(json!({
            "payload": {
                "op": "u",
                "source": { "table": "answers" },
                "before": null,
                "after": {
                    "id": answer_id.to_string(),
                    "form_id": form_id.to_string(),
                    "title": "検索できるタイトル",
                    "status": "COMPLETED"
                }
            }
        }))
        .unwrap();
        let actual = schema.payload.try_into_after().unwrap().unwrap();
        let SearchableFields::AnswerTitle(document) = SearchableFields::try_from(actual).unwrap()
        else {
            panic!("answers payload must become a title document");
        };
        assert_eq!(
            document.status,
            domain::form::answer::AnswerStatus::COMPLETED
        );

        let old_schema: RabbitMQSchema = serde_json::from_value(json!({
            "payload": {
                "op": "u",
                "source": { "table": "answers" },
                "before": null,
                "after": {
                    "id": answer_id.to_string(),
                    "form_id": form_id.to_string(),
                    "title": "旧ペイロード"
                }
            }
        }))
        .unwrap();
        let actual = old_schema.payload.try_into_after().unwrap().unwrap();
        let SearchableFields::AnswerTitle(document) = SearchableFields::try_from(actual).unwrap()
        else {
            panic!("answers payload must become a title document");
        };
        assert_eq!(
            document.status,
            domain::form::answer::AnswerStatus::UNADDRESSED
        );
    }

    #[test]
    fn answers_delete_payload_uses_before_image_and_preserves_null_title() {
        let answer_id = Uuid::from_u128(1);
        let form_id = Uuid::from_u128(2);
        let schema: RabbitMQSchema = serde_json::from_value(json!({
            "payload": {
                "op": "d",
                "source": { "table": "answers" },
                "before": {
                    "id": answer_id.to_string(),
                    "form_id": form_id.to_string(),
                    "title": null
                },
                "after": null
            }
        }))
        .unwrap();

        let actual = schema.payload.try_into_before().unwrap().unwrap();
        let SearchableFields::AnswerTitle(document) = SearchableFields::try_from(actual).unwrap()
        else {
            panic!("answers payload must become an answer title search document");
        };

        assert_eq!(document.id.into_inner(), answer_id);
        assert_eq!(serde_json::to_value(document.title).unwrap(), json!(null));
    }

    #[test]
    fn imported_comment_payload_uses_comment_id_as_the_search_document_id() {
        let comment_id = Uuid::from_u128(3);
        let answer_id = Uuid::from_u128(4);
        let schema: RabbitMQSchema = serde_json::from_value(json!({
            "payload": {
                "op": "c",
                "source": { "table": "redmine_imported_comments" },
                "before": null,
                "after": {
                    "comment_id": comment_id.to_string(),
                    "answer_id": answer_id.to_string(),
                    "redmine_journal_id": 99,
                    "redmine_author_name": "Redmine user",
                    "content": "検索できるコメント"
                }
            }
        }))
        .unwrap();

        let actual = schema.payload.try_into_after().unwrap().unwrap();
        let SearchableFields::FormAnswerComments(document) =
            SearchableFields::try_from(actual).unwrap()
        else {
            panic!("imported comments must become a comment search document");
        };

        assert_eq!(document.id.into_inner(), comment_id);
        assert_eq!(document.answer_id.into_inner(), answer_id);
        assert_eq!(document.content, "検索できるコメント");
    }
}
