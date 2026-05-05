use async_trait::async_trait;
use domain::form::{
    models::WebhookUrl,
    question::models::{Question, QuestionId, QuestionType},
};
use domain::{
    form::models::{Form, FormId},
    user::models::User,
};
use errors::infra::InfraError;
use itertools::Itertools;
use sqlx::{MySqlConnection, Row, query};
use std::collections::{BTreeMap, BTreeSet};
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

use crate::{
    database::{
        components::FormDatabase,
        connection::{ConnectionPool, DatabaseTransaction},
        count::count_as_u32,
    },
    dto::{ChoiceDto, FormDto, QuestionDto},
};

struct FormRowDto {
    id: String,
    title: String,
    description: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    start_at: Option<chrono::DateTime<chrono::Utc>>,
    end_at: Option<chrono::DateTime<chrono::Utc>>,
    webhook_url: Option<String>,
    default_answer_title: Option<String>,
    visibility: String,
    answer_visibility: String,
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
        questions: get_questions_txn(txn, form_id).await?,
    })
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
                let form_rows = sqlx::query_as!(
                    FormRowDto,
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

                let mut forms = Vec::with_capacity(form_rows.len());
                for form_row in form_rows {
                    forms.push(form_dto_from_row(txn, form_row).await?);
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
                let form_row = sqlx::query_as!(
                    FormRowDto,
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

                match form_row {
                    Some(form_row) => form_dto_from_row(txn, form_row).await.map(Some),
                    None => Ok(None),
                }
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn delete(&self, form_id: FormId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM form_meta_data WHERE id = ?",
                    form_id.into_inner().to_string(),
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
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

// ---------------------------------------------------------------------------
// Form root (form_meta_data + 1:1 child tables: form_webhooks,
// default_answer_titles, response_period)
// ---------------------------------------------------------------------------

async fn insert_form_root(
    txn: &mut DatabaseTransaction,
    form: &Form,
    created_by: &User,
) -> Result<(), InfraError> {
    let form_id = form.id().into_inner().to_string();
    let title = form.title().to_string();
    let description = form.description().to_owned().into_inner();
    let user_id = created_by.id.to_string();

    sqlx::query!(
        r#"INSERT INTO form_meta_data (id, title, description, created_by, updated_by)
                            VALUES (?, ?, ?, ?, ?)"#,
        form_id,
        title,
        description,
        user_id,
        user_id,
    )
    .execute(&mut **txn)
    .await?;

    sqlx::query!(
        r"INSERT INTO default_answer_titles (form_id, title) VALUES (?, NULL)",
        form_id,
    )
    .execute(&mut **txn)
    .await?;

    sqlx::query!(
        r"INSERT INTO response_period (form_id, start_at, end_at) VALUES (?, NULL, NULL)",
        form_id,
    )
    .execute(&mut **txn)
    .await?;

    sqlx::query!(
        r"INSERT INTO form_webhooks (form_id, url) VALUES (?, NULL)",
        form_id,
    )
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

    sqlx::query!(
        r#"UPDATE form_meta_data SET
                    title = ?,
                    description = ?,
                    visibility = ?,
                    answer_visibility = ?,
                    updated_by = ?
                    WHERE id = ?
                    "#,
        title,
        description,
        visibility,
        answer_visibility,
        updated_by_id,
        form_id,
    )
    .execute(&mut **txn)
    .await?;

    sqlx::query!(
        r#"INSERT INTO form_webhooks (form_id, url) VALUES (?, ?)
                    ON DUPLICATE KEY UPDATE
                    url = VALUES(url)
                    "#,
        form_id,
        webhook_url,
    )
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r#"INSERT INTO default_answer_titles (form_id, title) VALUES (?, ?)
        ON DUPLICATE KEY UPDATE title = VALUES(title)
        "#,
    )
    .bind(&form_id)
    .bind(default_answer_title)
    .execute(&mut **txn)
    .await?;

    sqlx::query(
        r#"INSERT INTO response_period (form_id, start_at, end_at) VALUES (?, ?, ?)
        ON DUPLICATE KEY UPDATE
        start_at = VALUES(start_at),
        end_at = VALUES(end_at)
        "#,
    )
    .bind(&form_id)
    .bind(response_period.start_at().to_owned())
    .bind(response_period.end_at().to_owned())
    .execute(&mut **txn)
    .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Question 子集合の同期
//
// ドメイン側 (Form 集約) の questions を desired、DB 側の現状を current として
// 突き合わせ、DB を desired に一致させる。Question id はクライアント採番 (Uuid)
// なので、desired 全件を 1 本の INSERT ... ON DUPLICATE KEY UPDATE で永続化し、
// desired に含まれない既存 id を削除すればよい。ただし (form_id, template_key)
// と (form_id, position) の UNIQUE 制約があるため、upsert の前に既存行を一時値へ
// 退避する。
// ---------------------------------------------------------------------------

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

pub(crate) async fn get_questions_txn(
    txn: &mut DatabaseTransaction,
    form_id: FormId,
) -> Result<Vec<QuestionDto>, InfraError> {
    let form_id = form_id.into_inner().to_string();
    let questions_rs = sqlx::query(
        r"SELECT question_id, form_id, template_key, position, title, description, question_type, is_required
        FROM form_questions
        WHERE form_id = ?
        ORDER BY position ASC, question_id ASC",
    )
    .bind(form_id.clone())
    .fetch_all(&mut **txn)
    .await?;

    let choices_by_question_id = sqlx::query(
        r"SELECT form_choices.id, form_choices.question_id, form_choices.position, form_choices.label
        FROM form_choices
        INNER JOIN form_questions ON form_choices.question_id = form_questions.question_id
        WHERE form_questions.form_id = ?
        ORDER BY form_choices.position ASC, form_choices.id ASC",
    )
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

/// 既存 Question 行の (form_id, template_key) と (form_id, position) は
/// UNIQUE 制約下にある。upsert で desired の新しい template_key/position を
/// 入れるとき、別の既存行がまだ古い値を保持していると衝突するため、先に
/// 全既存行を一意な一時値へ退避させる。upsert はその後で desired の値で
/// 上書きする。
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

// ---------------------------------------------------------------------------
// Choice 子集合の同期
// ---------------------------------------------------------------------------

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

    // desired (期待状態) として diff_children に渡す Choice 集合を構築する。
    // 質問種別が Text の場合、新規 Choice (id == None) は持ちえないため挿入対象外にする。
    // ただし既に DB にあった Choice (id == Some) は upsert/delete 判定に乗せたいので残す。
    // それ以外の質問種別では全 Choice を desired に含める。
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

    // desired と DB の差分を取って delete / upsert / insert を適用する。
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
