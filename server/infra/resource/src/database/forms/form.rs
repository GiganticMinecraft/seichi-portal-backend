use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::account::models::UserGroupId;
use domain::form::{
    answer::{AnswerEntry, AnswerId, AnswerPagePosition, AnswerPublication},
    models::{ArchivedFormPagePosition, FormLabelId, FormPagePosition, FormSettings},
    question::{Choice, Question, QuestionId, QuestionType},
};
use domain::{
    account::models::{AccountUser, Role},
    auth::Actor,
    form::models::{ActiveForm, ArchivedForm, FormId},
    pagination::{Page, PageRequest},
    repository::form::answer_entry_repository::AnswerListFilter,
};
use errors::{Error, infra::InfraError};
use itertools::Itertools;
use sqlx::{AssertSqlSafe, MySql, MySqlConnection, QueryBuilder, Row, query};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::str::FromStr;
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

use crate::{
    database::forms::answers::{
        attach_entry_children, author_from_values, fetch_messages_by_answer_ids,
        fetch_real_answers_by_answer_ids,
    },
    database::{
        components::FormDatabase,
        connection::{ConnectionPool, DatabaseTransaction},
        count::count_as_u32,
    },
    records::{
        ActiveFormRecord, ArchivedFormRecord, ChoiceRecord, FormAnswerRecord, QuestionRecord,
    },
};

struct FormRow {
    id: String,
    title: String,
    description: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    discord_webhook_url: Option<String>,
    visibility: String,
    answer_visibility: String,
    answer_response_visibility: String,
    hide_author: bool,
    allow_temporary_answers: bool,
    acceptance_period_start_at: Option<DateTime<Utc>>,
    acceptance_period_end_at: Option<DateTime<Utc>>,
    default_answer_title: Option<String>,
}

struct ArchivedFormRow {
    form: FormRow,
    archived_at: DateTime<Utc>,
    archived_by_name: String,
    archived_by_id: String,
    archived_by_role: Role,
}

struct ArchivedFormQueryRow {
    id: String,
    title: String,
    description: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    discord_webhook_url: Option<String>,
    visibility: String,
    answer_visibility: String,
    answer_response_visibility: String,
    hide_author: bool,
    allow_temporary_answers: bool,
    acceptance_period_start_at: Option<DateTime<Utc>>,
    acceptance_period_end_at: Option<DateTime<Utc>>,
    default_answer_title: Option<String>,
    archived_at: DateTime<Utc>,
    archived_by_name: String,
    archived_by_id: String,
    archived_by_role: String,
}

#[derive(sqlx::FromRow)]
struct AnswerQueryRow {
    form_id: String,
    form_revision_id: String,
    answer_id: String,
    title: Option<String>,
    publication: String,
    status: String,
    author_type: String,
    user: Option<String>,
    user_name: Option<String>,
    user_role: Option<String>,
    temporary_user_id: Option<String>,
    temporary_user_name: Option<String>,
    temporary_user_contact_text: Option<String>,
    redmine_user_id: Option<i64>,
    redmine_author_name: Option<String>,
    redmine_issue_id: Option<i64>,
    timestamp: DateTime<Utc>,
}

macro_rules! execute_typed_query {
    ($txn:expr, $sql:literal $(, $argument:expr)* $(,)?) => {
        sqlx::query!($sql $(, $argument)*)
            .execute(&mut **$txn)
            .await?
    };
}

impl TryFrom<ArchivedFormQueryRow> for ArchivedFormRow {
    type Error = InfraError;

    fn try_from(row: ArchivedFormQueryRow) -> Result<Self, Self::Error> {
        Ok(Self {
            form: FormRow {
                id: row.id,
                title: row.title,
                description: row.description,
                created_at: row.created_at,
                updated_at: row.updated_at,
                discord_webhook_url: row.discord_webhook_url,
                visibility: row.visibility,
                answer_visibility: row.answer_visibility,
                answer_response_visibility: row.answer_response_visibility,
                hide_author: row.hide_author,
                allow_temporary_answers: row.allow_temporary_answers,
                acceptance_period_start_at: row.acceptance_period_start_at,
                acceptance_period_end_at: row.acceptance_period_end_at,
                default_answer_title: row.default_answer_title,
            },
            archived_at: row.archived_at,
            archived_by_name: row.archived_by_name,
            archived_by_id: row.archived_by_id,
            archived_by_role: Role::from_str(&row.archived_by_role)?,
        })
    }
}

/// active / archived で異なる、フォームに付随するテーブル名の組。
#[derive(Clone, Copy)]
struct FormTables {
    questions: &'static str,
    choices: &'static str,
    revisions: &'static str,
}

const ACTIVE_FORM_TABLES: FormTables = FormTables {
    questions: "form_questions",
    choices: "form_choices",
    revisions: "form_revisions",
};

const ARCHIVED_FORM_TABLES: FormTables = FormTables {
    questions: "archived_form_questions",
    choices: "archived_form_choices",
    revisions: "archived_form_revisions",
};

