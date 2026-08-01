use std::str::FromStr;

use async_trait::async_trait;
use domain::{
    account::models::{AccountUser, Role},
    form::{
        answer::{
            AnswerAuthor, AnswerEntry, AnswerId, RedmineImportedAnswerReference,
            RedmineUserSnapshot, TemporaryAnswerAuthor,
        },
        models::FormId,
    },
};
use errors::infra::InfraError;
use itertools::Itertools;
use sqlx::{AssertSqlSafe, Row, mysql::MySqlRow, query};
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

use crate::{
    database::{
        components::FormAnswerDatabase,
        connection::{ConnectionPool, DatabaseTransaction},
        count::count_as_u32,
    },
    records::{AnswerAuthorRecord, FormAnswerContentRecord, FormAnswerRecord, MessageRecord},
};

fn answer_author_columns(
    answer: &AnswerEntry,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
) {
    match answer.author() {
        AnswerAuthor::AuthenticatedUser(user_id) => (
            "AUTHENTICATED_USER".to_string(),
            Some(user_id.to_string()),
            None,
            None,
            None,
        ),
        AnswerAuthor::Temporary(temporary_user) => (
            "TEMPORARY_USER".to_string(),
            None,
            Some(temporary_user.id().to_string()),
            None,
            None,
        ),
        AnswerAuthor::ImportedFromRedmine(author) => (
            "IMPORTED_FROM_REDMINE".to_string(),
            None,
            None,
            *author.redmine_user_id(),
            Some(author.display_name().to_owned()),
        ),
    }
}

