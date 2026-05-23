use std::str::FromStr;

use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerAuthor, AnswerEntry, AnswerId},
        models::FormId,
    },
    user::models::{Role, TemporaryUser, User},
};
use errors::infra::InfraError;
use itertools::Itertools;
use sqlx::{Row, query};
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

use crate::{
    database::{
        components::FormAnswerDatabase,
        connection::{ConnectionPool, DatabaseTransaction},
        count::count_as_u32,
    },
    records::{AnswerAuthorRecord, FormAnswerContentRecord, FormAnswerRecord},
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
            Some(temporary_user.id.to_string()),
        ),
    }
}

fn author_from_row(row: &sqlx::mysql::MySqlRow) -> Result<AnswerAuthorRecord, InfraError> {
    let author_type: String = row.try_get("author_type")?;
    match author_type.as_str() {
        "AUTHENTICATED_USER" => Ok(AnswerAuthorRecord::AuthenticatedUser(User {
            id: Uuid::from_str(&row.try_get::<String, _>("user")?)?.into(),
            name: row.try_get("user_name")?,
            role: Role::from_str(&row.try_get::<String, _>("user_role")?)?,
        })),
        "TEMPORARY_USER" => Ok(AnswerAuthorRecord::TemporaryUser(TemporaryUser {
            id: Uuid::from_str(&row.try_get::<String, _>("temporary_user_id")?)?.into(),
            name: row.try_get("temporary_user_name")?,
            contact_text: row.try_get("temporary_user_contact_text")?,
        })),
        value => Err(InfraError::Unexpected {
            cause: format!("unknown answer author_type: {value}"),
        }),
    }
}

async fn fetch_real_answers_by_answer_ids<T>(
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

fn attach_contents(
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

#[async_trait]
impl FormAnswerDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn post_answer(&self, answer: &AnswerEntry) -> Result<(), InfraError> {
        let answer_id = answer.id().to_owned().into_inner().to_string();
        let form_id = answer.form_id().to_owned().into_inner().to_string();
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
                    sqlx::query(
                        r"INSERT INTO temporary_users (id, name, contact_text)
                        VALUES (?, ?, ?)
                        ON DUPLICATE KEY UPDATE name = VALUES(name), contact_text = VALUES(contact_text)",
                    )
                    .bind(temporary_user.id.to_string())
                    .bind(&temporary_user.name)
                    .bind(&temporary_user.contact_text)
                    .execute(&mut **txn)
                    .await?;
                }

                sqlx::query(
                    r"INSERT INTO answers (id, form_id, author_type, user, temporary_user_id, title, timestamp)
                    VALUES (?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(answer_id)
                .bind(form_id)
                .bind(author_type)
                .bind(user_id)
                .bind(temporary_user_id)
                .bind(title)
                .bind(timestamp)
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
                        })
                    })
                    .transpose()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<Vec<FormAnswerRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answers = sqlx::query(
                    r"SELECT form_id, answers.id AS answer_id, title, author_type, user,
                        users.name AS user_name, users.role AS user_role,
                        temporary_user_id, temporary_users.name AS temporary_user_name,
                        temporary_users.contact_text AS temporary_user_contact_text,
                        timestamp FROM answers
                        LEFT JOIN users ON answers.user = users.id
                        LEFT JOIN temporary_users ON answers.temporary_user_id = temporary_users.id
                        WHERE form_id = ?
                        ORDER BY answers.timestamp",
                )
                .bind(form_id.into_inner().to_string())
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
    async fn get_all_answers(&self) -> Result<Vec<FormAnswerRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answers = sqlx::query(
                    r"SELECT form_id, answers.id AS answer_id, title, author_type, user,
                        users.name AS user_name, users.role AS user_role,
                        temporary_user_id, temporary_users.name AS temporary_user_name,
                        temporary_users.contact_text AS temporary_user_contact_text,
                        timestamp FROM answers
                        LEFT JOIN users ON answers.user = users.id
                        LEFT JOIN temporary_users ON answers.temporary_user_id = temporary_users.id
                        ORDER BY answers.timestamp",
                )
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
    async fn update_answer_entry(&self, answer_entry: &AnswerEntry) -> Result<(), InfraError> {
        let answer_id = answer_entry.id().to_owned().into_inner().to_string();
        let form_id = answer_entry.form_id().to_owned().to_string();
        let (author_type, user, temporary_user_id) = answer_author_columns(answer_entry);
        let temporary_user = answer_entry.author().temporary_user().cloned();
        let title = <Option<NonEmptyString> as Clone>::clone(&answer_entry.title().to_owned())
            .map(|title| title.into_inner());

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                if let Some(temporary_user) = temporary_user {
                    sqlx::query(
                        r"INSERT INTO temporary_users (id, name, contact_text)
                        VALUES (?, ?, ?)
                        ON DUPLICATE KEY UPDATE name = VALUES(name), contact_text = VALUES(contact_text)",
                    )
                    .bind(temporary_user.id.to_string())
                    .bind(temporary_user.name)
                    .bind(temporary_user.contact_text)
                    .execute(&mut **txn)
                    .await?;
                }

                sqlx::query(
                    r#"INSERT INTO answers (id, form_id, author_type, user, temporary_user_id, title)
                    VALUES (?, ?, ?, ?, ?, ?)
                    ON DUPLICATE KEY UPDATE
                    title = VALUES(title)"#,
                )
                .bind(answer_id)
                .bind(form_id)
                .bind(author_type)
                .bind(user)
                .bind(temporary_user_id)
                .bind(title)
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