/// 複数フォームの質問と選択肢をまとめて取り、フォーム ID ごとに分ける。
///
/// フォーム一覧でフォームごとに質問・選択肢・リビジョンを引くと 3N 回のクエリになるため、
/// 一覧に含まれるフォーム ID をまとめて IN 句で引く。
async fn fetch_questions_batch(
    txn: &mut DatabaseTransaction,
    tables: FormTables,
    form_ids: &[String],
) -> Result<HashMap<String, Vec<QuestionRecord>>, InfraError> {
    if form_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // 対象テーブル (active / archived) と form_id の数で SQL が変わるため typed query にできない
    let placeholders = std::iter::repeat_n("?", form_ids.len()).join(", ");
    let FormTables {
        questions: questions_table,
        choices: choices_table,
        ..
    } = tables;
    let questions_sql = format!(
        "SELECT question_id, form_id, template_key, position, title, description, question_type, is_required
        FROM {questions_table}
        WHERE form_id IN ({placeholders})
        ORDER BY form_id ASC, position ASC, question_id ASC"
    );
    let choices_sql = format!(
        "SELECT c.id, c.question_id, c.position, c.label
        FROM {choices_table} c
        INNER JOIN {questions_table} q ON c.question_id = q.question_id
        WHERE q.form_id IN ({placeholders})
        ORDER BY c.position ASC, c.id ASC"
    );

    let questions_rs = form_ids
        .iter()
        .fold(sqlx::query(AssertSqlSafe(&*questions_sql)), |q, fid| {
            q.bind(fid)
        })
        .fetch_all(&mut **txn)
        .await?;

    let choices_by_question_id = form_ids
        .iter()
        .fold(sqlx::query(AssertSqlSafe(&*choices_sql)), |q, fid| {
            q.bind(fid)
        })
        .fetch_all(&mut **txn)
        .await?
        .into_iter()
        .map(|choice_rs| {
            let question_id = Uuid::parse_str(&choice_rs.try_get::<String, _>("question_id")?)?;
            Ok::<_, InfraError>((
                question_id,
                ChoiceRecord {
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
            let form_id: String = question_rs.try_get("form_id")?;

            Ok::<_, InfraError>((
                form_id.clone(),
                QuestionRecord {
                    id: question_id.to_string(),
                    form_id,
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
                },
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|pairs| pairs.into_iter().into_group_map())
}

/// 複数フォームの最新リビジョン ID をまとめて取る。
async fn fetch_latest_revision_ids_batch(
    txn: &mut DatabaseTransaction,
    tables: FormTables,
    form_ids: &[String],
) -> Result<HashMap<String, String>, InfraError> {
    if form_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // 対象テーブル (active / archived) と form_id の数で SQL が変わるため typed query にできない
    let placeholders = std::iter::repeat_n("?", form_ids.len()).join(", ");
    let revisions_table = tables.revisions;
    let sql = format!(
        "SELECT r.form_id, r.id
        FROM {revisions_table} r
        INNER JOIN (
            SELECT form_id, MAX(revision_number) AS revision_number
            FROM {revisions_table}
            WHERE form_id IN ({placeholders})
            GROUP BY form_id
        ) latest ON latest.form_id = r.form_id AND latest.revision_number = r.revision_number"
    );

    form_ids
        .iter()
        .fold(sqlx::query(AssertSqlSafe(&*sql)), |q, fid| q.bind(fid))
        .fetch_all(&mut **txn)
        .await?
        .into_iter()
        .map(|row| Ok::<_, InfraError>((row.try_get("form_id")?, row.try_get("id")?)))
        .collect()
}

/// 一覧のフォームに付随する質問と最新リビジョンをまとめて取ったもの。
struct FormDetailsBatch {
    questions: HashMap<String, Vec<QuestionRecord>>,
    revision_ids: HashMap<String, String>,
}

impl FormDetailsBatch {
    async fn fetch(
        txn: &mut DatabaseTransaction,
        tables: FormTables,
        form_ids: &[String],
    ) -> Result<Self, InfraError> {
        Ok(Self {
            questions: fetch_questions_batch(txn, tables, form_ids).await?,
            revision_ids: fetch_latest_revision_ids_batch(txn, tables, form_ids).await?,
        })
    }

    /// `form_id` の最新リビジョン ID と質問を取り出す。
    fn take(&mut self, form_id: &str) -> Result<(String, Vec<QuestionRecord>), InfraError> {
        let revision_id =
            self.revision_ids
                .remove(form_id)
                .ok_or_else(|| InfraError::Unexpected {
                    cause: format!("form {form_id} has no revision"),
                })?;

        Ok((
            revision_id,
            self.questions.remove(form_id).unwrap_or_default(),
        ))
    }
}

async fn fetch_label_ids(
    txn: &mut DatabaseTransaction,
    labels_table: &str,
    form_id: &str,
) -> Result<Vec<FormLabelId>, InfraError> {
    let labels_sql =
        format!("SELECT label_id FROM {labels_table} WHERE form_id = ? ORDER BY id ASC");
    sqlx::query(AssertSqlSafe(&*labels_sql))
        .bind(form_id)
        .fetch_all(&mut **txn)
        .await?
        .into_iter()
        .map(|r| {
            Ok::<_, InfraError>(FormLabelId::from(Uuid::parse_str(
                &r.try_get::<String, _>("label_id")?,
            )?))
        })
        .collect()
}

async fn fetch_label_ids_batch(
    txn: &mut DatabaseTransaction,
    labels_table: &str,
    form_ids: &[String],
) -> Result<HashMap<String, Vec<FormLabelId>>, InfraError> {
    if form_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = std::iter::repeat_n("?", form_ids.len()).join(", ");
    let sql = format!(
        "SELECT form_id, label_id FROM {labels_table} WHERE form_id IN ({placeholders}) ORDER BY id ASC"
    );

    let rows = form_ids
        .iter()
        .fold(query(AssertSqlSafe(&*sql)), |q, fid| q.bind(fid))
        .fetch_all(&mut **txn)
        .await?;

    rows.into_iter()
        .map(|row| {
            let form_id: String = row.try_get("form_id")?;
            let label_id =
                FormLabelId::from(Uuid::parse_str(&row.try_get::<String, _>("label_id")?)?);
            Ok::<_, InfraError>((form_id, label_id))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|pairs| pairs.into_iter().into_group_map())
}

fn build_active_form_record(
    row: FormRow,
    restrictions: FormGroupRestrictions,
    label_ids: Vec<FormLabelId>,
    revision_id: String,
    questions: Vec<QuestionRecord>,
) -> ActiveFormRecord {
    ActiveFormRecord {
        id: row.id,
        revision_id,
        title: row.title,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
        discord_webhook_url: row.discord_webhook_url,
        visibility: row.visibility,
        answer_visibility: row.answer_visibility,
        answer_response_visibility: row.answer_response_visibility,
        hide_author: row.hide_author,
        allow_temporary_answers: row.allow_temporary_answers,
        acceptance_period_start_at: row.acceptance_period_start_at,
        acceptance_period_end_at: row.acceptance_period_end_at,
        default_answer_title: row.default_answer_title,
        allowed_group_ids: restrictions.allowed_group_ids,
        answer_group_ids: restrictions.answer_group_ids,
        questions,
        label_ids,
    }
}

/// 一覧のフォーム行から、付随する質問・リビジョンをまとめて引いてレコードを組み立てる。
async fn build_active_form_records(
    txn: &mut DatabaseTransaction,
    form_rows: Vec<FormRow>,
    mut restrictions: HashMap<String, FormGroupRestrictions>,
    mut labels: HashMap<String, Vec<FormLabelId>>,
) -> Result<Vec<ActiveFormRecord>, InfraError> {
    let form_ids = form_rows.iter().map(|row| row.id.clone()).collect_vec();
    let mut details = FormDetailsBatch::fetch(txn, ACTIVE_FORM_TABLES, &form_ids).await?;

    form_rows
        .into_iter()
        .map(|row| {
            let restrictions = restrictions.remove(&row.id).unwrap_or_default();
            let label_ids = labels.remove(&row.id).unwrap_or_default();
            let (revision_id, questions) = details.take(&row.id)?;

            Ok(build_active_form_record(
                row,
                restrictions,
                label_ids,
                revision_id,
                questions,
            ))
        })
        .collect()
}

async fn active_form_record_from_row(
    txn: &mut DatabaseTransaction,
    row: FormRow,
) -> Result<ActiveFormRecord, InfraError> {
    let form_id = FormId::from(Uuid::parse_str(&row.id)?);
    let restrictions = FormGroupRestrictions {
        allowed_group_ids: fetch_form_group_restriction_ids(
            txn,
            "form_allowed_user_groups",
            form_id,
        )
        .await?,
        answer_group_ids: fetch_form_group_restriction_ids(txn, "form_answer_groups", form_id)
            .await?,
    };

    let label_ids = fetch_label_ids(txn, "label_settings_for_forms", &row.id).await?;

    let mut details =
        FormDetailsBatch::fetch(txn, ACTIVE_FORM_TABLES, std::slice::from_ref(&row.id)).await?;
    let (revision_id, questions) = details.take(&row.id)?;

    Ok(build_active_form_record(
        row,
        restrictions,
        label_ids,
        revision_id,
        questions,
    ))
}

async fn fetch_form_group_restriction_ids(
    txn: &mut DatabaseTransaction,
    table_name: &str,
    form_id: FormId,
) -> Result<Vec<UserGroupId>, InfraError> {
    let sql = format!("SELECT group_id FROM {table_name} WHERE form_id = ? ORDER BY id ASC");
    let rows = sqlx::query(AssertSqlSafe(&*sql))
        .bind(form_id.into_inner().to_string())
        .fetch_all(&mut **txn)
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok::<_, InfraError>(UserGroupId::from(Uuid::parse_str(
                &row.try_get::<String, _>("group_id")?,
            )?))
        })
        .collect()
}

#[derive(Default)]
struct FormGroupRestrictions {
    allowed_group_ids: Vec<UserGroupId>,
    answer_group_ids: Vec<UserGroupId>,
}

async fn fetch_group_ids_batch(
    txn: &mut DatabaseTransaction,
    table_name: &str,
    form_ids: &[String],
) -> Result<HashMap<String, Vec<UserGroupId>>, InfraError> {
    let placeholders = std::iter::repeat_n("?", form_ids.len()).join(", ");
    let sql = format!(
        "SELECT form_id, group_id FROM {table_name} WHERE form_id IN ({placeholders}) ORDER BY id ASC"
    );

    let rows = form_ids
        .iter()
        .fold(query(AssertSqlSafe(&*sql)), |q, fid| q.bind(fid))
        .fetch_all(&mut **txn)
        .await?;

    rows.into_iter()
        .map(|row| {
            let form_id: String = row.try_get("form_id")?;
            let group_id =
                UserGroupId::from(Uuid::parse_str(&row.try_get::<String, _>("group_id")?)?);
            Ok::<_, InfraError>((form_id, group_id))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|pairs| pairs.into_iter().into_group_map())
}

async fn fetch_form_group_restrictions_batch(
    txn: &mut DatabaseTransaction,
    table_prefix: &str,
    form_ids: &[String],
) -> Result<HashMap<String, FormGroupRestrictions>, InfraError> {
    if form_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut allowed =
        fetch_group_ids_batch(txn, &format!("{table_prefix}allowed_user_groups"), form_ids).await?;
    let mut answer =
        fetch_group_ids_batch(txn, &format!("{table_prefix}answer_groups"), form_ids).await?;

    Ok(form_ids
        .iter()
        .map(|fid| {
            (
                fid.clone(),
                FormGroupRestrictions {
                    allowed_group_ids: allowed.remove(fid).unwrap_or_default(),
                    answer_group_ids: answer.remove(fid).unwrap_or_default(),
                },
            )
        })
        .collect())
}

fn build_archived_form_record(
    row: ArchivedFormRow,
    restrictions: FormGroupRestrictions,
    label_ids: Vec<FormLabelId>,
    revision_id: String,
    questions: Vec<QuestionRecord>,
) -> ArchivedFormRecord {
    ArchivedFormRecord {
        form: build_active_form_record(row.form, restrictions, label_ids, revision_id, questions),
        archived_at: row.archived_at,
        archived_by_name: row.archived_by_name,
        archived_by_id: row.archived_by_id,
        archived_by_role: row.archived_by_role,
    }
}

/// 一覧のアーカイブ済みフォーム行から、付随する質問・リビジョンをまとめて引いてレコードを組み立てる。
async fn build_archived_form_records(
    txn: &mut DatabaseTransaction,
    form_rows: Vec<ArchivedFormRow>,
    mut restrictions: HashMap<String, FormGroupRestrictions>,
    mut labels: HashMap<String, Vec<FormLabelId>>,
) -> Result<Vec<ArchivedFormRecord>, InfraError> {
    let form_ids = form_rows
        .iter()
        .map(|row| row.form.id.clone())
        .collect_vec();
    let mut details = FormDetailsBatch::fetch(txn, ARCHIVED_FORM_TABLES, &form_ids).await?;

    form_rows
        .into_iter()
        .map(|row| {
            let restrictions = restrictions.remove(&row.form.id).unwrap_or_default();
            let label_ids = labels.remove(&row.form.id).unwrap_or_default();
            let (revision_id, questions) = details.take(&row.form.id)?;

            Ok(build_archived_form_record(
                row,
                restrictions,
                label_ids,
                revision_id,
                questions,
            ))
        })
        .collect()
}

async fn archived_form_record_from_row(
    txn: &mut DatabaseTransaction,
    row: ArchivedFormRow,
) -> Result<ArchivedFormRecord, InfraError> {
    let form_id = FormId::from(Uuid::parse_str(&row.form.id)?);
    let restrictions = FormGroupRestrictions {
        allowed_group_ids: fetch_form_group_restriction_ids(
            txn,
            "archived_form_allowed_user_groups",
            form_id,
        )
        .await?,
        answer_group_ids: fetch_form_group_restriction_ids(
            txn,
            "archived_form_answer_groups",
            form_id,
        )
        .await?,
    };

    let label_ids = fetch_label_ids(txn, "archived_label_settings_for_forms", &row.form.id).await?;

    let mut details = FormDetailsBatch::fetch(
        txn,
        ARCHIVED_FORM_TABLES,
        std::slice::from_ref(&row.form.id),
    )
    .await?;
    let (revision_id, questions) = details.take(&row.form.id)?;

    Ok(build_archived_form_record(
        row,
        restrictions,
        label_ids,
        revision_id,
        questions,
    ))
}

async fn fetch_answer_entries_page(
    txn: &mut DatabaseTransaction,
    form_id: Option<FormId>,
    request: PageRequest<AnswerPagePosition>,
    filter: AnswerListFilter,
) -> Result<Page<AnswerEntry, AnswerPagePosition>, InfraError> {
    if filter.form_ids().is_some_and(<[FormId]>::is_empty) {
        return Ok(Page::new(Vec::new(), None));
    }

    // Repeated query parameters require dynamic IN clauses. SQL fragments are kept static and
    // every value from the filter is bound separately to preserve SQL injection protection.
    let mut query_builder = QueryBuilder::<MySql>::new(
        r"SELECT answers.form_id, answers.form_revision_id, answers.id AS answer_id, answers.title, answers.publication,
            answers.status,
            answers.author_type, answers.user, users.name AS user_name, users.role AS user_role,
            answers.temporary_user_id, temporary_users.name AS temporary_user_name,
            temporary_users.contact_text AS temporary_user_contact_text,
            answers.redmine_user_id, answers.redmine_author_name,
            redmine_reference.redmine_issue_id,
            answers.timestamp
        FROM answers
        LEFT JOIN users ON answers.user = users.id
        LEFT JOIN temporary_users ON answers.temporary_user_id = temporary_users.id
        LEFT JOIN redmine_imported_answer_references redmine_reference
            ON redmine_reference.answer_id = answers.id
        WHERE 1 = 1",
    );

    if let Some(form_id) = form_id {
        query_builder
            .push(" AND answers.form_id = ")
            .push_bind(form_id.into_inner().to_string());
    }

    if let Some(form_ids) = filter.form_ids() {
        query_builder.push(" AND answers.form_id IN (");
        let mut separated = query_builder.separated(", ");
        for form_id in form_ids {
            separated.push_bind(form_id.into_inner().to_string());
        }
        separated.push_unseparated(")");
    }

    if !filter.statuses().is_empty() {
        query_builder.push(" AND answers.status IN (");
        let mut separated = query_builder.separated(", ");
        for status in filter.statuses() {
            separated.push_bind(status.to_string());
        }
        separated.push_unseparated(")");
    }

    if let Some(user_id) = filter.user_id() {
        query_builder
            .push(" AND answers.user = ")
            .push_bind(user_id.into_inner().to_string());
    }

    if !filter.label_ids().is_empty() {
        let label_ids = filter.label_ids().iter().unique().collect_vec();
        query_builder.push(
            r#" AND answers.id IN (
                SELECT answer_id
                FROM label_settings_for_form_answers
                WHERE label_id IN ("#,
        );
        let mut separated = query_builder.separated(", ");
        for label_id in &label_ids {
            separated.push_bind(label_id.into_inner().to_string());
        }
        separated.push_unseparated(") GROUP BY answer_id HAVING COUNT(DISTINCT label_id) = ");
        query_builder.push_bind(label_ids.len() as i64);
        query_builder.push(")");
    }

    if let Some(created_after) = filter.created_after() {
        query_builder
            .push(" AND answers.timestamp >= ")
            .push_bind(created_after);
    }

    if let Some(created_before) = filter.created_before() {
        query_builder
            .push(" AND answers.timestamp <= ")
            .push_bind(created_before);
    }

    if let Some(position) = request.after_position() {
        query_builder
            .push(" AND (answers.timestamp < ")
            .push_bind(position.last_timestamp())
            .push(" OR (answers.timestamp = ")
            .push_bind(position.last_timestamp())
            .push(" AND answers.id < ")
            .push_bind(position.last_answer_id().into_inner().to_string())
            .push("))");
    }

    let answers = query_builder
        .push(" ORDER BY answers.timestamp DESC, answers.id DESC LIMIT ")
        .push_bind(i64::from(request.limit().overfetch_value()))
        .build_query_as::<AnswerQueryRow>()
        .fetch_all(&mut **txn)
        .await?;

    let form_answer_records = answers
        .into_iter()
        .map(answer_record_from_row)
        .collect::<Result<Vec<_>, _>>()?;

    let answer_ids = form_answer_records
        .iter()
        .map(|record| record.id.to_owned())
        .collect_vec();
    let contents = fetch_real_answers_by_answer_ids(txn, &answer_ids).await?;
    let messages = fetch_messages_by_answer_ids(txn, &answer_ids).await?;

    let entries = attach_entry_children(form_answer_records, contents, messages)?
        .into_iter()
        .map(TryInto::<AnswerEntry>::try_into)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error: Error| InfraError::Unexpected {
            cause: error.to_string(),
        })?;

    Ok(Page::from_overfetched_items(
        entries,
        request.limit(),
        |entry| AnswerPagePosition::new(*entry.timestamp(), *entry.id()),
    ))
}

fn answer_record_from_row(row: AnswerQueryRow) -> Result<FormAnswerRecord, InfraError> {
    let answer_id = Uuid::parse_str(&row.answer_id)?;

    Ok(FormAnswerRecord {
        id: row.answer_id,
        author: author_from_values(
            row.author_type,
            row.user,
            row.user_name,
            row.user_role,
            row.temporary_user_id,
            row.temporary_user_name,
            row.temporary_user_contact_text,
            row.redmine_user_id,
            row.redmine_author_name,
        )?,
        timestamp: row.timestamp,
        form_id: row.form_id,
        form_revision_id: row.form_revision_id,
        title: row.title,
        publication: row.publication,
        status: row.status,
        contents: Vec::new(),
        messages: Vec::new(),
        redmine_reference: row.redmine_issue_id.map(|issue_id| {
            domain::form::answer::RedmineImportedAnswerReference::new(
                answer_id.into(),
                issue_id.into(),
            )
        }),
    })
}

async fn fetch_form_row(
    txn: &mut DatabaseTransaction,
    form_id: FormId,
) -> Result<Option<FormRow>, InfraError> {
    let row = sqlx::query_as!(
        FormRow,
        r"SELECT f.id, f.title, f.description, f.visibility,
            f.answer_visibility, f.answer_response_visibility,
            f.hide_author AS `hide_author: _`,
            f.allow_temporary_answers AS `allow_temporary_answers: _`,
            f.acceptance_period_start_at AS `acceptance_period_start_at: _`,
            f.acceptance_period_end_at AS `acceptance_period_end_at: _`, f.default_answer_title,
            f.created_at AS `created_at: _`, f.updated_at AS `updated_at: _`,
            w.url AS `discord_webhook_url?`
        FROM form_meta_data f
        LEFT JOIN form_discord_webhooks w ON f.id = w.form_id
        WHERE f.id = ?",
        form_id.into_inner().to_string(),
    )
    .fetch_optional(&mut **txn)
    .await?;

    Ok(row)
}

async fn fetch_archived_form_row(
    txn: &mut DatabaseTransaction,
    form_id: FormId,
) -> Result<Option<ArchivedFormRow>, InfraError> {
    let row = sqlx::query_as!(
        ArchivedFormQueryRow,
        r"SELECT f.id, f.title, f.description, f.visibility,
            f.answer_visibility, f.answer_response_visibility,
            f.hide_author AS `hide_author: _`,
            f.allow_temporary_answers AS `allow_temporary_answers: _`,
            f.acceptance_period_start_at AS `acceptance_period_start_at: _`,
            f.acceptance_period_end_at AS `acceptance_period_end_at: _`, f.default_answer_title,
            f.created_at AS `created_at: _`, f.updated_at AS `updated_at: _`,
            w.url AS `discord_webhook_url?`,
            f.archived_at AS `archived_at: _`, u.name AS archived_by_name,
            u.id AS archived_by_id, u.role AS archived_by_role
        FROM archived_form_meta_data f
        INNER JOIN users u ON f.archived_by = u.id
        LEFT JOIN archived_form_discord_webhooks w ON f.id = w.form_id
        WHERE f.id = ?",
        form_id.into_inner().to_string(),
    )
    .fetch_optional(&mut **txn)
    .await?;

    row.map(TryInto::try_into).transpose()
}

async fn insert_form_root(
    txn: &mut DatabaseTransaction,
    form: &ActiveForm,
    created_by: &AccountUser,
) -> Result<(), InfraError> {
    let form_id = form.id().into_inner().to_string();
    let title = form.title().to_string();
    let description = form.description().to_owned().into_inner();
    let visibility = form.settings().visibility().to_string();
    let user_id = created_by.id().to_string();

    let answer_settings = form.answer_settings();
    let answer_visibility = answer_settings.visibility().to_string();
    let answer_response_visibility = answer_settings.answer_response_visibility().to_string();
    let hide_author = answer_settings.author_publication_policy().hides_author();
    let allow_temporary_answers = answer_settings.allow_temporary_answers();
    let default_answer_title = answer_settings
        .default_answer_title()
        .to_owned()
        .into_inner()
        .map(NonEmptyString::into_inner);
    let acceptance_period_start_at = answer_settings.acceptance_period().start_at().to_owned();
    let acceptance_period_end_at = answer_settings.acceptance_period().end_at().to_owned();
    let discord_webhook_url = discord_webhook_url_for_persistence(form.settings());

    sqlx::query!(
        r#"INSERT INTO form_meta_data
        (id, title, description, visibility, answer_visibility, answer_response_visibility,
         hide_author, allow_temporary_answers,
         acceptance_period_start_at, acceptance_period_end_at, default_answer_title,
         created_by, updated_by)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        form_id,
        title,
        description,
        visibility,
        answer_visibility,
        answer_response_visibility,
        hide_author,
        allow_temporary_answers,
        acceptance_period_start_at,
        acceptance_period_end_at,
        default_answer_title,
        user_id,
        user_id,
    )
    .execute(&mut **txn)
    .await?;

    sqlx::query!(
        r"INSERT INTO form_discord_webhooks (form_id, url) VALUES (?, ?)",
        form_id,
        discord_webhook_url,
    )
    .execute(&mut **txn)
    .await?;

    Ok(())
}

fn discord_webhook_url_for_persistence(settings: &FormSettings) -> Option<String> {
    settings
        .discord_webhook_url(&Actor::System)
        .expect("system actor must be allowed to read the Discord webhook URL")
        .to_owned()
        .into_inner()
        .map(NonEmptyString::into_inner)
}

async fn update_form_root(
    txn: &mut DatabaseTransaction,
    form: &ActiveForm,
    updated_by: &AccountUser,
) -> Result<(), InfraError> {
    let form_id = form.id().into_inner().to_string();
    let title = form.title().to_owned().into_inner().into_inner();
    let description = form.description().to_owned().into_inner();
    let visibility = form.settings().visibility().to_string();
    let updated_by_id = updated_by.id().to_string();

    let answer_settings = form.answer_settings();
    let answer_visibility = answer_settings.visibility().to_string();
    let answer_response_visibility = answer_settings.answer_response_visibility().to_string();
    let hide_author = answer_settings.author_publication_policy().hides_author();
    let allow_temporary_answers = answer_settings.allow_temporary_answers();
    let default_answer_title = answer_settings
        .default_answer_title()
        .to_owned()
        .into_inner()
        .map(NonEmptyString::into_inner);
    let acceptance_period_start_at = answer_settings.acceptance_period().start_at().to_owned();
    let acceptance_period_end_at = answer_settings.acceptance_period().end_at().to_owned();

    let discord_webhook_url = discord_webhook_url_for_persistence(form.settings());

    sqlx::query!(
        r#"UPDATE form_meta_data SET
            title = ?,
            description = ?,
            visibility = ?,
            answer_visibility = ?,
            answer_response_visibility = ?,
            hide_author = ?,
            allow_temporary_answers = ?,
            acceptance_period_start_at = ?,
            acceptance_period_end_at = ?,
            default_answer_title = ?,
            updated_by = ?
            WHERE id = ?"#,
        title,
        description,
        visibility,
        answer_visibility,
        answer_response_visibility,
        hide_author,
        allow_temporary_answers,
        acceptance_period_start_at,
        acceptance_period_end_at,
        default_answer_title,
        updated_by_id,
        form_id,
    )
    .execute(&mut **txn)
    .await?;

    sqlx::query!(
        r#"INSERT INTO form_discord_webhooks (form_id, url) VALUES (?, ?)
        ON DUPLICATE KEY UPDATE url = VALUES(url)"#,
        form_id,
        discord_webhook_url,
    )
    .execute(&mut **txn)
    .await?;

    Ok(())
}

async fn copy_active_form_to_archive(
    txn: &mut DatabaseTransaction,
    form: &ArchivedForm,
) -> Result<(), InfraError> {
    let form_id = form.form().id().into_inner().to_string();
    let archived_at = *form.archived_at();
    let archived_by = form.archived_by().to_string();

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_form_meta_data
        (id, title, description, visibility, allow_temporary_answers, answer_visibility,
         answer_response_visibility, hide_author, acceptance_period_start_at,
         acceptance_period_end_at, default_answer_title, created_at, created_by, updated_at,
         updated_by, archived_at, archived_by)
        SELECT id, title, description, visibility, allow_temporary_answers, answer_visibility,
            answer_response_visibility, hide_author, acceptance_period_start_at,
            acceptance_period_end_at, default_answer_title, created_at, created_by, updated_at,
            updated_by, ?, ?
        FROM form_meta_data
        WHERE id = ?",
        archived_at,
        archived_by,
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_form_revisions (id, form_id, revision_number, created_at)
        SELECT id, form_id, revision_number, created_at
        FROM form_revisions WHERE form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_form_revision_questions
        (form_revision_id, question_id, template_key, position, title, description,
         question_type, is_required)
        SELECT q.form_revision_id, q.question_id, q.template_key, q.position, q.title,
            q.description, q.question_type, q.is_required
        FROM form_revision_questions q
        INNER JOIN form_revisions r ON r.id = q.form_revision_id
        WHERE r.form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_form_revision_choices
        (form_revision_id, id, question_id, position, label)
        SELECT c.form_revision_id, c.id, c.question_id, c.position, c.label
        FROM form_revision_choices c
        INNER JOIN form_revisions r ON r.id = c.form_revision_id
        WHERE r.form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_form_questions
        (question_id, form_id, template_key, position, title, description, question_type, is_required)
        SELECT question_id, form_id, template_key, position, title, description, question_type, is_required
        FROM form_questions WHERE form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_form_choices (id, question_id, position, label)
        SELECT c.id, c.question_id, c.position, c.label
        FROM form_choices c
        INNER JOIN form_questions q ON c.question_id = q.question_id
        WHERE q.form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_form_discord_webhooks (id, form_id, url)
        SELECT id, form_id, url FROM form_discord_webhooks WHERE form_id = ?",
        &form_id,
    );

    copy_form_group_restrictions_to_archive(txn, &form_id).await?;

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_answers
        (id, form_id, form_revision_id, author_type, user, temporary_user_id, redmine_user_id, redmine_author_name,
         title, publication, status, timestamp)
        SELECT id, form_id, form_revision_id, author_type, user, temporary_user_id, redmine_user_id,
            redmine_author_name, title, publication, status, timestamp FROM answers WHERE form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_real_answers (id, answer_id, form_revision_id, question_id, answer)
        SELECT r.id, r.answer_id, r.form_revision_id, r.question_id, r.answer
        FROM real_answers r
        INNER JOIN answers a ON r.answer_id = a.id
        WHERE a.form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_form_answer_comments (id, answer_id, commented_by, content, timestamp)
        SELECT c.id, c.answer_id, c.commented_by, c.content, c.timestamp
        FROM form_answer_comments c
        INNER JOIN answers a ON c.answer_id = a.id
        WHERE a.form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_form_answer_comment_attachments
            (id, answer_id, comment_id, file_name, content_type, size, created_at)
        SELECT a.id, a.answer_id, a.comment_id, a.file_name, a.content_type, a.size, a.created_at
        FROM form_answer_comment_attachments a
        INNER JOIN answers answer ON a.answer_id = answer.id
        WHERE answer.form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_messages (id, related_answer_id, sender, body, timestamp)
        SELECT m.id, m.related_answer_id, m.sender, m.body, m.timestamp
        FROM messages m
        INNER JOIN answers a ON m.related_answer_id = a.id
        WHERE a.form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_label_settings_for_forms (id, form_id, label_id)
        SELECT id, form_id, label_id FROM label_settings_for_forms WHERE form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO archived_label_settings_for_form_answers (id, answer_id, label_id)
        SELECT s.id, s.answer_id, s.label_id
        FROM label_settings_for_form_answers s
        INNER JOIN answers a ON s.answer_id = a.id
        WHERE a.form_id = ?",
        &form_id,
    );

    execute_typed_query!(txn, "DELETE FROM answers WHERE form_id = ?", &form_id);
    execute_typed_query!(txn, "DELETE FROM form_meta_data WHERE id = ?", form_id);

    Ok(())
}

async fn restore_archived_form_to_active(
    txn: &mut DatabaseTransaction,
    form_id: FormId,
) -> Result<(), InfraError> {
    let form_id = form_id.into_inner().to_string();

    execute_typed_query!(
        txn,
        r"INSERT INTO form_meta_data
        (id, title, description, visibility, allow_temporary_answers, answer_visibility,
         answer_response_visibility, hide_author, acceptance_period_start_at,
         acceptance_period_end_at, default_answer_title, created_at, created_by, updated_at,
         updated_by)
        SELECT id, title, description, visibility, allow_temporary_answers, answer_visibility,
            answer_response_visibility, hide_author, acceptance_period_start_at,
            acceptance_period_end_at, default_answer_title, created_at, created_by, updated_at,
            updated_by
        FROM archived_form_meta_data
        WHERE id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO form_revisions (id, form_id, revision_number, created_at)
        SELECT id, form_id, revision_number, created_at
        FROM archived_form_revisions WHERE form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO form_revision_questions
        (form_revision_id, question_id, template_key, position, title, description,
         question_type, is_required)
        SELECT q.form_revision_id, q.question_id, q.template_key, q.position, q.title,
            q.description, q.question_type, q.is_required
        FROM archived_form_revision_questions q
        INNER JOIN archived_form_revisions r ON r.id = q.form_revision_id
        WHERE r.form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO form_revision_choices
        (form_revision_id, id, question_id, position, label)
        SELECT c.form_revision_id, c.id, c.question_id, c.position, c.label
        FROM archived_form_revision_choices c
        INNER JOIN archived_form_revisions r ON r.id = c.form_revision_id
        WHERE r.form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO form_questions
        (question_id, form_id, template_key, position, title, description, question_type, is_required)
        SELECT question_id, form_id, template_key, position, title, description, question_type, is_required
        FROM archived_form_questions WHERE form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO form_choices (id, question_id, position, label)
        SELECT id, question_id, position, label
        FROM archived_form_choices
        WHERE question_id IN (
            SELECT question_id FROM archived_form_questions WHERE form_id = ?
        )",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO form_discord_webhooks (form_id, url)
        SELECT form_id, url FROM archived_form_discord_webhooks WHERE form_id = ?",
        &form_id,
    );

    restore_form_group_restrictions_from_archive(txn, &form_id).await?;

    execute_typed_query!(
        txn,
        r"INSERT INTO answers
        (id, form_id, form_revision_id, author_type, user, temporary_user_id, redmine_user_id, redmine_author_name,
         title, publication, status, timestamp)
        SELECT id, form_id, form_revision_id, author_type, user, temporary_user_id, redmine_user_id,
            redmine_author_name, title, publication, status, timestamp FROM archived_answers WHERE form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO real_answers (id, answer_id, form_revision_id, question_id, answer)
        SELECT id, answer_id, form_revision_id, question_id, answer
        FROM archived_real_answers
        WHERE answer_id IN (SELECT id FROM archived_answers WHERE form_id = ?)",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO form_answer_comments (id, answer_id, commented_by, content, timestamp)
        SELECT id, answer_id, commented_by, content, timestamp
        FROM archived_form_answer_comments
        WHERE answer_id IN (SELECT id FROM archived_answers WHERE form_id = ?)",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO form_answer_comment_attachments
            (id, answer_id, comment_id, file_name, content_type, size, created_at)
        SELECT id, answer_id, comment_id, file_name, content_type, size, created_at
        FROM archived_form_answer_comment_attachments
        WHERE answer_id IN (SELECT id FROM archived_answers WHERE form_id = ?)",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO messages (id, related_answer_id, sender, body, timestamp)
        SELECT id, related_answer_id, sender, body, timestamp
        FROM archived_messages
        WHERE related_answer_id IN (SELECT id FROM archived_answers WHERE form_id = ?)",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO label_settings_for_forms (form_id, label_id)
        SELECT form_id, label_id FROM archived_label_settings_for_forms WHERE form_id = ?",
        &form_id,
    );

    execute_typed_query!(
        txn,
        r"INSERT INTO label_settings_for_form_answers (answer_id, label_id)
        SELECT answer_id, label_id
        FROM archived_label_settings_for_form_answers
        WHERE answer_id IN (SELECT id FROM archived_answers WHERE form_id = ?)",
        &form_id,
    );

    execute_typed_query!(
        txn,
        "DELETE FROM archived_answers WHERE form_id = ?",
        &form_id
    );

    execute_typed_query!(
        txn,
        "DELETE FROM archived_form_meta_data WHERE id = ?",
        form_id,
    );

    Ok(())
}

async fn copy_form_group_restrictions_to_archive(
    txn: &mut DatabaseTransaction,
    form_id: &str,
) -> Result<(), InfraError> {
    copy_group_restriction_table(
        txn,
        "archived_form_allowed_user_groups",
        "form_allowed_user_groups",
        form_id,
    )
    .await?;
    copy_group_restriction_table(
        txn,
        "archived_form_answer_groups",
        "form_answer_groups",
        form_id,
    )
    .await
}

async fn restore_form_group_restrictions_from_archive(
    txn: &mut DatabaseTransaction,
    form_id: &str,
) -> Result<(), InfraError> {
    copy_group_restriction_table(
        txn,
        "form_allowed_user_groups",
        "archived_form_allowed_user_groups",
        form_id,
    )
    .await?;
    copy_group_restriction_table(
        txn,
        "form_answer_groups",
        "archived_form_answer_groups",
        form_id,
    )
    .await
}

async fn copy_group_restriction_table(
    txn: &mut DatabaseTransaction,
    destination_table: &str,
    source_table: &str,
    form_id: &str,
) -> Result<(), InfraError> {
    let sql = format!(
        "INSERT INTO {destination_table} (form_id, group_id)
        SELECT form_id, group_id FROM {source_table} WHERE form_id = ?"
    );
    query(AssertSqlSafe(&*sql))
        .bind(form_id)
        .execute(&mut **txn)
        .await?;

    Ok(())
}

#[async_trait]
impl FormDatabase for ConnectionPool {
    #[tracing::instrument(skip_all)]
    async fn create(&self, form: &ActiveForm, user: &AccountUser) -> Result<(), InfraError> {
        let form = form.clone();
        let user = user.clone();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                insert_form_root(txn, &form, &user).await?;
                sync_questions(txn, &form).await?;
                persist_form_revision(txn, &form).await?;
                sync_label_ids(txn, &form).await?;
                sync_form_group_restrictions(txn, &form).await?;
                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument(skip_all)]
    async fn list(
        &self,
        request: PageRequest<FormPagePosition>,
    ) -> Result<Page<ActiveFormRecord, FormPagePosition>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rows = if let Some(position) = request.after_position() {
                    sqlx::query_as!(
                        FormRow,
                        r"SELECT f.id, f.title, f.description, f.visibility,
                    f.answer_visibility, f.answer_response_visibility,
                    f.hide_author AS `hide_author: _`,
                    f.allow_temporary_answers AS `allow_temporary_answers: _`,
                    f.acceptance_period_start_at AS `acceptance_period_start_at: _`,
                    f.acceptance_period_end_at AS `acceptance_period_end_at: _`,
                    f.default_answer_title, f.created_at AS `created_at: _`,
                    f.updated_at AS `updated_at: _`, w.url AS `discord_webhook_url?`
                    FROM form_meta_data f
                    LEFT JOIN form_discord_webhooks w ON f.id = w.form_id
                    WHERE f.id > ?
                    ORDER BY f.id
                    LIMIT ?",
                        position.last_form_id().into_inner().to_string(),
                        i64::from(request.limit().overfetch_value()),
                    )
                    .fetch_all(&mut **txn)
                    .await?
                } else {
                    sqlx::query_as!(
                        FormRow,
                        r"SELECT f.id, f.title, f.description, f.visibility,
                    f.answer_visibility, f.answer_response_visibility,
                    f.hide_author AS `hide_author: _`,
                    f.allow_temporary_answers AS `allow_temporary_answers: _`,
                    f.acceptance_period_start_at AS `acceptance_period_start_at: _`,
                    f.acceptance_period_end_at AS `acceptance_period_end_at: _`,
                    f.default_answer_title, f.created_at AS `created_at: _`,
                    f.updated_at AS `updated_at: _`, w.url AS `discord_webhook_url?`
                    FROM form_meta_data f
                    LEFT JOIN form_discord_webhooks w ON f.id = w.form_id
                    ORDER BY f.id
                    LIMIT ?",
                        i64::from(request.limit().overfetch_value()),
                    )
                    .fetch_all(&mut **txn)
                    .await?
                };

                let form_rows = rows;

                let form_ids = form_rows.iter().map(|r| r.id.clone()).collect_vec();
                let group_restrictions =
                    fetch_form_group_restrictions_batch(txn, "form_", &form_ids).await?;
                let labels_by_form =
                    fetch_label_ids_batch(txn, "label_settings_for_forms", &form_ids).await?;

                let records =
                    build_active_form_records(txn, form_rows, group_restrictions, labels_by_form)
                        .await?;

                Ok::<_, InfraError>(Page::from_overfetched_items(
                    records,
                    request.limit(),
                    |record: &ActiveFormRecord| {
                        FormPagePosition::new(FormId::from(
                            Uuid::parse_str(&record.id).expect("database form id must be uuid"),
                        ))
                    },
                ))
            })
        })
        .await
    }

    #[tracing::instrument(skip_all)]
    async fn list_all(&self) -> Result<Vec<ActiveFormRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let form_rows = sqlx::query_as!(
                    FormRow,
                    r"SELECT f.id, f.title, f.description, f.visibility,
                    f.answer_visibility, f.answer_response_visibility,
                    f.hide_author AS `hide_author: _`,
                    f.allow_temporary_answers AS `allow_temporary_answers: _`,
                    f.acceptance_period_start_at AS `acceptance_period_start_at: _`,
                    f.acceptance_period_end_at AS `acceptance_period_end_at: _`,
                    f.default_answer_title, f.created_at AS `created_at: _`,
                    f.updated_at AS `updated_at: _`, w.url AS `discord_webhook_url?`
                    FROM form_meta_data f
                    LEFT JOIN form_discord_webhooks w ON f.id = w.form_id
                    ORDER BY f.id",
                )
                .fetch_all(&mut **txn)
                .await?;

                let form_ids = form_rows.iter().map(|r| r.id.clone()).collect_vec();
                let group_restrictions =
                    fetch_form_group_restrictions_batch(txn, "form_", &form_ids).await?;
                let labels_by_form =
                    fetch_label_ids_batch(txn, "label_settings_for_forms", &form_ids).await?;

                build_active_form_records(txn, form_rows, group_restrictions, labels_by_form).await
            })
        })
        .await
    }

    #[tracing::instrument(skip_all, fields(form_id = %form_id))]
    async fn get(&self, form_id: FormId) -> Result<Option<ActiveFormRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                match fetch_form_row(txn, form_id).await? {
                    Some(row) => active_form_record_from_row(txn, row).await.map(Some),
                    None => Ok(None),
                }
            })
        })
        .await
    }

    #[tracing::instrument(skip_all)]
    async fn list_archived(
        &self,
        request: PageRequest<ArchivedFormPagePosition>,
        query_text: Option<String>,
    ) -> Result<Page<ArchivedFormRecord, ArchivedFormPagePosition>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rows = if let Some(query_text) = query_text {
                    let like = format!("%{query_text}%");
                    if let Some(position) = request.after_position() {
                        sqlx::query_as!(
                            ArchivedFormQueryRow,
                            r"SELECT f.id, f.title, f.description, f.visibility,
                        f.answer_visibility, f.answer_response_visibility,
                        f.hide_author AS `hide_author: _`,
                        f.allow_temporary_answers AS `allow_temporary_answers: _`,
                        f.acceptance_period_start_at AS `acceptance_period_start_at: _`,
                        f.acceptance_period_end_at AS `acceptance_period_end_at: _`,
                        f.default_answer_title, f.created_at AS `created_at: _`,
                        f.updated_at AS `updated_at: _`, w.url AS `discord_webhook_url?`,
                        f.archived_at AS `archived_at: _`, u.name AS archived_by_name,
                        u.id AS archived_by_id, u.role AS archived_by_role
                        FROM archived_form_meta_data f
                        INNER JOIN users u ON f.archived_by = u.id
                        LEFT JOIN archived_form_discord_webhooks w ON f.id = w.form_id
                        WHERE (f.archived_at < ? OR (f.archived_at = ? AND f.id > ?))
                            AND (f.title LIKE ? OR f.description LIKE ?)
                        ORDER BY f.archived_at DESC, f.id ASC
                        LIMIT ?",
                            position.last_archived_at(),
                            position.last_archived_at(),
                            position.last_form_id().into_inner().to_string(),
                            &like,
                            &like,
                            i64::from(request.limit().overfetch_value()),
                        )
                        .fetch_all(&mut **txn)
                        .await?
                    } else {
                        sqlx::query_as!(
                            ArchivedFormQueryRow,
                            r"SELECT f.id, f.title, f.description, f.visibility,
                        f.answer_visibility, f.answer_response_visibility,
                        f.hide_author AS `hide_author: _`,
                        f.allow_temporary_answers AS `allow_temporary_answers: _`,
                        f.acceptance_period_start_at AS `acceptance_period_start_at: _`,
                        f.acceptance_period_end_at AS `acceptance_period_end_at: _`,
                        f.default_answer_title, f.created_at AS `created_at: _`,
                        f.updated_at AS `updated_at: _`, w.url AS `discord_webhook_url?`,
                        f.archived_at AS `archived_at: _`, u.name AS archived_by_name,
                        u.id AS archived_by_id, u.role AS archived_by_role
                        FROM archived_form_meta_data f
                        INNER JOIN users u ON f.archived_by = u.id
                        LEFT JOIN archived_form_discord_webhooks w ON f.id = w.form_id
                        WHERE f.title LIKE ? OR f.description LIKE ?
                        ORDER BY f.archived_at DESC, f.id ASC
                        LIMIT ?",
                            &like,
                            &like,
                            i64::from(request.limit().overfetch_value()),
                        )
                        .fetch_all(&mut **txn)
                        .await?
                    }
                } else if let Some(position) = request.after_position() {
                    sqlx::query_as!(
                        ArchivedFormQueryRow,
                        r"SELECT f.id, f.title, f.description, f.visibility,
                        f.answer_visibility, f.answer_response_visibility,
                        f.hide_author AS `hide_author: _`,
                        f.allow_temporary_answers AS `allow_temporary_answers: _`,
                        f.acceptance_period_start_at AS `acceptance_period_start_at: _`,
                        f.acceptance_period_end_at AS `acceptance_period_end_at: _`,
                        f.default_answer_title, f.created_at AS `created_at: _`,
                        f.updated_at AS `updated_at: _`, w.url AS `discord_webhook_url?`,
                        f.archived_at AS `archived_at: _`, u.name AS archived_by_name,
                        u.id AS archived_by_id, u.role AS archived_by_role
                        FROM archived_form_meta_data f
                        INNER JOIN users u ON f.archived_by = u.id
                        LEFT JOIN archived_form_discord_webhooks w ON f.id = w.form_id
                        WHERE f.archived_at < ? OR (f.archived_at = ? AND f.id > ?)
                        ORDER BY f.archived_at DESC, f.id ASC
                        LIMIT ?",
                        position.last_archived_at(),
                        position.last_archived_at(),
                        position.last_form_id().into_inner().to_string(),
                        i64::from(request.limit().overfetch_value()),
                    )
                    .fetch_all(&mut **txn)
                    .await?
                } else {
                    sqlx::query_as!(
                        ArchivedFormQueryRow,
                        r"SELECT f.id, f.title, f.description, f.visibility,
                        f.answer_visibility, f.answer_response_visibility,
                        f.hide_author AS `hide_author: _`,
                        f.allow_temporary_answers AS `allow_temporary_answers: _`,
                        f.acceptance_period_start_at AS `acceptance_period_start_at: _`,
                        f.acceptance_period_end_at AS `acceptance_period_end_at: _`,
                        f.default_answer_title, f.created_at AS `created_at: _`,
                        f.updated_at AS `updated_at: _`, w.url AS `discord_webhook_url?`,
                        f.archived_at AS `archived_at: _`, u.name AS archived_by_name,
                        u.id AS archived_by_id, u.role AS archived_by_role
                        FROM archived_form_meta_data f
                        INNER JOIN users u ON f.archived_by = u.id
                        LEFT JOIN archived_form_discord_webhooks w ON f.id = w.form_id
                        ORDER BY f.archived_at DESC, f.id ASC
                        LIMIT ?",
                        i64::from(request.limit().overfetch_value()),
                    )
                    .fetch_all(&mut **txn)
                    .await?
                };

                let form_rows = rows
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<ArchivedFormRow>, InfraError>>()?;

                let form_ids = form_rows.iter().map(|r| r.form.id.clone()).collect_vec();
                let group_restrictions =
                    fetch_form_group_restrictions_batch(txn, "archived_form_", &form_ids).await?;
                let labels_by_form =
                    fetch_label_ids_batch(txn, "archived_label_settings_for_forms", &form_ids)
                        .await?;

                let records =
                    build_archived_form_records(txn, form_rows, group_restrictions, labels_by_form)
                        .await?;

                Ok::<_, InfraError>(Page::from_overfetched_items(
                    records,
                    request.limit(),
                    |record: &ArchivedFormRecord| {
                        ArchivedFormPagePosition::new(
                            record.archived_at,
                            FormId::from(
                                Uuid::parse_str(&record.form.id)
                                    .expect("database archived form id must be uuid"),
                            ),
                        )
                    },
                ))
            })
        })
        .await
    }

    #[tracing::instrument(skip_all, fields(form_id = %form_id))]
    async fn get_archived(
        &self,
        form_id: FormId,
    ) -> Result<Option<ArchivedFormRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                match fetch_archived_form_row(txn, form_id).await? {
                    Some(row) => archived_form_record_from_row(txn, row).await.map(Some),
                    None => Ok(None),
                }
            })
        })
        .await
    }

    #[tracing::instrument(skip_all, fields(form_id = %form_id, answer_id = %answer_id))]
    async fn archived_answer_publication(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Option<AnswerPublication>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let publication = sqlx::query_scalar!(
                    "SELECT publication FROM archived_answers WHERE form_id = ? AND id = ?",
                    form_id.to_string(),
                    answer_id.to_string(),
                )
                .fetch_optional(&mut **txn)
                .await?;

                publication
                    .map(AnswerPublication::try_from)
                    .transpose()
                    .map_err(|error| InfraError::Unexpected {
                        cause: error.to_string(),
                    })
            })
        })
        .await
    }

    #[tracing::instrument(skip_all)]
    async fn archive(&self, form: &ArchivedForm) -> Result<ArchivedForm, InfraError> {
        let form = form.clone();
        self.read_write_transaction(move |txn| {
            Box::pin(async move {
                let form_id = *form.form().id();
                // コメント書込みが form_meta_data を最初にロックするため、アーカイブも同じ
                // 行を先にロックしてから回答・コメントをコピー／削除する。
                if sqlx::query!(
                    "SELECT id FROM form_meta_data WHERE id = ? FOR UPDATE",
                    form_id.to_string(),
                )
                .fetch_optional(&mut **txn)
                .await?
                .is_none()
                {
                    return Err(InfraError::FormNotFound {
                        id: form_id.into_inner(),
                    });
                }

                copy_active_form_to_archive(txn, &form).await?;

                let row = fetch_archived_form_row(txn, form_id)
                    .await?
                    .ok_or_else(|| InfraError::FormNotFound {
                        id: form_id.into_inner(),
                    })?;

                archived_form_record_from_row(txn, row)
                    .await?
                    .try_into()
                    .map_err(|error: Error| InfraError::Unexpected {
                        cause: error.to_string(),
                    })
            })
        })
        .await
    }

    #[tracing::instrument(skip_all, fields(form_id = %form_id))]
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

    #[tracing::instrument(skip_all)]
    async fn update(&self, form: &ActiveForm, updated_by: &AccountUser) -> Result<(), InfraError> {
        let form = form.clone();
        let updated_by = updated_by.clone();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                ensure_expected_form_revision(txn, &form).await?;
                update_form_root(txn, &form, &updated_by).await?;
                sync_questions(txn, &form).await?;
                persist_form_revision(txn, &form).await?;
                sync_label_ids(txn, &form).await?;
                sync_form_group_restrictions(txn, &form).await?;
                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument(skip_all)]
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

    #[tracing::instrument(skip_all, fields(form_id = %form_id))]
    async fn list_answer_entries(
        &self,
        form_id: FormId,
        request: PageRequest<AnswerPagePosition>,
        filter: AnswerListFilter,
    ) -> Result<Page<AnswerEntry, AnswerPagePosition>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(
                async move { fetch_answer_entries_page(txn, Some(form_id), request, filter).await },
            )
        })
        .await
    }

    #[tracing::instrument(skip_all)]
    async fn list_all_answer_entries(
        &self,
        request: PageRequest<AnswerPagePosition>,
        filter: AnswerListFilter,
    ) -> Result<Page<AnswerEntry, AnswerPagePosition>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move { fetch_answer_entries_page(txn, None, request, filter).await })
        })
        .await
    }
}

