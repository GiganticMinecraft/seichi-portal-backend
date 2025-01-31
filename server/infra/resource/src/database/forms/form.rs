use std::str::FromStr;

use async_trait::async_trait;
use domain::{
    form::{
        answer::settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod},
        models::{Form, FormDescription, FormId, FormTitle, Visibility, WebhookUrl},
    },
    user::models::User,
};
use errors::infra::InfraError;
use futures::future::try_join;
use uuid::Uuid;

use crate::{
    database::{
        components::FormDatabase,
        connection::{execute_and_values, query_all_and_values, ConnectionPool},
    },
    dto::FormDto,
};

#[async_trait]
impl FormDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create(&self, form: &Form, user: &User) -> Result<(), InfraError> {
        let form_id = form.id().to_owned();
        let form_title = form.title().to_owned();
        let description = form
            .description()
            .to_owned()
            .into_inner()
            .map(|d| d.to_string());
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
                            id: Uuid::from_str(query_rs.try_get::<String>("", "form_id")?.as_str())?,
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
                    [form_id.into_inner().into()],
                    txn,
                )
                    .await?;

                let form = match form_query.first() {
                    Some(form) => form,
                    None => return Ok(None),
                };

                Ok::<_, InfraError>(Some(FormDto {
                    id: form_id.into_inner(),
                    title: form.try_get("", "form_title")?,
                    description: form.try_get("", "description")?,
                    metadata: (
                        form.try_get("", "created_at")?,
                        form.try_get("", "updated_at")?,
                    ),
                    start_at: form.try_get("", "start_at")?,
                    end_at: form.try_get("", "end_at")?,
                    webhook_url: form.try_get("", "url")?,
                    default_answer_title: form.try_get("", "default_answer_titles.title")?,
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
                    [form_id.into_inner().into()],
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
    async fn update_form_title(
        &self,
        form_id: &FormId,
        form_title: &FormTitle,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let form_title = form_title.to_owned().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r"UPDATE form_meta_data SET title = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                    [
                        form_title.to_string().into(),
                        form_id.into(),
                    ],
                    txn,
                ).await?;

                Ok::<_, InfraError>(())
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn update_form_description(
        &self,
        form_id: &FormId,
        form_description: &FormDescription,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let form_description = form_description.to_owned().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r"UPDATE form_meta_data SET description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                    [
                        form_description.map(|des| des.to_string()).into(),
                        form_id.into(),
                    ],
                    txn,
                ).await?;

                Ok::<_, InfraError>(())
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn update_form_response_period(
        &self,
        form_id: &FormId,
        response_period: &ResponsePeriod,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let start_at = response_period.start_at().to_owned();
        let end_at = response_period.end_at().to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE response_period SET start_at = ?, end_at = ? WHERE form_id = ?",
                    [start_at.into(), end_at.into(), form_id.into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn update_form_webhook_url(
        &self,
        form_id: &FormId,
        webhook_url: &WebhookUrl,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let webhook_url = webhook_url.to_owned().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE form_webhooks SET url = ? WHERE form_id = ?",
                    [webhook_url.map(|s| s.to_string()).into(), form_id.into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn update_form_default_answer_title(
        &self,
        form_id: &FormId,
        default_answer_title: &DefaultAnswerTitle,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let default_answer_title = default_answer_title.to_owned().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE default_answer_titles SET title = ? WHERE form_id = ?",
                    [
                        default_answer_title.map(|v| v.to_string()).into(),
                        form_id.into(),
                    ],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn update_form_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let visibility = visibility.to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE form_meta_data SET visibility = ? WHERE id = ?",
                    [visibility.into(), form_id.into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn update_form_answer_visibility(
        &self,
        form_id: &FormId,
        visibility: &AnswerVisibility,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let visibility = visibility.to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE form_meta_data SET answer_visibility = ? WHERE id = ?",
                    [visibility.into(), form_id.into()],
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