fn validated_redmine_issue_id(answer: &AnswerEntry) -> Result<Option<i64>, InfraError> {
    let issue_id = answer
        .redmine_reference()
        .map(|reference| reference.issue_id().into_inner());
    let is_imported = matches!(answer.author(), AnswerAuthor::ImportedFromRedmine(_));
    if is_imported != issue_id.is_some() {
        return Err(InfraError::Unexpected {
            cause: "imported answers must have exactly one Redmine issue reference".to_string(),
        });
    }
    Ok(issue_id)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn author_from_values(
    author_type: String,
    user: Option<String>,
    user_name: Option<String>,
    user_role: Option<String>,
    temporary_user_id: Option<String>,
    temporary_user_name: Option<String>,
    temporary_user_contact_text: Option<String>,
    redmine_user_id: Option<i64>,
    redmine_author_name: Option<String>,
) -> Result<AnswerAuthorRecord, InfraError> {
    match author_type.as_str() {
        "AUTHENTICATED_USER" => Ok(AnswerAuthorRecord::AuthenticatedUser(AccountUser::new(
            user_name.ok_or_else(|| InfraError::Unexpected {
                cause: "authenticated answer author is missing user_name".to_string(),
            })?,
            Uuid::from_str(&user.ok_or_else(|| InfraError::Unexpected {
                cause: "authenticated answer author is missing user".to_string(),
            })?)?
            .into(),
            Role::from_str(&user_role.ok_or_else(|| InfraError::Unexpected {
                cause: "authenticated answer author is missing user_role".to_string(),
            })?)?,
        ))),
        "TEMPORARY_USER" => Ok(AnswerAuthorRecord::TemporaryAnswerAuthor(unsafe {
            TemporaryAnswerAuthor::from_raw_parts(
                Uuid::from_str(&temporary_user_id.ok_or_else(|| InfraError::Unexpected {
                    cause: "temporary answer author is missing temporary_user_id".to_string(),
                })?)?
                .into(),
                temporary_user_name.ok_or_else(|| InfraError::Unexpected {
                    cause: "temporary answer author is missing temporary_user_name".to_string(),
                })?,
                temporary_user_contact_text.ok_or_else(|| InfraError::Unexpected {
                    cause: "temporary answer author is missing temporary_user_contact_text"
                        .to_string(),
                })?,
            )
        })),
        "IMPORTED_FROM_REDMINE" => Ok(AnswerAuthorRecord::ImportedFromRedmine(unsafe {
            RedmineUserSnapshot::from_raw_parts(
                redmine_user_id,
                redmine_author_name.ok_or_else(|| InfraError::Unexpected {
                    cause: "imported answer author is missing redmine_author_name".to_string(),
                })?,
            )
        })),
        value => Err(InfraError::Unexpected {
            cause: format!("unknown answer author_type: {value}"),
        }),
    }
}

pub(crate) fn author_from_row(row: &MySqlRow) -> Result<AnswerAuthorRecord, InfraError> {
    author_from_values(
        row.try_get("author_type")?,
        row.try_get("user")?,
        row.try_get("user_name")?,
        row.try_get("user_role")?,
        row.try_get("temporary_user_id")?,
        row.try_get("temporary_user_name")?,
        row.try_get("temporary_user_contact_text")?,
        row.try_get("redmine_user_id")?,
        row.try_get("redmine_author_name")?,
    )
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
        .fold(query(AssertSqlSafe(&*sql)), |query, answer_id| {
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
        .fold(query(AssertSqlSafe(&*sql)), |query, answer_id| {
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
    message_records: Vec<(Uuid, MessageRecord)>,
) -> Result<Vec<FormAnswerRecord>, InfraError> {
    let grouped_contents = content_records
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
        let (author_type, user_id, temporary_user_id, redmine_user_id, redmine_author_name) =
            answer_author_columns(answer);
        let redmine_issue_id = validated_redmine_issue_id(answer)?;
        let temporary_user = answer.author().temporary_user().cloned();
        let title = <Option<NonEmptyString> as Clone>::clone(&answer.title().to_owned())
            .map(|title| title.into_inner());
        let publication = answer.publication().to_string();
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
                    r"INSERT INTO answers (id, form_id, author_type, user, temporary_user_id,
                        redmine_user_id, redmine_author_name, title, publication, timestamp)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    answer_id,
                    form_id,
                    author_type,
                    user_id,
                    temporary_user_id,
                    redmine_user_id,
                    redmine_author_name,
                    title,
                    publication,
                    timestamp,
                )
                .execute(&mut **txn)
                .await?;

                if let Some(redmine_issue_id) = redmine_issue_id {
                    sqlx::query!(
                        "INSERT INTO redmine_imported_answer_references (answer_id, redmine_issue_id) VALUES (?, ?)",
                        answer_id,
                        redmine_issue_id,
                    )
                    .execute(&mut **txn)
                    .await?;
                }

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
                        .fold(query(AssertSqlSafe(&*sql)), |query, value| query.bind(value))
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
                let answer_query_result_opt = sqlx::query!(
                    r"SELECT form_id, answers.id AS answer_id, title, publication, author_type, user,
                        users.name AS user_name, users.role AS user_role,
                        temporary_user_id, temporary_users.name AS temporary_user_name,
                        temporary_users.contact_text AS temporary_user_contact_text,
                        answers.redmine_user_id, answers.redmine_author_name,
                        redmine_reference.redmine_issue_id,
                        timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>` FROM answers
                        LEFT JOIN users ON answers.user = users.id
                        LEFT JOIN temporary_users ON answers.temporary_user_id = temporary_users.id
                        LEFT JOIN redmine_imported_answer_references redmine_reference
                            ON redmine_reference.answer_id = answers.id
                        WHERE answers.id = ?",
                    answer_id.into_inner().to_string(),
                )
                .fetch_optional(&mut **txn)
                .await?;

                let contents = sqlx::query!(
                    r"SELECT id, question_id, answer FROM real_answers WHERE answer_id = ?",
                    answer_id.into_inner().to_string(),
                )
                .fetch_all(&mut **txn)
                .await?;

                let contents = contents
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerContentRecord {
                            id: rs.id,
                            question_id: rs.question_id,
                            answer: rs.answer,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                answer_query_result_opt
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerRecord {
                            id: rs.answer_id,
                            author: author_from_values(
                                rs.author_type,
                                rs.user,
                                rs.user_name,
                                rs.user_role,
                                rs.temporary_user_id,
                                rs.temporary_user_name,
                                rs.temporary_user_contact_text,
                                rs.redmine_user_id,
                                rs.redmine_author_name,
                            )?,
                            timestamp: rs.timestamp,
                            form_id: rs.form_id,
                            title: rs.title,
                            publication: rs.publication,
                            contents,
                            messages: Vec::new(),
                            redmine_reference: rs
                                .redmine_issue_id
                                .map(|issue_id| {
                                    RedmineImportedAnswerReference::new(
                                        answer_id,
                                        issue_id.into(),
                                    )
                                }),
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
                    "SELECT form_id, answers.id AS answer_id, title, publication, author_type, user,
                        users.name AS user_name, users.role AS user_role,
                        temporary_user_id, temporary_users.name AS temporary_user_name,
                        temporary_users.contact_text AS temporary_user_contact_text,
                        answers.redmine_user_id, answers.redmine_author_name,
                        redmine_reference.redmine_issue_id,
                        timestamp FROM answers
                        LEFT JOIN users ON answers.user = users.id
                        LEFT JOIN temporary_users ON answers.temporary_user_id = temporary_users.id
                        LEFT JOIN redmine_imported_answer_references redmine_reference
                            ON redmine_reference.answer_id = answers.id
                        WHERE answers.id IN ({})
                        ORDER BY answers.timestamp",
                    std::iter::repeat_n("?", ids.len()).join(", ")
                );
                let answers = ids
                    .iter()
                    .fold(query(AssertSqlSafe(&*sql)), |query, id| query.bind(id))
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
                            publication: rs.try_get("publication")?,
                            contents: Vec::new(),
                            messages: Vec::new(),
                            redmine_reference: rs
                                .try_get::<Option<i64>, _>("redmine_issue_id")?
                                .map(|issue_id| {
                                    RedmineImportedAnswerReference::new(
                                        answer_id.into(),
                                        issue_id.into(),
                                    )
                                }),
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
        let (author_type, user, temporary_user_id, redmine_user_id, redmine_author_name) =
            answer_author_columns(answer_entry);
        let redmine_issue_id = validated_redmine_issue_id(answer_entry)?;
        let temporary_user = answer_entry.author().temporary_user().cloned();
        let title = <Option<NonEmptyString> as Clone>::clone(&answer_entry.title().to_owned())
            .map(|title| title.into_inner());
        let publication = answer_entry.publication().to_string();

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
                    r#"INSERT INTO answers (id, form_id, author_type, user, temporary_user_id,
                    redmine_user_id, redmine_author_name, title, publication)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON DUPLICATE KEY UPDATE
                    title = VALUES(title),
                    publication = VALUES(publication)"#,
                    answer_id,
                    form_id,
                    author_type,
                    user,
                    temporary_user_id,
                    redmine_user_id,
                    redmine_author_name,
                    title,
                    publication,
                )
                .execute(&mut **txn)
                .await?;
                if let Some(redmine_issue_id) = redmine_issue_id {
                    sqlx::query!(
                        r"INSERT INTO redmine_imported_answer_references (answer_id, redmine_issue_id)
                        VALUES (?, ?)
                        ON DUPLICATE KEY UPDATE redmine_issue_id = VALUES(redmine_issue_id)",
                        answer_id,
                        redmine_issue_id,
                    )
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
                let size = sqlx::query_scalar!("SELECT COUNT(*) AS `count!: i64` FROM answers")
                    .fetch_one(&mut **txn)
                    .await?;

                count_as_u32(size, "answers")
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn content_size(&self) -> Result<u32, InfraError> {
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
