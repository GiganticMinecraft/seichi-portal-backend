use std::str::FromStr;

use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerEntry, AnswerId},
        models::FormId,
    },
    user::models::Role,
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
    dto::{FormAnswerContentDto, FormAnswerDto},
};

async fn fetch_real_answers_by_answer_ids<T>(
    txn: &mut DatabaseTransaction,
    answer_ids: &[T],
) -> Result<Vec<(Uuid, FormAnswerContentDto)>, InfraError>
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
                FormAnswerContentDto {
                    id: row.try_get("id")?,
                    question_id: row.try_get("question_id")?,
                    answer: row.try_get("answer")?,
                },
            ))
        })
        .collect()
}

fn attach_contents(
    form_answer_dtos: Vec<FormAnswerDto>,
    answer_id_with_content_dto: Vec<(Uuid, FormAnswerContentDto)>,
) -> Result<Vec<FormAnswerDto>, InfraError> {
    let grouped_answer_contents = answer_id_with_content_dto
        .into_iter()
        .into_group_map_by(|(answer_id, _)| *answer_id);

    form_answer_dtos
        .into_iter()
        .map(|dto| {
            Ok::<_, InfraError>(FormAnswerDto {
                contents: grouped_answer_contents
                    .get(&Uuid::from_str(&dto.id)?)
                    .cloned()
                    .map(|contents| {
                        contents
                            .into_iter()
                            .map(|(_, content_dto)| content_dto)
                            .collect_vec()
                    })
                    .unwrap_or_default(),
                ..dto
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
        let user_id = answer.user().to_owned().id.to_string().to_owned();
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
                sqlx::query!(
                    r"INSERT INTO answers (id, form_id, user, title, timestamp) VALUES (?, ?, ?, ?, ?)",
                    answer_id,
                    form_id,
                    user_id,
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
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<FormAnswerDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answer_query_result_opt = sqlx::query(
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, timestamp FROM answers
                        INNER JOIN users ON answers.user = users.id
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
                        Ok::<_, InfraError>(FormAnswerContentDto {
                            id: rs.try_get("id")?,
                            question_id: rs.try_get("question_id")?,
                            answer: rs.try_get("answer")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                answer_query_result_opt
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            id: rs.try_get("answer_id")?,
                            uuid: rs.try_get("user")?,
                            user_name: rs.try_get("name")?,
                            user_role: Role::from_str(&rs.try_get::<String, _>("role")?)?,
                            timestamp: rs.try_get("timestamp")?,
                            form_id: rs.try_get("form_id")?,
                            title: rs.try_get("title")?,
                            contents
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
    ) -> Result<Vec<FormAnswerDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answers = sqlx::query(
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, timestamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        WHERE form_id = ?
                        ORDER BY answers.timestamp",
                )
                .bind(form_id.into_inner().to_string())
                .fetch_all(&mut **txn)
                .await?;

                let form_answer_dtos = answers
                    .into_iter()
                    .map(|rs| {
                        let answer_id = Uuid::from_str(&rs.try_get::<String, _>("answer_id")?)?;

                        Ok::<_, InfraError>(FormAnswerDto {
                            id: answer_id.to_string(),
                            uuid: rs.try_get("user")?,
                            user_name: rs.try_get("name")?,
                            user_role: Role::from_str(&rs.try_get::<String, _>("role")?)?,
                            timestamp: rs.try_get("timestamp")?,
                            form_id: rs.try_get("form_id")?,
                            title: rs.try_get("title")?,
                            contents: Vec::new()
                        })
                    }).collect::<Result<Vec<_>, _>>()?;

                let answer_ids = form_answer_dtos.iter().map(|dto| dto.id.to_owned()).collect_vec();

                let contents = fetch_real_answers_by_answer_ids(txn, &answer_ids).await?;
                attach_contents(form_answer_dtos, contents)
            })
        }).await
    }

    #[tracing::instrument]
    async fn get_all_answers(&self) -> Result<Vec<FormAnswerDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answers = sqlx::query(
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, timestamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        ORDER BY answers.timestamp",
                )
                .fetch_all(&mut **txn)
                .await?;

                let form_answer_dtos = answers
                    .into_iter()
                    .map(|rs| {
                        let answer_id = Uuid::from_str(&rs.try_get::<String, _>("answer_id")?)?;

                        Ok::<_, InfraError>(FormAnswerDto {
                            id: answer_id.to_string(),
                            uuid: rs.try_get("user")?,
                            user_name: rs.try_get("name")?,
                            user_role: Role::from_str(&rs.try_get::<String, _>("role")?)?,
                            timestamp: rs.try_get("timestamp")?,
                            form_id: rs.try_get("form_id")?,
                            title: rs.try_get("title")?,
                            contents: Vec::new()
                        })
                    }).collect::<Result<Vec<_>, _>>()?;

                let answer_ids = form_answer_dtos.iter().map(|dto| dto.id.to_owned()).collect_vec();
                let contents = fetch_real_answers_by_answer_ids(txn, &answer_ids).await?;
                attach_contents(form_answer_dtos, contents)
            })
        })
            .await
    }

    #[tracing::instrument]
    async fn get_answers_by_answer_ids(
        &self,
        answer_ids: Vec<AnswerId>,
    ) -> Result<Vec<FormAnswerDto>, InfraError> {
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
                    "SELECT form_id, answers.id AS answer_id, title, user, name, role, timestamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        WHERE answers.id IN ({})
                        ORDER BY answers.timestamp",
                    std::iter::repeat_n("?", ids.len()).join(", ")
                );
                let answers = ids
                    .iter()
                    .fold(query(&sql), |query, id| query.bind(id))
                    .fetch_all(&mut **txn)
                    .await?;

                let form_answer_dtos = answers
                    .into_iter()
                    .map(|rs| {
                        let answer_id = Uuid::from_str(&rs.try_get::<String, _>("answer_id")?)?;

                        Ok::<_, InfraError>(FormAnswerDto {
                            id: answer_id.to_string(),
                            uuid: rs.try_get("user")?,
                            user_name: rs.try_get("name")?,
                            user_role: Role::from_str(&rs.try_get::<String, _>("role")?)?,
                            timestamp: rs.try_get("timestamp")?,
                            form_id: rs.try_get("form_id")?,
                            title: rs.try_get("title")?,
                            contents: Vec::new()
                        })
                    }).collect::<Result<Vec<_>, _>>()?;

                let answer_ids = form_answer_dtos.iter().map(|dto| dto.id.to_owned()).collect_vec();

                let contents = fetch_real_answers_by_answer_ids(txn, &answer_ids).await?;
                attach_contents(form_answer_dtos, contents)
            })
        }).await
    }

    #[tracing::instrument]
    async fn update_answer_entry(&self, answer_entry: &AnswerEntry) -> Result<(), InfraError> {
        let answer_id = answer_entry.id().to_owned().into_inner().to_string();
        let form_id = answer_entry.form_id().to_owned().to_string();
        let user = answer_entry.user().id.to_owned().to_string();
        let title = <Option<NonEmptyString> as Clone>::clone(&answer_entry.title().to_owned())
            .map(|title| title.into_inner());

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r#"INSERT INTO answers (id, form_id, user, title)
                    VALUES (?, ?, ?, ?)
                    ON DUPLICATE KEY UPDATE
                    title = VALUES(title)"#,
                    answer_id,
                    form_id,
                    user,
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