async fn ensure_expected_form_revision(
    txn: &mut DatabaseTransaction,
    form: &ActiveForm,
) -> Result<(), InfraError> {
    let form_id = form.id().to_string();
    let expected_revision_id = form
        .expected_revision_id()
        .as_ref()
        .ok_or_else(|| InfraError::Unexpected {
            cause: format!("persisted form {form_id} is missing its expected revision"),
        })?
        .to_string();

    sqlx::query_scalar!(
        "SELECT id FROM form_meta_data WHERE id = ? FOR UPDATE",
        form_id,
    )
    .fetch_optional(&mut **txn)
    .await?
    .ok_or_else(|| InfraError::FormNotFound {
        id: form.id().into_inner(),
    })?;

    let current_revision_id = sqlx::query_scalar!(
        r"SELECT id FROM form_revisions
        WHERE form_id = ?
        ORDER BY revision_number DESC
        LIMIT 1",
        form_id,
    )
    .fetch_optional(&mut **txn)
    .await?;

    if current_revision_id.as_deref() != Some(expected_revision_id.as_str()) {
        return Err(InfraError::Unexpected {
            cause: format!(
                "form {form_id} was updated from revision {expected_revision_id} before this update"
            ),
        });
    }

    Ok(())
}

async fn sync_label_ids(
    txn: &mut DatabaseTransaction,
    form: &ActiveForm,
) -> Result<(), InfraError> {
    let form_id = form.id().into_inner().to_string();

    sqlx::query!(
        "DELETE FROM label_settings_for_forms WHERE form_id = ?",
        form_id.clone(),
    )
    .execute(&mut **txn)
    .await?;

    if form.label_ids().as_slice().is_empty() {
        return Ok(());
    }

    let label_ids = form
        .label_ids()
        .as_slice()
        .iter()
        .map(|label_id| label_id.into_inner().to_string())
        .collect_vec();
    let sql = format!(
        "INSERT INTO label_settings_for_forms (form_id, label_id) VALUES {}",
        std::iter::repeat_n("(?, ?)", label_ids.len()).join(", ")
    );
    label_ids
        .into_iter()
        .flat_map(|label_id| [form_id.clone(), label_id])
        .fold(query(AssertSqlSafe(&*sql)), |query, value| {
            query.bind(value)
        })
        .execute(&mut **txn)
        .await?;

    Ok(())
}

