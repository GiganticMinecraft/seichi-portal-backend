use async_trait::async_trait;
use domain::form::models::WebhookUrl;
use domain::{
    form::models::{Form, FormId},
    user::models::User,
};
use errors::infra::InfraError;
use types::non_empty_string::NonEmptyString;

use crate::{
    database::{
        components::FormDatabase,
        connection::{ConnectionPool, execute_and_values},
        count::count_as_u32,
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

                execute_and_values(
                    r"INSERT INTO default_answer_titles (form_id, title) VALUES (?, NULL)",
                    [form_id.to_owned().into_inner().to_string().into()],
                    txn,
                )
                .await?;

                execute_and_values(
                    r"INSERT INTO response_period (form_id, start_at, end_at) VALUES (?, NULL, NULL)",
                    [form_id.to_owned().into_inner().to_string().into()],
                    txn,
                )
                .await?;

                execute_and_values(
                    r"INSERT INTO form_webhooks (form_id, url) VALUES (?, NULL)",
                    [form_id.to_owned().into_inner().to_string().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<FormDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let form_rows = sqlx::query_as!(
                    FormDto,
                    r"SELECT form_meta_data.id AS id, form_meta_data.title AS title, description, visibility, answer_visibility, created_at AS `created_at!: chrono::DateTime<chrono::Utc>`, updated_at AS `updated_at!: chrono::DateTime<chrono::Utc>`, form_webhooks.url AS webhook_url, start_at AS `start_at: chrono::DateTime<chrono::Utc>`, end_at AS `end_at: chrono::DateTime<chrono::Utc>`, default_answer_titles.title AS default_answer_title
                    FROM form_meta_data
                    LEFT JOIN form_webhooks ON form_meta_data.id = form_webhooks.form_id
                    LEFT JOIN response_period ON form_meta_data.id = response_period.form_id
                    LEFT JOIN default_answer_titles ON form_meta_data.id = default_answer_titles.form_id
                    ORDER BY form_meta_data.id
                    LIMIT ? OFFSET ?",
                    limit.unwrap_or(u32::MAX),
                    offset.unwrap_or(0),
                )
                .fetch_all(&mut **txn)
                .await?;

                Ok::<_, InfraError>(form_rows)
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get(&self, form_id: FormId) -> Result<Option<FormDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let form = sqlx::query_as!(
                    FormDto,
                    r"SELECT form_meta_data.id AS id, form_meta_data.title AS title, description, visibility, answer_visibility, created_at AS `created_at!: chrono::DateTime<chrono::Utc>`, updated_at AS `updated_at!: chrono::DateTime<chrono::Utc>`, form_webhooks.url AS webhook_url, start_at AS `start_at: chrono::DateTime<chrono::Utc>`, end_at AS `end_at: chrono::DateTime<chrono::Utc>`, default_answer_titles.title AS default_answer_title
                    FROM form_meta_data
                    LEFT JOIN form_webhooks ON form_meta_data.id = form_webhooks.form_id
                    LEFT JOIN response_period ON form_meta_data.id = response_period.form_id
                    LEFT JOIN default_answer_titles ON form_meta_data.id = default_answer_titles.form_id
                    WHERE form_meta_data.id = ?",
                    form_id.into_inner().to_string(),
                )
                .fetch_optional(&mut **txn)
                .await?;

                Ok::<_, InfraError>(form)
            })
        })
        .await
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

        let webhook_url = form
            .settings()
            .webhook_url(updated_by)
            .ok()
            .map(ToOwned::to_owned)
            .and_then(WebhookUrl::into_inner)
            .map(NonEmptyString::into_inner);

        let update_form_webhooks_params = [
            form.id().into_inner().to_string().to_owned().into(),
            webhook_url.into(),
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
                    r#"INSERT INTO form_webhooks (form_id, url) VALUES (?, ?)
                    ON DUPLICATE KEY UPDATE
                    url = VALUES(url)
                    "#,
                    update_form_webhooks_params,
                    txn,
                )
                .await?;

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
                    sqlx::query_scalar!("SELECT COUNT(*) AS `count!: i64` FROM form_meta_data")
                        .fetch_one(&mut **txn)
                        .await?;

                count_as_u32(size, "form_meta_data")
            })
        })
        .await
    }
}
