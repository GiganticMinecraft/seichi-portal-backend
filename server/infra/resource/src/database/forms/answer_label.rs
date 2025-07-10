use async_trait::async_trait;
use domain::form::answer::models::{AnswerId, AnswerLabel, AnswerLabelId};
use errors::infra::InfraError;
use itertools::Itertools;

use crate::database::connection::query_one;
use crate::{
    database::{
        components::FormAnswerLabelDatabase,
        connection::{
            ConnectionPool, batch_insert, execute_and_values, multiple_delete, query_all,
            query_all_and_values,
        },
    },
    dto::AnswerLabelDto,
};

#[async_trait]
impl FormAnswerLabelDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create_label_for_answers(&self, label_name: String) -> Result<(), InfraError> {
        let params = [label_name.to_owned().into()];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "INSERT INTO label_for_form_answers (name) VALUES (?)",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_labels_for_answers(&self) -> Result<Vec<AnswerLabelDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs =
                    query_all("SELECT id, name FROM label_for_form_answers", txn).await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(AnswerLabelDto {
                            id: rs.try_get("", "id")?,
                            name: rs.try_get("", "name")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_label_for_answers(
        &self,
        label_id: AnswerLabelId,
    ) -> Result<Option<AnswerLabelDto>, InfraError> {
        let label_id = label_id.into_inner();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let label_rs = query_all_and_values(
                    "SELECT id, name FROM label_for_form_answers WHERE id = ?",
                    [label_id.to_string().into()],
                    txn,
                )
                .await?;

                label_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(AnswerLabelDto {
                            id: rs.try_get("", "id")?,
                            name: rs.try_get("", "name")?,
                        })
                    })
                    .next()
                    .transpose()
            })
        })
        .await
        .map_err(Into::into)
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
            .collect_vec()
            .join(", ");

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs = query_all(
                    format!("SELECT id, name FROM label_for_form_answers WHERE id IN {label_ids}")
                        .as_str(),
                    txn,
                )
                .await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(AnswerLabelDto {
                            id: rs.try_get("", "id")?,
                            name: rs.try_get("", "name")?,
                        })
                    })
                    .collect::<Result<Vec<AnswerLabelDto>, _>>()
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerLabelDto>, InfraError> {
        let answer_id = answer_id.into_inner();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs = query_all_and_values(
                    r"SELECT label_for_form_answers.id AS label_id, name FROM label_for_form_answers
                    INNER JOIN label_settings_for_form_answers ON label_for_form_answers.id = label_settings_for_form_answers.label_id
                    WHERE answer_id = ?",
                    [answer_id.into()],
                    txn,
                )
                    .await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(AnswerLabelDto {
                            id: rs.try_get("", "label_id")?,
                            name: rs.try_get("", "name")?,
                        })
                    })
                    .collect::<Result<Vec<AnswerLabelDto>, _>>()
            })
        })
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn delete_label_for_answers(&self, label_id: AnswerLabelId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM label_for_form_answers WHERE id = ?",
                    [label_id.into_inner().to_string().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn edit_label_for_answers(&self, label: &AnswerLabel) -> Result<(), InfraError> {
        let params = [
            label.name().to_owned().into(),
            label.id().to_string().into(),
        ];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE label_for_form_answers SET name = ? WHERE id = ?",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                multiple_delete(
                    "DELETE FROM label_settings_for_form_answers WHERE answer_id = ?",
                    vec![answer_id.into_inner().to_string().into()],
                    txn,
                )
                .await?;

                let params = label_ids
                    .into_iter()
                    .flat_map(|label_id| {
                        [
                            answer_id.into_inner().to_string().into(),
                            label_id.into_inner().to_string().into(),
                        ]
                    })
                    .collect_vec();

                batch_insert(
                    "INSERT INTO label_settings_for_form_answers (answer_id, label_id) VALUES (?, \
                     ?)",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn size(&self) -> Result<u32, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let query =
                    query_one("SELECT COUNT(*) AS count FROM label_for_form_answers", txn).await?;

                let size = query
                    .map(|rs| rs.try_get::<i32>("", "count"))
                    .transpose()?
                    .unwrap_or(0);

                Ok::<_, InfraError>(size as u32)
            })
        })
        .await
        .map_err(Into::into)
    }
}
