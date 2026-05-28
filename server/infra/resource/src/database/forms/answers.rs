use std::str::FromStr;

use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerAuthor, AnswerEntry, AnswerId},
        models::FormId,
    },
    user::models::{ActiveUser, Role, TemporaryUser},
};
use errors::infra::InfraError;
use itertools::Itertools;
use sqlx::{Row, mysql::MySqlRow, query};
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

use crate::{
    database::{
        components::FormAnswerDatabase,
        connection::{ConnectionPool, DatabaseTransaction},
        count::count_as_u32,
    },
    records::{
        AnswerAuthorRecord, CommentRecord, FormAnswerContentRecord, FormAnswerRecord, MessageRecord,
    },
};

fn answer_author_columns(answer: &AnswerEntry) -> (String, Option<String>, Option<String>) {
    match answer.author() {
        AnswerAuthor::AuthenticatedUser(user_id) => (
            "AUTHENTICATED_USER".to_string(),
            Some(user_id.to_string()),
            None,
        ),
        AnswerAuthor::TemporaryUser(temporary_user) => (
            "TEMPORARY_USER".to_string(),
            None,
            Some(temporary_user.id().to_string()),
        ),
    }
}

pub(crate) fn author_from_row(row: &MySqlRow) -> Result<AnswerAuthorRecord, InfraError> {
    let author_type: String = row.try_get("author_type")?;
    match author_type.as_str() {
        "AUTHENTICATED_USER" => Ok(AnswerAuthorRecord::AuthenticatedUser(ActiveUser::new(
            row.try_get("user_name")?,
            Uuid::from_str(&row.try_get::<String, _>("user")?)?.into(),
            Role::from_str(&row.try_get::<String, _>("user_role")?)?,
        ))),
        "TEMPORARY_USER" => Ok(AnswerAuthorRecord::TemporaryUser(
            TemporaryUser::from_raw_parts(
                Uuid::from_str(&row.try_get::<String, _>("temporary_user_id")?)?.into(),
                row.try_get("temporary_user_name")?,
                row.try_get("temporary_user_contact_text")?,
            ),
        )),
        value => Err(InfraError::Unexpected {
            cause: format!("unknown answer author_type: {value}"),
        }),
    }
}

pub(crate) async fn fetch_real_answers_by_answer_ids<T>(
    txn: &mut DatabaseTransaction,
    answer_ids: &[T],
) -> Result<Vec<(Uuid, FormAnswerContentRecord)>, InfraError>
where
    T: AsRef<str>,
{
    if answer_ids.is_empty() {
        return Ok(Vec::new());
    }

    let sql = format!(
        "SELECT id, question_id, answer, answer_id FROM real_answers WHERE answer_id IN ({})",
        std::iter::repeat_n("?", answer_ids.len()).join(", ")
    );

    answer_ids
        .iter()
        .fold(query(&sql), |query, answer_id| {
            query.bind(answer_id.as_ref())
        })
        .fetch_all(&mut **txn)
        .await?
        .into_iter()
        .map(|row| {
            Ok::<_, InfraError>((
                Uuid::from_str(&row.try_get::<String, _>("answer_id")?)?,
                FormAnswerContentRecord {
                    id: row.try_get("id")?,
                    question_id: row.try_get("question_id")?,
                    answer: row.try_get("answer")?,
                },
            ))
        })
        .collect()
}

pub(crate) fn attach_contents(
    form_answer_records: Vec<FormAnswerRecord>,
    answer_id_with_content_record: Vec<(Uuid, FormAnswerContentRecord)>,
) -> Result<Vec<FormAnswerRecord>, InfraError> {
    let grouped_answer_contents = answer_id_with_content_record
        .into_iter()
        .into_group_map_by(|(answer_id, _)| *answer_id);

    form_answer_records
        .into_iter()
        .map(|record| {
            Ok::<_, InfraError>(FormAnswerRecord {
                contents: grouped_answer_contents
                    .get(&Uuid::from_str(&record.id)?)
                    .cloned()
                    .map(|contents| {
                        contents
                            .into_iter()
                            .map(|(_, content_record)| content_record)
                            .collect_vec()
                    })
                    .unwrap_or_default(),
                ..record
            })
        })
        .collect()
}