async fn sync_form_group_restrictions(
    txn: &mut DatabaseTransaction,
    form: &ActiveForm,
) -> Result<(), InfraError> {
    sync_group_restriction_ids(
        txn,
        "form_allowed_user_groups",
        *form.id(),
        form.settings().allowed_user_groups().as_slice(),
    )
    .await?;
    sync_group_restriction_ids(
        txn,
        "form_answer_groups",
        *form.id(),
        form.answer_settings().answer_group_ids(),
    )
    .await
}

async fn sync_group_restriction_ids(
    txn: &mut DatabaseTransaction,
    table_name: &str,
    form_id: FormId,
    group_ids: &[UserGroupId],
) -> Result<(), InfraError> {
    let form_id = form_id.into_inner().to_string();
    let delete_sql = format!("DELETE FROM {table_name} WHERE form_id = ?");
    query(AssertSqlSafe(&*delete_sql))
        .bind(form_id.clone())
        .execute(&mut **txn)
        .await?;

    if group_ids.is_empty() {
        return Ok(());
    }

    let sql = format!(
        "INSERT INTO {table_name} (form_id, group_id) VALUES {}",
        std::iter::repeat_n("(?, ?)", group_ids.len()).join(", ")
    );
    group_ids
        .iter()
        .flat_map(|group_id| [form_id.clone(), group_id.to_string()])
        .fold(query(AssertSqlSafe(&*sql)), |query, value| {
            query.bind(value)
        })
        .execute(&mut **txn)
        .await?;

    Ok(())
}

