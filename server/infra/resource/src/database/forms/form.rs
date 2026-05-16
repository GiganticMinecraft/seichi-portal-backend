use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::form::{
    models::WebhookUrl,
    question::models::{Question, QuestionId, QuestionType},
};
use domain::{
    form::models::{ArchivedForm, Form, FormId},
    user::models::{Role, User},
};
use errors::infra::InfraError;
use itertools::Itertools;
use sqlx::{MySqlConnection, Row, query};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

use crate::{
    database::{
        components::FormDatabase,
        connection::{ConnectionPool, DatabaseTransaction},
        count::count_as_u32,
    },
    dto::{ArchivedFormDto, ChoiceDto, FormDto, QuestionDto},
};

struct FormRowDto {
    id: String,
    title: String,
    description: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    start_at: Option<DateTime<Utc>>,
    end_at: Option<DateTime<Utc>>,
    webhook_url: Option<String>,
    default_answer_title: Option<String>,
    visibility: String,
    answer_visibility: String,
}

struct ArchivedFormRowDto {
    form: FormRowDto,
    archived_at: DateTime<Utc>,
    archived_by_name: String,
    archived_by_id: String,
    archived_by_role: Role,
}

async fn get_questions_txn_with_tables(
    txn: &mut DatabaseTransaction,
    form_id: FormId,
    questions_table: &str,
    choices_table: &str,
) -> Result<Vec<QuestionDto>, InfraError> {
    let form_id = form_id.into_inner().to_string();
    let questions_sql = format!(
        "SELECT question_id, form_id, template_key, position, title, description, question_type, is_required
        FROM {questions_table}
        WHERE form_id = ?
        ORDER BY position ASC, question_id ASC"
    );
    let choices_sql = format!(
        "SELECT c.id, c.question_id, c.position, c.label
        FROM {choices_table} c
        INNER JOIN {questions_table} q ON c.question_id = q.question_id
        WHERE q.form_id = ?
        ORDER BY c.position ASC, c.id ASC"
    );

    let questions_rs = sqlx::query(&questions_sql)
        .bind(form_id.clone())
        .fetch_all(&mut **txn)
        .await?;

    let choices_by_question_id = sqlx::query(&choices_sql)
        .bind(form_id)
        .fetch_all(&mut **txn)
        .await?
        .into_iter()
        .map(|choice_rs| {
            let question_id = Uuid::parse_str(&choice_rs.try_get::<String, _>("question_id")?)?;
            Ok::<_, InfraError>((
                question_id,
                ChoiceDto {
                    id: Some(choice_rs.try_get("id")?),
                    position: choice_rs.try_get::<u16, _>("position")?,
                    label: choice_rs.try_get("label")?,
                },
            ))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .into_group_map();

    questions_rs
        .into_iter()
        .map(|question_rs| {
            let question_id = Uuid::parse_str(&question_rs.try_get::<String, _>("question_id")?)?;

            Ok::<_, InfraError>(QuestionDto {
                id: question_id.to_string(),
                form_id: question_rs.try_get("form_id")?,
                template_key: question_rs.try_get("template_key")?,
                position: question_rs.try_get::<u16, _>("position")?,
                title: question_rs.try_get("title")?,
                description: question_rs.try_get("description")?,
                question_type: question_rs.try_get::<String, _>("question_type")?,
                choices: choices_by_question_id
                    .get(&question_id)
                    .cloned()
                    .unwrap_or_default(),
                is_required: question_rs
                    .try_get::<Option<bool>, _>("is_required")?
                    .unwrap_or(false),
            })
        })
        .collect::<Result<Vec<QuestionDto>, _>>()
}

async fn form_dto_from_row(
    txn: &mut DatabaseTransaction,
    row: FormRowDto,
) -> Result<FormDto, InfraError> {
    let form_id = FormId::from(Uuid::parse_str(&row.id)?);

    Ok(FormDto {
        id: row.id,
        title: row.title,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
        start_at: row.start_at,
        end_at: row.end_at,
        webhook_url: row.webhook_url,
        default_answer_title: row.default_answer_title,
        visibility: row.visibility,
        answer_visibility: row.answer_visibility,
        questions: get_questions_txn_with_tables(txn, form_id, "form_questions", "form_choices")
            .await?,
    })
}

async fn archived_form_dto_from_row(
    txn: &mut DatabaseTransaction,
    row: ArchivedFormRowDto,
) -> Result<ArchivedFormDto, InfraError> {
    let form_id = FormId::from(Uuid::parse_str(&row.form.id)?);

    Ok(ArchivedFormDto {
        form: FormDto {
            id: row.form.id,
            title: row.form.title,
            description: row.form.description,
            created_at: row.form.created_at,
            updated_at: row.form.updated_at,
            start_at: row.form.start_at,
            end_at: row.form.end_at,
            webhook_url: row.form.webhook_url,
            default_answer_title: row.form.default_answer_title,
            visibility: row.form.visibility,
            answer_visibility: row.form.answer_visibility,
            questions: get_questions_txn_with_tables(
                txn,
                form_id,
                "archived_form_questions",
                "archived_form_choices",
            )
            .await?,
        },
        archived_at: row.archived_at,
        archived_by_name: row.archived_by_name,
        archived_by_id: row.archived_by_id,
        archived_by_role: row.archived_by_role,
    })
}

fn form_row_from_db_row(row: sqlx::mysql::MySqlRow) -> Result<FormRowDto, InfraError> {
    Ok(FormRowDto {
        id: row.try_get("id")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        start_at: row.try_get("start_at")?,
        end_at: row.try_get("end_at")?,
        webhook_url: row.try_get("webhook_url")?,
        default_answer_title: row.try_get("default_answer_title")?,
        visibility: row.try_get("visibility")?,
        answer_visibility: row.try_get("answer_visibility")?,
    })
}

fn archived_form_row_from_db_row(
    row: sqlx::mysql::MySqlRow,
) -> Result<ArchivedFormRowDto, InfraError> {
    Ok(ArchivedFormRowDto {
        form: FormRowDto {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            description: row.try_get("description")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            start_at: row.try_get("start_at")?,
            end_at: row.try_get("end_at")?,
            webhook_url: row.try_get("webhook_url")?,
            default_answer_title: row.try_get("default_answer_title")?,
            visibility: row.try_get("visibility")?,
            answer_visibility: row.try_get("answer_visibility")?,
        },
        archived_at: row.try_get("archived_at")?,
        archived_by_name: row.try_get("archived_by_name")?,
        archived_by_id: row.try_get("archived_by_id")?,
        archived_by_role: Role::from_str(&row.try_get::<String, _>("archived_by_role")?)?,
    })
}

async fn fetch_form_row(
    txn: &mut DatabaseTransaction,
    form_id: FormId,
) -> Result<Option<FormRowDto>, InfraError> {
    let row = sqlx::query(
        r"SELECT f.id, f.title, f.description, f.visibility, f.answer_visibility,
            f.created_at, f.updated_at, w.url AS webhook_url, p.start_at, p.end_at,
            d.title AS default_answer_title
        FROM form_meta_data f
        LEFT JOIN form_webhooks w ON f.id = w.form_id
        LEFT JOIN response_period p ON f.id = p.form_id
        LEFT JOIN default_answer_titles d ON f.id = d.form_id
        WHERE f.id = ?",
    )
    .bind(form_id.into_inner().to_string())
    .fetch_optional(&mut **txn)
    .await?;

    row.map(form_row_from_db_row).transpose()
}

async fn fetch_archived_form_row(
    txn: &mut DatabaseTransaction,
    form_id: FormId,
) -> Result<Option<ArchivedFormRowDto>, InfraError> {
    let row = sqlx::query(
        r"SELECT f.id, f.title, f.description, f.visibility, f.answer_visibility,
            f.created_at, f.updated_at, w.url AS webhook_url, p.start_at, p.end_at,
            d.title AS default_answer_title, f.archived_at, u.name AS archived_by_name,
            u.id AS archived_by_id, u.role AS archived_by_role
        FROM archived_form_meta_data f
        INNER JOIN users u ON f.archived_by = u.id
        LEFT JOIN archived_form_webhooks w ON f.id = w.form_id
        LEFT JOIN archived_response_period p ON f.id = p.form_id
        LEFT JOIN archived_default_answer_titles d ON f.id = d.form_id
        WHERE f.id = ?",
    )
    .bind(form_id.into_inner().to_string())
    .fetch_optional(&mut **txn)
    .await?;

    row.map(archived_form_row_from_db_row).transpose()
}

async fn insert_form_root(
    txn: &mut DatabaseTransaction,
    form: &Form,
    created_by: &User,
) -> Result<(), InfraError> {
    let form_id = form.id().into_inner().to_string();
    let title = form.title().to_string();
    let description = form.description().to_owned().into_inner();
    let user_id = created_by.id.to_string();

    sqlx::query(
        r#"INSERT INTO form_meta_data (id, title, description, created_by, updated_by)
        VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(form_id.clone())
    .bind(title)
    .bind(description)
    .bind(user_id.clone())
    .bind(user_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(r"INSERT INTO default_answer_titles (form_id, title) VALUES (?, NULL)")
        .bind(form_id.clone())
        .execute(&mut **txn)
        .await?;

    sqlx::query(r"INSERT INTO response_period (form_id, start_at, end_at) VALUES (?, NULL, NULL)")
        .bind(form_id.clone())
        .execute(&mut **txn)
        .await?;

    sqlx::query(r"INSERT INTO form_webhooks (form_id, url) VALUES (?, NULL)")
        .bind(form_id)
        .execute(&mut **txn)
        .await?;

    Ok(())
}

async fn update_form_root(
    txn: &mut DatabaseTransaction,
    form: &Form,
    updated_by: &User,
) -> Result<(), InfraError> {
    let form_id = form.id().into_inner().to_string();
    let title = form.title().to_owned().into_inner().into_inner();
    let description = form.description().to_owned().into_inner();
    let visibility = form.settings().visibility().to_string();
    let answer_visibility = form.settings().answer_settings().visibility().to_string();
    let default_answer_title = form
        .settings()
        .answer_settings()
        .default_answer_title()
        .to_owned()
        .into_inner()
        .map(NonEmptyString::into_inner);
    let response_period = form.settings().answer_settings().response_period();
    let updated_by_id = updated_by.id.to_string();

    let webhook_url = form
        .settings()
        .webhook_url(updated_by)
        .ok()
        .map(ToOwned::to_owned)
        .and_then(WebhookUrl::into_inner)
        .map(NonEmptyString::into_inner);

    sqlx::query(
        r#"UPDATE form_meta_data SET
            title = ?,
            description = ?,
            visibility = ?,
            answer_visibility = ?,
            updated_by = ?
            WHERE id = ?"#,
    )
    .bind(title)
    .bind(description)
    .bind(visibility)
    .bind(answer_visibility)
    .bind(updated_by_id)
    .bind(form_id.clone())
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r#"INSERT INTO form_webhooks (form_id, url) VALUES (?, ?)
        ON DUPLICATE KEY UPDATE url = VALUES(url)"#,
    )
    .bind(form_id.clone())
    .bind(webhook_url)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r#"INSERT INTO default_answer_titles (form_id, title) VALUES (?, ?)
        ON DUPLICATE KEY UPDATE title = VALUES(title)"#,
    )
    .bind(&form_id)
    .bind(default_answer_title)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r#"INSERT INTO response_period (form_id, start_at, end_at) VALUES (?, ?, ?)
        ON DUPLICATE KEY UPDATE start_at = VALUES(start_at), end_at = VALUES(end_at)"#,
    )
    .bind(&form_id)
    .bind(response_period.start_at().to_owned())
    .bind(response_period.end_at().to_owned())
    .execute(&mut **txn)
    .await?;

    Ok(())
}

async fn copy_active_form_to_archive(
    txn: &mut DatabaseTransaction,
    form_id: FormId,
    actor: &User,
) -> Result<(), InfraError> {
    let form_id = form_id.into_inner().to_string();
    let actor_id = actor.id.to_string();

    sqlx::query(
        r"INSERT INTO archived_form_meta_data
        (id, title, description, visibility, answer_visibility, created_at, created_by, updated_at, updated_by, archived_at, archived_by)
        SELECT id, title, description, visibility, answer_visibility, created_at, created_by, updated_at, updated_by, CURRENT_TIMESTAMP, ?
        FROM form_meta_data
        WHERE id = ?",
    )
    .bind(actor_id)
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_form_questions
        (question_id, form_id, template_key, position, title, description, question_type, is_required)
        SELECT question_id, form_id, template_key, position, title, description, question_type, is_required
        FROM form_questions WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_form_choices (id, question_id, position, label)
        SELECT c.id, c.question_id, c.position, c.label
        FROM form_choices c
        INNER JOIN form_questions q ON c.question_id = q.question_id
        WHERE q.form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_response_period (id, form_id, start_at, end_at)
        SELECT id, form_id, start_at, end_at FROM response_period WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_form_webhooks (id, form_id, url)
        SELECT id, form_id, url FROM form_webhooks WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_default_answer_titles (id, form_id, title)
        SELECT id, form_id, title FROM default_answer_titles WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_answers (id, form_id, user, title, timestamp)
        SELECT id, form_id, user, title, timestamp FROM answers WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_real_answers (id, answer_id, question_id, answer)
        SELECT r.id, r.answer_id, r.question_id, r.answer
        FROM real_answers r
        INNER JOIN answers a ON r.answer_id = a.id
        WHERE a.form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_form_answer_comments (id, answer_id, commented_by, content, timestamp)
        SELECT c.id, c.answer_id, c.commented_by, c.content, c.timestamp
        FROM form_answer_comments c
        INNER JOIN answers a ON c.answer_id = a.id
        WHERE a.form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_messages (id, related_answer_id, sender, body, timestamp)
        SELECT m.id, m.related_answer_id, m.sender, m.body, m.timestamp
        FROM messages m
        INNER JOIN answers a ON m.related_answer_id = a.id
        WHERE a.form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_label_settings_for_forms (id, form_id, label_id)
        SELECT id, form_id, label_id FROM label_settings_for_forms WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO archived_label_settings_for_form_answers (id, answer_id, label_id)
        SELECT s.id, s.answer_id, s.label_id
        FROM label_settings_for_form_answers s
        INNER JOIN answers a ON s.answer_id = a.id
        WHERE a.form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query("DELETE FROM form_meta_data WHERE id = ?")
        .bind(form_id)
        .execute(&mut **txn)
        .await?;

    Ok(())
}

async fn restore_archived_form_to_active(
    txn: &mut DatabaseTransaction,
    form_id: FormId,
) -> Result<(), InfraError> {
    let form_id = form_id.into_inner().to_string();

    sqlx::query(
        r"INSERT INTO form_meta_data
        (id, title, description, visibility, answer_visibility, created_at, created_by, updated_at, updated_by)
        SELECT id, title, description, visibility, answer_visibility, created_at, created_by, updated_at, updated_by
        FROM archived_form_meta_data
        WHERE id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO form_questions
        (question_id, form_id, template_key, position, title, description, question_type, is_required)
        SELECT question_id, form_id, template_key, position, title, description, question_type, is_required
        FROM archived_form_questions WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO form_choices (question_id, position, label)
        SELECT question_id, position, label
        FROM archived_form_choices
        WHERE question_id IN (
            SELECT question_id FROM archived_form_questions WHERE form_id = ?
        )",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO response_period (form_id, start_at, end_at)
        SELECT form_id, start_at, end_at FROM archived_response_period WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO form_webhooks (form_id, url)
        SELECT form_id, url FROM archived_form_webhooks WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO default_answer_titles (form_id, title)
        SELECT form_id, title FROM archived_default_answer_titles WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO answers (id, form_id, user, title, timestamp)
        SELECT id, form_id, user, title, timestamp FROM archived_answers WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO real_answers (id, answer_id, question_id, answer)
        SELECT id, answer_id, question_id, answer
        FROM archived_real_answers
        WHERE answer_id IN (SELECT id FROM archived_answers WHERE form_id = ?)",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO form_answer_comments (id, answer_id, commented_by, content, timestamp)
        SELECT id, answer_id, commented_by, content, timestamp
        FROM archived_form_answer_comments
        WHERE answer_id IN (SELECT id FROM archived_answers WHERE form_id = ?)",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO messages (id, related_answer_id, sender, body, timestamp)
        SELECT id, related_answer_id, sender, body, timestamp
        FROM archived_messages
        WHERE related_answer_id IN (SELECT id FROM archived_answers WHERE form_id = ?)",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO label_settings_for_forms (form_id, label_id)
        SELECT form_id, label_id FROM archived_label_settings_for_forms WHERE form_id = ?",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r"INSERT INTO label_settings_for_form_answers (answer_id, label_id)
        SELECT answer_id, label_id
        FROM archived_label_settings_for_form_answers
        WHERE answer_id IN (SELECT id FROM archived_answers WHERE form_id = ?)",
    )
    .bind(&form_id)
    .execute(&mut **txn)
    .await?;

    sqlx::query("DELETE FROM archived_form_meta_data WHERE id = ?")
        .bind(form_id)
        .execute(&mut **txn)
        .await?;

    Ok(())
}

#[async_trait]
impl FormDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create(&self, form: &Form, user: &User) -> Result<(), InfraError> {
        let form = form.clone();
        let user = user.clone();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                insert_form_root(txn, &form, &user).await?;
                sync_questions(txn, &form).await?;
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
                let rows = sqlx::query(
                    r"SELECT f.id, f.title, f.description, f.visibility, f.answer_visibility,
                    f.created_at, f.updated_at, w.url AS webhook_url, p.start_at, p.end_at,
                    d.title AS default_answer_title
                    FROM form_meta_data f
                    LEFT JOIN form_webhooks w ON f.id = w.form_id
                    LEFT JOIN response_period p ON f.id = p.form_id
                    LEFT JOIN default_answer_titles d ON f.id = d.form_id
                    ORDER BY f.id
                    LIMIT ? OFFSET ?",
                )
                .bind(limit.unwrap_or(u32::MAX))
                .bind(offset.unwrap_or(0))
                .fetch_all(&mut **txn)
                .await?;

                let mut forms = Vec::with_capacity(rows.len());
                for row in rows {
                    forms.push(form_dto_from_row(txn, form_row_from_db_row(row)?).await?);
                }
                Ok::<_, InfraError>(forms)
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get(&self, form_id: FormId) -> Result<Option<FormDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                match fetch_form_row(txn, form_id).await? {
                    Some(row) => form_dto_from_row(txn, row).await.map(Some),
                    None => Ok(None),
                }
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn list_archived(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
        query_text: Option<String>,
    ) -> Result<Vec<ArchivedFormDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rows = if let Some(query_text) = query_text {
                    let like = format!("%{query_text}%");
                    sqlx::query(
                        r"SELECT f.id, f.title, f.description, f.visibility, f.answer_visibility,
                        f.created_at, f.updated_at, w.url AS webhook_url, p.start_at, p.end_at,
                        d.title AS default_answer_title, f.archived_at, u.name AS archived_by_name,
                        u.id AS archived_by_id, u.role AS archived_by_role
                        FROM archived_form_meta_data f
                        INNER JOIN users u ON f.archived_by = u.id
                        LEFT JOIN archived_form_webhooks w ON f.id = w.form_id
                        LEFT JOIN archived_response_period p ON f.id = p.form_id
                        LEFT JOIN archived_default_answer_titles d ON f.id = d.form_id
                        WHERE f.title LIKE ? OR f.description LIKE ?
                        ORDER BY f.archived_at DESC, f.id
                        LIMIT ? OFFSET ?",
                    )
                    .bind(&like)
                    .bind(&like)
                    .bind(limit.unwrap_or(u32::MAX))
                    .bind(offset.unwrap_or(0))
                    .fetch_all(&mut **txn)
                    .await?
                } else {
                    sqlx::query(
                        r"SELECT f.id, f.title, f.description, f.visibility, f.answer_visibility,
                        f.created_at, f.updated_at, w.url AS webhook_url, p.start_at, p.end_at,
                        d.title AS default_answer_title, f.archived_at, u.name AS archived_by_name,
                        u.id AS archived_by_id, u.role AS archived_by_role
                        FROM archived_form_meta_data f
                        INNER JOIN users u ON f.archived_by = u.id
                        LEFT JOIN archived_form_webhooks w ON f.id = w.form_id
                        LEFT JOIN archived_response_period p ON f.id = p.form_id
                        LEFT JOIN archived_default_answer_titles d ON f.id = d.form_id
                        ORDER BY f.archived_at DESC, f.id
                        LIMIT ? OFFSET ?",
                    )
                    .bind(limit.unwrap_or(u32::MAX))
                    .bind(offset.unwrap_or(0))
                    .fetch_all(&mut **txn)
                    .await?
                };

                let mut forms = Vec::with_capacity(rows.len());
                for row in rows {
                    forms.push(
                        archived_form_dto_from_row(txn, archived_form_row_from_db_row(row)?)
                            .await?,
                    );
                }
                Ok::<_, InfraError>(forms)
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_archived(&self, form_id: FormId) -> Result<Option<ArchivedFormDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                match fetch_archived_form_row(txn, form_id).await? {
                    Some(row) => archived_form_dto_from_row(txn, row).await.map(Some),
                    None => Ok(None),
                }
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn archive(&self, form_id: FormId, actor: &User) -> Result<ArchivedForm, InfraError> {
        let actor = actor.clone();
        self.read_write_transaction(move |txn| {
            Box::pin(async move {
                if fetch_form_row(txn, form_id).await?.is_none() {
                    return Err(InfraError::FormNotFound {
                        id: form_id.into_inner(),
                    });
                }

                copy_active_form_to_archive(txn, form_id, &actor).await?;

                let row = fetch_archived_form_row(txn, form_id)
                    .await?
                    .ok_or_else(|| InfraError::FormNotFound {
                        id: form_id.into_inner(),
                    })?;

                archived_form_dto_from_row(txn, row)
                    .await?
                    .try_into()
                    .map_err(|error: errors::Error| InfraError::Unexpected {
                        cause: error.to_string(),
                    })
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn restore(&self, form_id: FormId) -> Result<(), InfraError> {
        self.read_write_transaction(move |txn| {
            Box::pin(async move {
                if fetch_archived_form_row(txn, form_id).await?.is_none() {
                    return Err(InfraError::FormNotFound {
                        id: form_id.into_inner(),
                    });
                }

                restore_archived_form_to_active(txn, form_id).await
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn update(&self, form: &Form, updated_by: &User) -> Result<(), InfraError> {
        let form = form.clone();
        let updated_by = updated_by.clone();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                update_form_root(txn, &form, &updated_by).await?;
                sync_questions(txn, &form).await?;
                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn size(&self) -> Result<u32, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let row = sqlx::query("SELECT COUNT(*) AS count FROM form_meta_data")
                    .fetch_one(&mut **txn)
                    .await?;
                let size: i64 = row.try_get("count")?;
                count_as_u32(size, "form_meta_data")
            })
        })
        .await
    }
}

async fn sync_questions(txn: &mut DatabaseTransaction, form: &Form) -> Result<(), InfraError> {
    let form_id = *form.id();
    let form_id_string = form_id.into_inner().to_string();
    let desired = form.questions().as_slice();

    let conn = &mut **txn;
    let existing_ids = fetch_question_ids(conn, &form_id_string).await?;
    let desired_ids: BTreeSet<Uuid> = desired.iter().map(|q| q.id().into_inner()).collect();
    let to_delete: Vec<Uuid> = existing_ids
        .into_iter()
        .filter(|id| !desired_ids.contains(id))
        .collect();

    temporarily_relocate_existing_questions(conn, &form_id_string).await?;
    persist_questions(conn, &form_id, desired).await?;
    delete_questions(conn, to_delete).await?;

    let assigned_questions: Vec<(QuestionId, &Question)> =
        desired.iter().map(|q| (q.id(), q)).collect();

    sync_choices(conn, &assigned_questions).await
}

async fn fetch_question_ids(
    txn: &mut MySqlConnection,
    form_id: &str,
) -> Result<Vec<Uuid>, InfraError> {
    let rows = sqlx::query(
        "SELECT question_id FROM form_questions WHERE form_id = ? ORDER BY position ASC",
    )
    .bind(form_id)
    .fetch_all(&mut *txn)
    .await?;

    rows.into_iter()
        .map(|row| Ok::<_, InfraError>(Uuid::parse_str(&row.try_get::<String, _>("question_id")?)?))
        .collect()
}

#[derive(Debug, Clone)]
struct ExistingQuestionRow {
    question_id: QuestionId,
    template_key: String,
    position: u16,
}

async fn fetch_existing_questions(
    txn: &mut MySqlConnection,
    form_id: &str,
) -> Result<Vec<ExistingQuestionRow>, InfraError> {
    let rows = sqlx::query(
        "SELECT question_id, template_key, position FROM form_questions WHERE form_id = ? ORDER BY position ASC, question_id ASC",
    )
    .bind(form_id)
    .fetch_all(&mut *txn)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok::<_, InfraError>(ExistingQuestionRow {
                question_id: Uuid::parse_str(&row.try_get::<String, _>("question_id")?)?.into(),
                template_key: row.try_get("template_key")?,
                position: row.try_get("position")?,
            })
        })
        .collect()
}

fn temporary_template_key_prefix(existing_template_keys: &[String]) -> String {
    std::iter::successors(Some("__tmp_form_question__".to_string()), |prefix| {
        Some(format!("{prefix}_"))
    })
    .find(|prefix| {
        existing_template_keys
            .iter()
            .all(|template_key| !template_key.starts_with(prefix))
    })
    .expect("successors yields at least one prefix candidate")
}

async fn temporarily_relocate_existing_questions(
    txn: &mut MySqlConnection,
    form_id: &str,
) -> Result<(), InfraError> {
    let existing_questions = fetch_existing_questions(txn, form_id).await?;
    if existing_questions.is_empty() {
        return Ok(());
    }

    let position_offset =
        u16::try_from(existing_questions.len()).map_err(|_| InfraError::Unexpected {
            cause: "too many questions to relocate temporarily".to_string(),
        })?;
    let temporary_prefix = temporary_template_key_prefix(
        &existing_questions
            .iter()
            .map(|question| question.template_key.clone())
            .collect_vec(),
    );
    existing_questions.iter().try_for_each(|question| {
        question
            .position
            .checked_add(position_offset)
            .ok_or_else(|| InfraError::Unexpected {
                cause: format!(
                    "temporary position overflow for question_id {}",
                    question.question_id.into_inner()
                ),
            })
            .map(|_| ())
    })?;

    sqlx::query(
        "UPDATE form_questions
         SET template_key = CONCAT(?, question_id),
             position = position + ?
         WHERE form_id = ?",
    )
    .bind(temporary_prefix)
    .bind(position_offset)
    .bind(form_id)
    .execute(&mut *txn)
    .await?;

    Ok(())
}

async fn persist_questions(
    txn: &mut MySqlConnection,
    form_id: &FormId,
    questions: &[Question],
) -> Result<(), InfraError> {
    if questions.is_empty() {
        return Ok(());
    }

    let form_id_string = form_id.into_inner().to_string();
    let sql = format!(
        r"INSERT INTO form_questions (question_id, form_id, template_key, position, title, description, question_type, is_required)
        VALUES {}
        ON DUPLICATE KEY UPDATE
        template_key = VALUES(template_key),
        position = VALUES(position),
        title = VALUES(title),
        description = VALUES(description),
        question_type = VALUES(question_type),
        is_required = VALUES(is_required)",
        std::iter::repeat_n("(?, ?, ?, ?, ?, ?, ?, ?)", questions.len()).join(", ")
    );

    questions
        .iter()
        .fold(query(&sql), |query, question| {
            query
                .bind(question.id().into_inner().to_string())
                .bind(&form_id_string)
                .bind(question.template_key().to_owned().into_inner())
                .bind(question.position())
                .bind(question.title().to_owned().into_inner())
                .bind(
                    question
                        .description()
                        .cloned()
                        .map(|description| description.into_inner()),
                )
                .bind(question.question_type().to_string())
                .bind(question.is_required())
        })
        .execute(&mut *txn)
        .await?;

    Ok(())
}

async fn delete_questions(
    txn: &mut MySqlConnection,
    question_ids: Vec<Uuid>,
) -> Result<(), InfraError> {
    if question_ids.is_empty() {
        return Ok(());
    }

    let sql = format!(
        "DELETE FROM form_questions WHERE question_id IN ({})",
        std::iter::repeat_n("?", question_ids.len()).join(", ")
    );
    question_ids
        .iter()
        .fold(query(&sql), |query, question_id| {
            query.bind(question_id.to_string())
        })
        .execute(&mut *txn)
        .await?;

    Ok(())
}

async fn sync_choices(
    txn: &mut MySqlConnection,
    assigned_questions: &[(QuestionId, &Question)],
) -> Result<(), InfraError> {
    let question_ids = assigned_questions
        .iter()
        .map(|(question_id, _)| *question_id)
        .collect_vec();
    if question_ids.is_empty() {
        return Ok(());
    }

    let desired_choices: Vec<(QuestionId, &domain::form::question::models::Choice)> =
        assigned_questions
            .iter()
            .flat_map(|(question_id, question)| {
                let accepts_new_choices = question.question_type() != QuestionType::Text;
                question.choices().into_iter().flat_map(move |choices| {
                    choices.iter().filter_map(move |choice| {
                        if choice.id.is_some() || accepts_new_choices {
                            Some((*question_id, choice))
                        } else {
                            None
                        }
                    })
                })
            })
            .collect();

    let existing_choice_owners = fetch_existing_choices(txn, &question_ids).await?;
    let existing_ids: BTreeSet<i32> = existing_choice_owners.keys().copied().collect();

    let (to_upsert, to_insert): (Vec<(QuestionId, &_)>, Vec<(QuestionId, &_)>) =
        desired_choices.iter().copied().partition(
            |(_, choice)| matches!(choice.id, Some(id) if existing_ids.contains(&id.into_inner())),
        );
    let retained: BTreeSet<i32> = to_upsert
        .iter()
        .filter_map(|(_, choice)| choice.id.map(|id| id.into_inner()))
        .collect();
    let to_delete: Vec<i32> = existing_ids.difference(&retained).copied().collect();

    delete_choices(txn, to_delete).await?;

    let upsert_rows = to_upsert
        .into_iter()
        .map(|(question_id, choice)| {
            (
                choice
                    .id
                    .expect("to_upsert items have Some(id)")
                    .into_inner(),
                question_id,
                choice.position,
                choice.label.to_owned().into_inner(),
            )
        })
        .collect_vec();
    upsert_existing_choices(txn, upsert_rows).await?;

    let insert_rows = to_insert
        .into_iter()
        .map(|(question_id, choice)| {
            (
                question_id,
                choice.position,
                choice.label.to_owned().into_inner(),
            )
        })
        .collect_vec();
    insert_new_choices(txn, insert_rows).await
}

async fn fetch_existing_choices(
    txn: &mut MySqlConnection,
    question_ids: &[QuestionId],
) -> Result<BTreeMap<i32, QuestionId>, InfraError> {
    let sql = format!(
        "SELECT id, question_id FROM form_choices WHERE question_id IN ({})",
        std::iter::repeat_n("?", question_ids.len()).join(", ")
    );

    let rows = question_ids
        .iter()
        .fold(sqlx::query(&sql), |query, question_id| {
            query.bind(question_id.into_inner().to_string())
        })
        .fetch_all(&mut *txn)
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok::<_, InfraError>((
                row.try_get("id")?,
                Uuid::parse_str(&row.try_get::<String, _>("question_id")?)?.into(),
            ))
        })
        .collect()
}

async fn delete_choices(txn: &mut MySqlConnection, choice_ids: Vec<i32>) -> Result<(), InfraError> {
    if choice_ids.is_empty() {
        return Ok(());
    }

    let sql = format!(
        "DELETE FROM form_choices WHERE id IN ({})",
        std::iter::repeat_n("?", choice_ids.len()).join(", ")
    );
    choice_ids
        .iter()
        .fold(query(&sql), |query, choice_id| query.bind(choice_id))
        .execute(&mut *txn)
        .await?;

    Ok(())
}

async fn upsert_existing_choices(
    txn: &mut MySqlConnection,
    choices: Vec<(i32, QuestionId, u16, String)>,
) -> Result<(), InfraError> {
    if choices.is_empty() {
        return Ok(());
    }

    let sql = format!(
        r"INSERT INTO form_choices (id, question_id, position, label) VALUES {}
        ON DUPLICATE KEY UPDATE
        question_id = VALUES(question_id),
        position = VALUES(position),
        label = VALUES(label)",
        std::iter::repeat_n("(?, ?, ?, ?)", choices.len()).join(", ")
    );
    choices
        .iter()
        .fold(
            query(&sql),
            |query, (choice_id, question_id, position, label)| {
                query
                    .bind(choice_id)
                    .bind(question_id.into_inner().to_string())
                    .bind(position)
                    .bind(label)
            },
        )
        .execute(&mut *txn)
        .await?;

    Ok(())
}

async fn insert_new_choices(
    txn: &mut MySqlConnection,
    choices: Vec<(QuestionId, u16, String)>,
) -> Result<(), InfraError> {
    if choices.is_empty() {
        return Ok(());
    }

    let sql = format!(
        "INSERT INTO form_choices (question_id, position, label) VALUES {}",
        std::iter::repeat_n("(?, ?, ?)", choices.len()).join(", ")
    );
    choices
        .iter()
        .fold(query(&sql), |query, (question_id, position, label)| {
            query
                .bind(question_id.into_inner().to_string())
                .bind(position)
                .bind(label)
        })
        .execute(&mut *txn)
        .await?;

    Ok(())
}
