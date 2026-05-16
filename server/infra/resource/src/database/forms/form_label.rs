use async_trait::async_trait;
use domain::form::models::{FormId, FormLabel, FormLabelId, FormLabelName};
use errors::infra::InfraError;
use itertools::Itertools;
use sqlx::{Row, query};

use crate::{
    database::{components::FormLabelDatabase, connection::ConnectionPool, count::count_as_u32},
    dto::FormLabelDto,
};

#[async_trait]
impl FormLabelDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create_label_for_forms(&self, label: &FormLabel) -> Result<(), InfraError> {
        let label_id = label.id().to_owned().into_inner().to_string();
        let label_name = label.name().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO label_for_forms (id, name) VALUES (?, ?)",
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
    async fn fetch_labels(&self) -> Result<Vec<FormLabelDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs = sqlx::query("SELECT id, name FROM label_for_forms")
                    .fetch_all(&mut **txn)
                    .await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormLabelDto {
                            id: rs.try_get("id")?,
                            name: rs.try_get("name")?,
                        })
                    })
                    .collect::<Result<Vec<FormLabelDto>, _>>()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn fetch_labels_by_ids(
        &self,
        ids: Vec<FormLabelId>,
    ) -> Result<Vec<FormLabelDto>, InfraError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let label_ids = ids
            .into_iter()
            .map(|id| id.into_inner().to_string())
            .collect_vec();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let sql = format!(
                    "SELECT id, name FROM label_for_forms WHERE id IN ({})",
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
                        Ok::<_, InfraError>(FormLabelDto {
                            id: rs.try_get("id")?,
                            name: rs.try_get("name")?,
                        })
                    })
                    .collect::<Result<Vec<FormLabelDto>, _>>()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn delete_label_for_forms(&self, label_id: FormLabelId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM label_for_forms WHERE id = ?",
                    label_id.to_string(),
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn fetch_label(&self, id: FormLabelId) -> Result<Option<FormLabelDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let label_rs = sqlx::query("SELECT id, name FROM label_for_forms WHERE id = ?")
                    .bind(id.into_inner().to_string())
                    .fetch_optional(&mut **txn)
                    .await?;

                label_rs
                    .map(|rs| {
                        Ok::<_, InfraError>(FormLabelDto {
                            id: rs.try_get("id")?,
                            name: rs.try_get("name")?,
                        })
                    })
                    .transpose()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn edit_label_for_forms(
        &self,
        id: FormLabelId,
        name: FormLabelName,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "UPDATE label_for_forms SET name = ? WHERE id = ?",
                    name.to_string(),
                    id.to_string(),
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn fetch_labels_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<Vec<FormLabelDto>, InfraError> {
        let form_id = form_id.into_inner().to_string();
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs = sqlx::query(
                    "SELECT id, name FROM label_for_forms WHERE id IN (
                        SELECT label_id FROM label_settings_for_forms WHERE form_id = ?
                        UNION
                        SELECT label_id FROM archived_label_settings_for_forms WHERE form_id = ?
                    )",
                )
                .bind(form_id.clone())
                .bind(form_id)
                .fetch_all(&mut **txn)
                .await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormLabelDto {
                            id: rs.try_get("id")?,
                            name: rs.try_get("name")?,
                        })
                    })
                    .collect::<Result<Vec<FormLabelDto>, _>>()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<FormLabelId>,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM label_settings_for_forms WHERE form_id = ?",
                    form_id.clone(),
                )
                .execute(&mut **txn)
                .await?;

                if !label_ids.is_empty() {
                    let label_ids = label_ids
                        .into_iter()
                        .map(|label_id| label_id.into_inner().to_string())
                        .collect_vec();
                    let sql = format!(
                        "INSERT INTO label_settings_for_forms (form_id, label_id) VALUES {}",
                        std::iter::repeat_n("(?, ?)", label_ids.len()).join(", ")
                    );
                    label_ids
                        .into_iter()
                        .flat_map(|label_id| [form_id.clone(), label_id])
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
                let size =
                    sqlx::query_scalar!("SELECT COUNT(*) AS `count!: i64` FROM label_for_forms")
                        .fetch_one(&mut **txn)
                        .await?;

                count_as_u32(size, "label_for_forms")
            })
        })
        .await
    }
}