async fn sync_questions(
    txn: &mut DatabaseTransaction,
    form: &ActiveForm,
) -> Result<(), InfraError> {
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

async fn persist_form_revision(
    txn: &mut DatabaseTransaction,
    form: &ActiveForm,
) -> Result<(), InfraError> {
    let revision_id = form.revision().id().to_string();
    let form_id = form.id().to_string();
    let already_persisted = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1 FROM form_revisions WHERE id = ?
        ) AS `exists!: bool`"#,
        revision_id,
    )
    .fetch_one(&mut **txn)
    .await?;

    if already_persisted {
        return Ok(());
    }

    sqlx::query!(
        r"INSERT INTO form_revisions (id, form_id, revision_number)
        SELECT ?, ?, COALESCE(MAX(revision_number), 0) + 1
        FROM form_revisions
        WHERE form_id = ?",
        revision_id,
        form_id,
        form_id,
    )
    .execute(&mut **txn)
    .await?;

    sqlx::query!(
        r"INSERT INTO form_revision_questions
        (form_revision_id, question_id, template_key, position, title, description,
         question_type, is_required)
        SELECT ?, question_id, template_key, position, title, description, question_type,
            is_required
        FROM form_questions
        WHERE form_id = ?",
        revision_id,
        form_id,
    )
    .execute(&mut **txn)
    .await?;

    sqlx::query!(
        r"INSERT INTO form_revision_choices (form_revision_id, id, question_id, position, label)
        SELECT ?, c.id, c.question_id, c.position, c.label
        FROM form_choices c
        INNER JOIN form_questions q ON q.question_id = c.question_id
        WHERE q.form_id = ?",
        revision_id,
        form_id,
    )
    .execute(&mut **txn)
    .await?;

    Ok(())
}

