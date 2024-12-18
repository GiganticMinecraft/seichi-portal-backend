use async_trait::async_trait;
use domain::form::models::{FormId, Label, LabelId};
use errors::infra::InfraError;
use itertools::Itertools;

use crate::{
    database::{
        components::FormLabelDatabase,
        connection::{
            batch_insert, execute_and_values, multiple_delete, query_all, ConnectionPool,
        },
    },
    dto::LabelDto,
};

#[async_trait]
impl FormLabelDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create_label_for_forms(&self, label_name: String) -> Result<(), InfraError> {
        let params = [label_name.to_owned().into()];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values("INSERT INTO label_for_forms (name) VALUES (?)", params, txn)
                    .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_labels_for_forms(&self) -> Result<Vec<LabelDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs = query_all("SELECT id, name FROM label_for_forms", txn).await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(LabelDto {
                            id: rs.try_get("", "id")?,
                            name: rs.try_get("", "name")?,
                        })
                    })
                    .collect::<Result<Vec<LabelDto>, _>>()
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn delete_label_for_forms(&self, label_id: LabelId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM label_for_forms WHERE id = ?",
                    [label_id.to_string().into()],
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
    async fn edit_label_for_forms(&self, label: &Label) -> Result<(), InfraError> {
        let params = [label.name.to_owned().into(), label.id.to_string().into()];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE label_for_forms SET name = ? WHERE id = ?",
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
    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                multiple_delete(
                    "DELETE FROM label_settings_for_forms WHERE form_id = ?",
                    vec![form_id.into_inner().into()],
                    txn,
                )
                .await?;

                let params = label_ids
                    .into_iter()
                    .flat_map(|label_id| {
                        [form_id.into_inner().into(), label_id.into_inner().into()]
                    })
                    .collect_vec();

                batch_insert(
                    "INSERT INTO label_settings_for_forms (form_id, label_id) VALUES (?, ?)",
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
}
