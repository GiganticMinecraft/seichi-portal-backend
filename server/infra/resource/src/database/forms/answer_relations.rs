use async_trait::async_trait;
use domain::form::answer::{AnswerId, AnswerReference, AnswerRelation};
use errors::infra::InfraError;
use uuid::Uuid;

use crate::database::{
    components::{AnswerRelationRecord, FormAnswerRelationDatabase},
    connection::ConnectionPool,
};

fn reference_columns(reference: AnswerReference) -> (String, String) {
    (
        reference.form_id().to_string(),
        reference.answer_id().to_string(),
    )
}

fn relation_columns(relation: AnswerRelation) -> (String, String, String, String) {
    let [first, second] = relation.endpoints();
    let (first_form_id, first_answer_id) = reference_columns(first);
    let (second_form_id, second_answer_id) = reference_columns(second);
    (
        first_form_id,
        first_answer_id,
        second_form_id,
        second_answer_id,
    )
}

fn relation_from_row(
    first_form_id: String,
    first_answer_id: String,
    second_form_id: String,
    second_answer_id: String,
) -> Result<AnswerRelationRecord, InfraError> {
    let first = AnswerReference::new(
        Uuid::parse_str(&first_form_id)?.into(),
        Uuid::parse_str(&first_answer_id)?.into(),
    );
    let second = AnswerReference::new(
        Uuid::parse_str(&second_form_id)?.into(),
        Uuid::parse_str(&second_answer_id)?.into(),
    );
    let relation = AnswerRelation::new(first, second).map_err(|error| InfraError::Unexpected {
        cause: error.to_string(),
    })?;

    Ok(AnswerRelationRecord {
        first: relation.first(),
        second: relation.second(),
    })
}

#[async_trait]
impl FormAnswerRelationDatabase for ConnectionPool {
    #[tracing::instrument(skip(self))]
    async fn list_for_answer(
        &self,
        source: AnswerReference,
    ) -> Result<Vec<AnswerRelationRecord>, InfraError> {
        let (form_id, answer_id) = reference_columns(source);
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rows = sqlx::query!(
                    r"SELECT first_form_id, first_answer_id, second_form_id, second_answer_id
                    FROM answer_relations
                    WHERE (first_form_id = ? AND first_answer_id = ?)
                       OR (second_form_id = ? AND second_answer_id = ?)
                    ORDER BY first_form_id, first_answer_id, second_form_id, second_answer_id",
                    &form_id,
                    &answer_id,
                    &form_id,
                    &answer_id,
                )
                .fetch_all(&mut **txn)
                .await?;

                rows.into_iter()
                    .map(|row| {
                        relation_from_row(
                            row.first_form_id,
                            row.first_answer_id,
                            row.second_form_id,
                            row.second_answer_id,
                        )
                    })
                    .collect()
            })
        })
        .await
    }

    #[tracing::instrument(skip(self))]
    async fn add(&self, relation: AnswerRelation) -> Result<(), InfraError> {
        let (first_form_id, first_answer_id, second_form_id, second_answer_id) =
            relation_columns(relation);

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r"INSERT INTO answer_relations
                        (first_form_id, first_answer_id, second_form_id, second_answer_id)
                    VALUES (?, ?, ?, ?)
                    ON DUPLICATE KEY UPDATE first_form_id = first_form_id",
                    first_form_id,
                    first_answer_id,
                    second_form_id,
                    second_answer_id,
                )
                .execute(&mut **txn)
                .await?;
                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument(skip(self))]
    async fn remove(&self, relation: AnswerRelation) -> Result<(), InfraError> {
        let (first_form_id, first_answer_id, second_form_id, second_answer_id) =
            relation_columns(relation);

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r"DELETE FROM answer_relations
                    WHERE first_form_id = ? AND first_answer_id = ?
                      AND second_form_id = ? AND second_answer_id = ?",
                    first_form_id,
                    first_answer_id,
                    second_form_id,
                    second_answer_id,
                )
                .execute(&mut **txn)
                .await?;
                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument(skip(self))]
    async fn find_for_source_and_answer_id(
        &self,
        source: AnswerReference,
        answer_id: AnswerId,
    ) -> Result<Option<AnswerRelationRecord>, InfraError> {
        let (source_form_id, source_answer_id) = reference_columns(source);
        let answer_id = answer_id.to_string();
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let row = sqlx::query!(
                    r"SELECT first_form_id, first_answer_id, second_form_id, second_answer_id
                    FROM answer_relations
                    WHERE (first_form_id = ? AND first_answer_id = ? AND second_answer_id = ?)
                       OR (second_form_id = ? AND second_answer_id = ? AND first_answer_id = ?)
                    ORDER BY first_form_id, first_answer_id, second_form_id, second_answer_id
                    LIMIT 1",
                    &source_form_id,
                    &source_answer_id,
                    &answer_id,
                    &source_form_id,
                    &source_answer_id,
                    &answer_id,
                )
                .fetch_optional(&mut **txn)
                .await?;

                row.map(|row| {
                    relation_from_row(
                        row.first_form_id,
                        row.first_answer_id,
                        row.second_form_id,
                        row.second_answer_id,
                    )
                })
                .transpose()
            })
        })
        .await
    }
}