async fn fetch_question_ids(
    txn: &mut MySqlConnection,
    form_id: &str,
) -> Result<Vec<Uuid>, InfraError> {
    let question_ids = sqlx::query_scalar!(
        "SELECT question_id FROM form_questions WHERE form_id = ? ORDER BY position ASC",
        form_id,
    )
    .fetch_all(&mut *txn)
    .await?;

    question_ids
        .iter()
        .map(|question_id| Ok::<_, InfraError>(Uuid::parse_str(question_id)?))
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
    let rows = sqlx::query!(
        "SELECT question_id, template_key, position FROM form_questions WHERE form_id = ? ORDER BY position ASC, question_id ASC",
        form_id,
    )
    .fetch_all(&mut *txn)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok::<_, InfraError>(ExistingQuestionRow {
                question_id: Uuid::parse_str(&row.question_id)?.into(),
                template_key: row.template_key,
                position: row.position,
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

    sqlx::query!(
        "UPDATE form_questions
         SET template_key = CONCAT(?, question_id),
             position = position + ?
         WHERE form_id = ?",
        temporary_prefix,
        position_offset,
        form_id,
    )
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
        .fold(query(AssertSqlSafe(&*sql)), |query, question| {
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
        .fold(query(AssertSqlSafe(&*sql)), |query, question_id| {
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

    let desired_choices: Vec<(QuestionId, &Choice)> = assigned_questions
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
        .fold(sqlx::query(AssertSqlSafe(&*sql)), |query, question_id| {
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
        .fold(query(AssertSqlSafe(&*sql)), |query, choice_id| {
            query.bind(choice_id)
        })
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
            query(AssertSqlSafe(&*sql)),
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
        .fold(
            query(AssertSqlSafe(&*sql)),
            |query, (question_id, position, label)| {
                query
                    .bind(question_id.into_inner().to_string())
                    .bind(position)
                    .bind(label)
            },
        )
        .execute(&mut *txn)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::form::models::DiscordWebhookUrl;

    #[test]
    fn persistence_extracts_the_webhook_url_as_system() {
        let url = "https://discord.com/api/webhooks/123/token";
        let settings_with_webhook = FormSettings::new().change_discord_webhook_url(
            DiscordWebhookUrl::try_new(Some(NonEmptyString::try_new(url.to_string()).unwrap()))
                .unwrap(),
        );

        assert_eq!(
            discord_webhook_url_for_persistence(&settings_with_webhook).as_deref(),
            Some(url)
        );
        assert_eq!(
            discord_webhook_url_for_persistence(&FormSettings::new()),
            None
        );
    }
}
