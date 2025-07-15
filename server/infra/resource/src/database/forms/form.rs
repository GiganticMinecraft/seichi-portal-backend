use async_trait::async_trait;
use domain::{
    form::models::{Form, FormId},
    user::models::User,
};
use errors::infra::InfraError;
use futures::future::try_join;
use types::non_empty_string::NonEmptyString;

use crate::database::connection::query_one;
use crate::{
    database::{
        components::FormDatabase,
        connection::{ConnectionPool, execute_and_values, query_all_and_values},
    },
    dto::FormDto,
};

#[async_trait]
impl FormDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create(&self, form: &Form, user: &User) -> Result<(), InfraError> {
        let form_id = form.id().to_owned();
        let form_title = form.title().to_owned();
        let description = form.description().to_owned().into_inner();
        let user_id = user.id.to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r#"INSERT INTO form_meta_data (id, title, description, created_by, updated_by)
                            VALUES (?, ?, ?, ?, ?)"#,
                    [
                        form_id.into_inner().to_string().into(),
                        form_title.to_string().into(),
                        description.to_owned().into(),
                        user_id.to_string().into(),
                        user_id.to_string().into(),
                    ],
                    txn,
                )
                .await?;

                let insert_default_answer_title_table = execute_and_values(
                    r"INSERT INTO default_answer_titles (form_id, title) VALUES (?, NULL)",
                    [form_id.to_owned().into_inner().to_string().into()],
                    txn,
                );

                let insert_response_period_table = execute_and_values(
                    r"INSERT INTO response_period (form_id, start_at, end_at) VALUES (?, NULL, NULL)",
                    [form_id.to_owned().into_inner().to_string().into()],
                    txn,
                );

                try_join(
                    insert_default_answer_title_table,
                    insert_response_period_table,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<FormDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let form_query = query_all_and_values(
                    r"SELECT form_meta_data.id AS form_id, form_meta_data.title AS form_title, description, visibility, answer_visibility, created_at, updated_at, url, start_at, end_at, default_answer_titles.title
                            FROM form_meta_data
                            LEFT JOIN form_webhooks ON form_meta_data.id = form_webhooks.form_id
                            LEFT JOIN response_period ON form_meta_data.id = response_period.form_id
                            LEFT JOIN default_answer_titles ON form_meta_data.id = default_answer_titles.form_id
                            ORDER BY form_meta_data.id
                            LIMIT ? OFFSET ?",
                    [limit.unwrap_or(u32::MAX).into(), offset.unwrap_or(0).into()],
                    txn,
                )
                    .await?;

                form_query
                    .into_iter()
                    .map(|query_rs| {
                        Ok::<_, InfraError>(FormDto {
                            id: query_rs.try_get("", "form_id")?,
                            title: query_rs.try_get("", "form_title")?,
                            description: query_rs.try_get("", "description")?,
                            metadata: (
                                query_rs.try_get("", "created_at")?,
                                query_rs.try_get("", "updated_at")?,
                            ),
                            start_at: query_rs.try_get("", "start_at")?,
                            end_at: query_rs.try_get("", "end_at")?,
                            webhook_url: query_rs.try_get("", "url")?,
                            default_answer_title: query_rs.try_get("", "default_answer_titles.title")?,
                            visibility: query_rs.try_get("", "visibility")?,
                            answer_visibility: query_rs.try_get("", "answer_visibility")?,
                        })
                    }).collect::<Result<Vec<_>, _>>()
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get(&self, form_id: FormId) -> Result<Option<FormDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let form_query = query_all_and_values(
                    r"SELECT form_meta_data.id AS form_id, form_meta_data.title AS form_title, description, visibility, answer_visibility, created_at, updated_at, url, start_at, end_at, default_answer_titles.title
                            FROM form_meta_data
                            LEFT JOIN form_webhooks ON form_meta_data.id = form_webhooks.form_id
                            LEFT JOIN response_period ON form_meta_data.id = response_period.form_id
                            LEFT JOIN default_answer_titles ON form_meta_data.id = default_answer_titles.form_id
                            WHERE form_meta_data.id = ?",
                    [form_id.into_inner().to_string().into()],
                    txn,
                )
                    .await?;

                let form = match form_query.first() {
                    Some(form) => form,
                    None => return Ok(None),
                };

                Ok::<_, InfraError>(Some(FormDto {
                    id: form_id.into_inner().to_string(),
                    title: form.try_get("", "form_title")?,
                    description: form.try_get("", "description")?,
                    metadata: (
                        form.try_get("", "created_at")?,
                        form.try_get("", "updated_at")?,
                    ),
                    start_at: form.try_get("", "start_at")?,
                    end_at: form.try_get("", "end_at")?,
                    webhook_url: form.try_get("", "url")?,
                    default_answer_title: form.try_get("", "title")?,
                    visibility: form.try_get("", "visibility")?,
                    answer_visibility: form.try_get("", "answer_visibility")?,
                }))
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn delete(&self, form_id: FormId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM form_meta_data WHERE id = ?",
                    [form_id.into_inner().to_string().into()],
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
    async fn update(&self, form: &Form, updated_by: &User) -> Result<(), InfraError> {
        let form_meta_update_params = [
            form.title().to_owned().into_inner().into_inner().into(),
            form.description().to_owned().into_inner().into(),
            form.settings().visibility().to_string().into(),
            form.settings()
                .answer_settings()
                .visibility()
                .to_string()
                .into(),
            updated_by.id.to_string().into(),
            form.id().into_inner().to_owned().to_string().into(),
        ];

        let update_form_webhooks_params = [
            form.settings()
                .webhook_url()
                .to_owned()
                .into_inner()
                .map(NonEmptyString::into_inner)
                .into(),
            form.id().into_inner().to_string().to_owned().into(),
        ];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r#"UPDATE form_meta_data SET
                    title = ?,
                    description = ?,
                    visibility = ?,
                    answer_visibility = ?,
                    updated_by = ?
                    WHERE id = ?
                    "#,
                    form_meta_update_params,
                    txn,
                )
                .await?;

                execute_and_values(
                    r#"UPDATE form_webhooks SET
                        url = ?
                        WHERE form_id = ?"#,
                    update_form_webhooks_params,
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
                let query = query_one("SELECT COUNT(*) AS count FROM form_meta_data", txn).await?;

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
