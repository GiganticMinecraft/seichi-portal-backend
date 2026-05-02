use async_trait::async_trait;
use domain::form::answer::models::{AnswerId, AnswerLabel, AnswerLabelId};
use errors::infra::InfraError;
use itertools::Itertools;
use sqlx::{Row, query};

use crate::{
    database::{
        components::FormAnswerLabelDatabase, connection::ConnectionPool, count::count_as_u32,
    },
    dto::AnswerLabelDto,
};

#[async_trait]
impl FormAnswerLabelDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create_label_for_answers(&self, label: &AnswerLabel) -> Result<(), InfraError> {
        let label_id = label.id().into_inner().to_string();
        let label_name = label.name().to_owned().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO label_for_form_answers (id, name) VALUES (?, ?)",
                    label_id,
                    label_name,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_labels_for_answers(&self) -> Result<Vec<AnswerLabelDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs = sqlx::query("SELECT id, name FROM label_for_form_answers")
                    .fetch_all(&mut **txn)
                    .await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(AnswerLabelDto {
                            id: rs.try_get("id")?,
                            name: rs.try_get("name")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_label_for_answers(
        &self,
        label_id: AnswerLabelId,
    ) -> Result<Option<AnswerLabelDto>, InfraError> {
        let label_id = label_id.into_inner();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let label_rs =
                    sqlx::query("SELECT id, name FROM label_for_form_answers WHERE id = ?")
                        .bind(label_id.to_string())
                        .fetch_all(&mut **txn)
                        .await?;

                label_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(AnswerLabelDto {
                            id: rs.try_get("id")?,
                            name: rs.try_get("name")?,
                        })
                    })
                    .next()
                    .transpose()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_labels_for_answers_by_label_ids(
        &self,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<Vec<AnswerLabelDto>, InfraError> {
        if label_ids.is_empty() {
            return Ok(Vec::new());
        }

        let label_ids = label_ids
            .into_iter()
            .map(|id| id.into_inner().to_string())
            .collect_vec();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let sql = format!(
                    "SELECT id, name FROM label_for_form_answers WHERE id IN ({})",
                    std::iter::repeat_n("?", label_ids.len()).join(", ")
                );
                let labels_rs = label_ids
                    .iter()
                    .fold(query(&sql), |query, label_id| query.bind(label_id))
                    .fetch_all(&mut **txn)
                    .await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(AnswerLabelDto {
                            id: rs.try_get("id")?,
                            name: rs.try_get("name")?,
                        })
                    })
                    .collect::<Result<Vec<AnswerLabelDto>, _>>()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerLabelDto>, InfraError> {
        let answer_id = answer_id.into_inner();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs = sqlx::query(
                    r"SELECT label_for_form_answers.id AS label_id, name FROM label_for_form_answers
                    INNER JOIN label_settings_for_form_answers ON label_for_form_answers.id = label_settings_for_form_answers.label_id
                    WHERE answer_id = ?",
                )
                .bind(answer_id)
                .fetch_all(&mut **txn)
                .await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(AnswerLabelDto {
                            id: rs.try_get("label_id")?,
                            name: rs.try_get("name")?,
                        })
                    })
                    .collect::<Result<Vec<AnswerLabelDto>, _>>()
            })
        })
            .await
    }

    #[tracing::instrument]
    async fn delete_label_for_answers(&self, label_id: AnswerLabelId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM label_for_form_answers WHERE id = ?",
                    label_id.into_inner().to_string(),
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn edit_label_for_answers(&self, label: &AnswerLabel) -> Result<(), InfraError> {
        let label_name = label.name().to_owned().into_inner();
        let label_id = label.id().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "UPDATE label_for_form_answers SET name = ? WHERE id = ?",
                    label_name,
                    label_id,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<(), InfraError> {
        let answer_id = answer_id.into_inner().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM label_settings_for_form_answers WHERE answer_id = ?",
                    answer_id.clone(),
                )
                .execute(&mut **txn)
                .await?;

                if !label_ids.is_empty() {
                    let label_ids = label_ids
                        .into_iter()
                        .map(|label_id| label_id.into_inner().to_string())
                        .collect_vec();
                    let sql = format!(
                        "INSERT INTO label_settings_for_form_answers (answer_id, label_id) VALUES {}",
                        std::iter::repeat_n("(?, ?)", label_ids.len()).join(", ")
                    );
                    label_ids
                        .into_iter()
                        .flat_map(|label_id| [answer_id.clone(), label_id])
                        .fold(query(&sql), |query, value| query.bind(value))
                        .execute(&mut **txn)
                        .await?;
                }

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn size(&self) -> Result<u32, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let size = sqlx::query_scalar!(
                    "SELECT COUNT(*) AS `count!: i64` FROM label_for_form_answers"
                )
                .fetch_one(&mut **txn)
                .await?;

                count_as_u32(size, "label_for_form_answers")
            })
        })
        .await
    }
}