pub(crate) async fn fetch_comments_by_answer_ids<T>(
    txn: &mut DatabaseTransaction,
    answer_ids: &[T],
) -> Result<Vec<(Uuid, CommentRecord)>, InfraError>
where
    T: AsRef<str>,
{
    if answer_ids.is_empty() {
        return Ok(Vec::new());
    }

    let sql = format!(
        r"SELECT form_answer_comments.id AS comment_id, answer_id,
            commented_by AS commented_by_id, users.name AS commented_by_name,
            users.role AS commented_by_role, content,
            timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
        FROM form_answer_comments
        INNER JOIN users ON form_answer_comments.commented_by = users.id
        WHERE answer_id IN ({})",
        std::iter::repeat_n("?", answer_ids.len()).join(", ")
    );

    answer_ids
        .iter()
        .fold(query(&sql), |query, answer_id| {
            query.bind(answer_id.as_ref())
        })
        .fetch_all(&mut **txn)
        .await?
        .into_iter()
        .map(|row| {
            Ok::<_, InfraError>((
                Uuid::from_str(&row.try_get::<String, _>("answer_id")?)?,
                CommentRecord {
                    answer_id: row.try_get("answer_id")?,
                    comment_id: row.try_get("comment_id")?,
                    content: row.try_get("content")?,
                    timestamp: row.try_get("timestamp!: chrono::DateTime<chrono::Utc>")?,
                    commented_by_id: row.try_get("commented_by_id")?,
                    commented_by_name: row.try_get("commented_by_name")?,
                    commented_by_role: row.try_get("commented_by_role")?,
                },
            ))
        })
        .collect()
}

pub(crate) async fn fetch_messages_by_answer_ids<T>(
    txn: &mut DatabaseTransaction,
    answer_ids: &[T],
) -> Result<Vec<(Uuid, MessageRecord)>, InfraError>
where
    T: AsRef<str>,
{
    if answer_ids.is_empty() {
        return Ok(Vec::new());
    }

    let sql = format!(
        r"SELECT messages.id AS id, related_answer_id AS related_answer,
            sender AS sender_id, users.name AS sender_name,
            users.role AS sender_role, body,
            timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
        FROM messages
        INNER JOIN users ON users.id = messages.sender
        WHERE related_answer_id IN ({})",
        std::iter::repeat_n("?", answer_ids.len()).join(", ")
    );

    answer_ids
        .iter()
        .fold(query(&sql), |query, answer_id| {
            query.bind(answer_id.as_ref())
        })
        .fetch_all(&mut **txn)
        .await?
        .into_iter()
        .map(|row| {
            Ok::<_, InfraError>((
                Uuid::from_str(&row.try_get::<String, _>("related_answer")?)?,
                MessageRecord {
                    id: row.try_get("id")?,
                    sender_id: row.try_get("sender_id")?,
                    sender_name: row.try_get("sender_name")?,
                    sender_role: row.try_get("sender_role")?,
                    body: row.try_get("body")?,
                    timestamp: row.try_get("timestamp!: chrono::DateTime<chrono::Utc>")?,
                },
            ))
        })
        .collect()
}

pub(crate) fn attach_entry_children(
    form_answer_records: Vec<FormAnswerRecord>,
    content_records: Vec<(Uuid, FormAnswerContentRecord)>,
    comment_records: Vec<(Uuid, CommentRecord)>,
    message_records: Vec<(Uuid, MessageRecord)>,
) -> Result<Vec<FormAnswerRecord>, InfraError> {
    let grouped_contents = content_records
        .into_iter()
        .into_group_map_by(|(answer_id, _)| *answer_id);
    let grouped_comments = comment_records
        .into_iter()
        .into_group_map_by(|(answer_id, _)| *answer_id);
    let grouped_messages = message_records
        .into_iter()
        .into_group_map_by(|(answer_id, _)| *answer_id);

    form_answer_records
        .into_iter()
        .map(|record| {
            let answer_uuid = Uuid::from_str(&record.id)?;
            Ok::<_, InfraError>(FormAnswerRecord {
                contents: grouped_contents
                    .get(&answer_uuid)
                    .cloned()
                    .map(|v| v.into_iter().map(|(_, r)| r).collect_vec())
                    .unwrap_or_default(),
                comments: grouped_comments
                    .get(&answer_uuid)
                    .cloned()
                    .map(|v| v.into_iter().map(|(_, r)| r).collect_vec())
                    .unwrap_or_default(),
                messages: grouped_messages
                    .get(&answer_uuid)
                    .cloned()
                    .map(|v| v.into_iter().map(|(_, r)| r).collect_vec())
                    .unwrap_or_default(),
                ..record
            })
        })
        .collect()
}

#[async_trait]
impl FormAnswerDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn post_answer(&self, answer: &AnswerEntry, form_id: FormId) -> Result<(), InfraError> {
        let answer_id = answer.id().to_owned().into_inner().to_string();
        let form_id = form_id.into_inner().to_string();
        let (author_type, user_id, temporary_user_id) = answer_author_columns(answer);
        let temporary_user = answer.author().temporary_user().cloned();
        let title = <Option<NonEmptyString> as Clone>::clone(&answer.title().to_owned())
            .map(|title| title.into_inner());
        let timestamp = answer.timestamp().to_owned();
        let contents = answer
            .contents()
            .as_slice()
            .iter()
            .map(|content| {
                (
                    content.id.to_owned().into_inner().to_string(),
                    answer_id.clone(),
                    content.question_id.to_owned().into_inner().to_string(),
                    content.answer.to_owned(),
                )
            })
            .collect::<Vec<_>>();

        self.read_write_transaction(move |txn| {
            Box::pin(async move {
                if let Some(temporary_user) = temporary_user {
                    sqlx::query!(
                        r"INSERT INTO temporary_users (id, name, contact_text)
                        VALUES (?, ?, ?)
                        ON DUPLICATE KEY UPDATE name = VALUES(name), contact_text = VALUES(contact_text)",
                        temporary_user.id().to_string(),
                        temporary_user.name(),
                        temporary_user.contact_text(),
                    )
                    .execute(&mut **txn)
                    .await?;
                }

                sqlx::query!(
                    r"INSERT INTO answers (id, form_id, author_type, user, temporary_user_id, title, timestamp)
                    VALUES (?, ?, ?, ?, ?, ?, ?)",
                    answer_id,
                    form_id,
                    author_type,
                    user_id,
                    temporary_user_id,
                    title,
                    timestamp,
                )
                .execute(&mut **txn)
                .await?;

                if !contents.is_empty() {
                    let sql = format!(
                        "INSERT INTO real_answers (id, answer_id, question_id, answer) VALUES {}",
                        std::iter::repeat_n("(?, ?, ?, ?)", contents.len()).join(", ")
                    );
                    contents
                        .into_iter()
                        .flat_map(|(id, answer_id, question_id, answer)| {
                            [id, answer_id, question_id, answer]
                        })
                        .fold(query(&sql), |query, value| query.bind(value))
                        .execute(&mut **txn)
                        .await?;
                }

                Ok::<_, InfraError>(())
            })
        }).await
    }

    #[tracing::instrument]
    async fn get_answers(
        &self,
        answer_id: AnswerId,
    ) -> Result<Option<FormAnswerRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answer_query_result_opt = sqlx::query(
                    r"SELECT form_id, answers.id AS answer_id, title, author_type, user,
                        users.name AS user_name, users.role AS user_role,
                        temporary_user_id, temporary_users.name AS temporary_user_name,
                        temporary_users.contact_text AS temporary_user_contact_text,
                        timestamp FROM answers
                        LEFT JOIN users ON answers.user = users.id
                        LEFT JOIN temporary_users ON answers.temporary_user_id = temporary_users.id
                        WHERE answers.id = ?",
                )
                .bind(answer_id.into_inner().to_string())
                .fetch_optional(&mut **txn)
                .await?;

                let contents = sqlx::query(
                    r"SELECT id, question_id, answer FROM real_answers WHERE answer_id = ?",
                )
                .bind(answer_id.into_inner().to_string())
                .fetch_all(&mut **txn)
                .await?;

                let contents = contents
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerContentRecord {
                            id: rs.try_get("id")?,
                            question_id: rs.try_get("question_id")?,
                            answer: rs.try_get("answer")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                answer_query_result_opt
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerRecord {
                            id: rs.try_get("answer_id")?,
                            author: author_from_row(&rs)?,
                            timestamp: rs.try_get("timestamp")?,
                            form_id: rs.try_get("form_id")?,
                            title: rs.try_get("title")?,
                            contents,
                            comments: Vec::new(),
                            messages: Vec::new(),
                        })
                    })
                    .transpose()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_answers_by_answer_ids(
        &self,
        answer_ids: Vec<AnswerId>,
    ) -> Result<Vec<FormAnswerRecord>, InfraError> {
        if answer_ids.is_empty() {
            return Ok(Vec::new());
        }

        let ids = answer_ids
            .iter()
            .map(|id| id.into_inner().to_string())
            .collect_vec();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let sql = format!(
                    "SELECT form_id, answers.id AS answer_id, title, author_type, user,
                        users.name AS user_name, users.role AS user_role,
                        temporary_user_id, temporary_users.name AS temporary_user_name,
                        temporary_users.contact_text AS temporary_user_contact_text,
                        timestamp FROM answers
                        LEFT JOIN users ON answers.user = users.id
                        LEFT JOIN temporary_users ON answers.temporary_user_id = temporary_users.id
                        WHERE answers.id IN ({})
                        ORDER BY answers.timestamp",
                    std::iter::repeat_n("?", ids.len()).join(", ")
                );
                let answers = ids
                    .iter()
                    .fold(query(&sql), |query, id| query.bind(id))
                    .fetch_all(&mut **txn)
                    .await?;

                let form_answer_records = answers
                    .into_iter()
                    .map(|rs| {
                        let answer_id = Uuid::from_str(&rs.try_get::<String, _>("answer_id")?)?;

                        Ok::<_, InfraError>(FormAnswerRecord {
                            id: answer_id.to_string(),
                            author: author_from_row(&rs)?,
                            timestamp: rs.try_get("timestamp")?,
                            form_id: rs.try_get("form_id")?,
                            title: rs.try_get("title")?,
                            contents: Vec::new(),
                            comments: Vec::new(),
                            messages: Vec::new(),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let answer_ids = form_answer_records
                    .iter()
                    .map(|record| record.id.to_owned())
                    .collect_vec();

                let contents = fetch_real_answers_by_answer_ids(txn, &answer_ids).await?;
                attach_contents(form_answer_records, contents)
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn update_answer_entry(
        &self,
        answer_entry: &AnswerEntry,
        form_id: FormId,
    ) -> Result<(), InfraError> {
        let answer_id = answer_entry.id().to_owned().into_inner().to_string();
        let form_id = form_id.into_inner().to_string();
        let (author_type, user, temporary_user_id) = answer_author_columns(answer_entry);
        let temporary_user = answer_entry.author().temporary_user().cloned();
        let title = <Option<NonEmptyString> as Clone>::clone(&answer_entry.title().to_owned())
            .map(|title| title.into_inner());

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                if let Some(temporary_user) = temporary_user {
                    sqlx::query!(
                        r"INSERT INTO temporary_users (id, name, contact_text)
                        VALUES (?, ?, ?)
                        ON DUPLICATE KEY UPDATE name = VALUES(name), contact_text = VALUES(contact_text)",
                        temporary_user.id().to_string(),
                        temporary_user.name(),
                        temporary_user.contact_text(),
                    )
                    .execute(&mut **txn)
                    .await?;
                }

                sqlx::query!(
                    r#"INSERT INTO answers (id, form_id, author_type, user, temporary_user_id, title)
                    VALUES (?, ?, ?, ?, ?, ?)
                    ON DUPLICATE KEY UPDATE
                    title = VALUES(title)"#,
                    answer_id,
                    form_id,
                    author_type,
                    user,
                    temporary_user_id,
                    title,
                )
                .execute(&mut **txn)
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
                    sqlx::query_scalar!("SELECT COUNT(*) AS `count!: i64` FROM real_answers")
                        .fetch_one(&mut **txn)
                        .await?;

                count_as_u32(size, "real_answers")
            })
        })
        .await
    }
}
