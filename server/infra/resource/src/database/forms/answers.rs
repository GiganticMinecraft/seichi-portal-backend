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
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

use crate::database::connection::query_one;
use crate::{
    database::{
        components::FormAnswerDatabase,
        connection::{
            ConnectionPool, batch_insert, execute_and_values, query_all, query_all_and_values,
            query_one_and_values,
        },
    },
    dto::{FormAnswerContentDto, FormAnswerDto},
};

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
            .flat_map(|content| {
                [
                    answer_id.to_owned().into(),
                    content
                        .question_id
                        .to_owned()
                        .into_inner()
                        .to_string()
                        .into(),
                    content.answer.to_owned().into(),
                ]
            })
            .collect::<Vec<_>>();

        self.read_write_transaction(move |txn| {
            Box::pin(async move {
                execute_and_values(
                    r"INSERT INTO answers (id, form_id, user, title, timestamp) VALUES (?, ?, ?, ?, ?)",
                    [
                        answer_id.into(),
                        form_id.into(),
                        user_id.into(),
                        title.into(),
                        timestamp.into(),
                    ],
                    txn,
                )
                .await?;

                batch_insert(
                    r"INSERT INTO real_answers (answer_id, question_id, answer) VALUES (?, ?, ?)",
                    contents,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<FormAnswerDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answer_query_result_opt = query_one_and_values(
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, timestamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        WHERE answers.id = ?",
                    [answer_id.into_inner().to_string().into()],
                    txn,
                ).await?;

                let contents = query_all_and_values(
                    r"SELECT id, question_id, answer FROM real_answers WHERE answer_id = ?",
                    [answer_id.into_inner().to_string().into()],
                    txn,
                )
                    .await?;

                let contents = contents
                    .iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerContentDto {
                            id: rs.try_get("", "id")?,
                            question_id: rs.try_get("", "question_id")?,
                            answer: rs.try_get("", "answer")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                answer_query_result_opt
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            id: answer_id.into_inner().to_string(),
                            uuid: rs.try_get("", "user")?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "timestamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                            contents
                        })
                    })
                    .transpose()
            })
        })
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<Vec<FormAnswerDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answers = query_all_and_values(
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, timestamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        WHERE form_id = ?
                        ORDER BY answers.timestamp",
                    [form_id.into_inner().to_string().into()],
                    txn,
                ).await?;

                let form_answer_dtos = answers
                    .iter()
                    .map(|rs| {
                        let answer_id = Uuid::from_str(&rs.try_get::<String>("", "answer_id")?)?;

                        Ok::<_, InfraError>(FormAnswerDto {
                            id: answer_id.to_string(),
                            uuid: rs.try_get("", "user")?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "timestamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                            contents: Vec::new()
                        })
                    }).collect::<Result<Vec<_>, _>>()?;

                let answer_ids = form_answer_dtos.iter().map(|dto| dto.id.to_owned()).collect_vec();

                let contents = if answer_ids.is_empty() {
                    Vec::new()
                } else {
                    query_all_and_values(
                        format!("SELECT id, question_id, answer, answer_id FROM real_answers WHERE answer_id IN ({})", vec!["?"; answer_ids.len()].join(",")).as_str(),
                        answer_ids.into_iter().map(|id| id.to_string().into()),
                        txn,
                    ).await?
                };


                let answer_id_with_content_dto = contents
                    .iter()
                    .map(|rs| {
                        Ok::<_, InfraError>((Uuid::from_str(&rs.try_get::<String>("", "answer_id")?)?, FormAnswerContentDto {
                            id: rs.try_get("", "id")?,
                            question_id: rs.try_get("", "question_id")?,
                            answer: rs.try_get("", "answer")?,
                        }))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let grouped_answer_contents = answer_id_with_content_dto
                    .into_iter()
                    .into_group_map_by(|(answer_id, _)| answer_id.to_owned());


                form_answer_dtos
                    .into_iter()
                    .map(|dto| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            contents: grouped_answer_contents.get(&Uuid::from_str(&dto.id)?).cloned()
                                .map(|contents| contents.into_iter().map(|(_, content_dto)| content_dto).collect_vec())
                                .unwrap_or_default(),
                            ..dto
                        })
                    }).collect()

            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_all_answers(&self) -> Result<Vec<FormAnswerDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answers = query_all(
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, timestamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        ORDER BY answers.timestamp",
                    txn,
                ).await?;

                let form_answer_dtos = answers
                    .iter()
                    .map(|rs| {
                        let answer_id = Uuid::from_str(&rs.try_get::<String>("", "answer_id")?)?;

                        Ok::<_, InfraError>(FormAnswerDto {
                            id: answer_id.to_string(),
                            uuid: rs.try_get("", "user")?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "timestamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                            contents: Vec::new()
                        })
                    }).collect::<Result<Vec<_>, _>>()?;

                let answer_ids = form_answer_dtos.iter().map(|dto| dto.id.to_owned()).collect_vec();

                let contents = if answer_ids.is_empty() {
                    Vec::new()
                } else {
                    query_all_and_values(
                        format!("SELECT id, question_id, answer, answer_id FROM real_answers WHERE answer_id IN ({})", vec!["?"; answer_ids.len()].join(",")).as_str(),
                        answer_ids.into_iter().map(|id| id.to_string().into()),
                        txn,
                    ).await?
                };

                let answer_id_with_content_dto = contents
                    .iter()
                    .map(|rs| {
                        Ok::<_, InfraError>((Uuid::from_str(&rs.try_get::<String>("", "answer_id")?)?, FormAnswerContentDto {
                            id: rs.try_get("", "id")?,
                            question_id: rs.try_get("", "question_id")?,
                            answer: rs.try_get("", "answer")?,
                        }))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let grouped_answer_contents = answer_id_with_content_dto
                    .into_iter()
                    .into_group_map_by(|(answer_id, _)| answer_id.to_owned());

                form_answer_dtos
                    .into_iter()
                    .map(|dto| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            contents: grouped_answer_contents.get(&Uuid::from_str(&dto.id)?).cloned()
                                .map(|contents| contents.into_iter().map(|(_, content_dto)| content_dto).collect_vec())
                                .unwrap_or_default(),
                            ..dto
                        })
                    }).collect()
            })
        })
            .await
            .map_err(Into::into)
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
            .map(|id| id.into_inner().to_string().into())
            .collect_vec();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answers = query_all_and_values(
                    format!("SELECT form_id, answers.id AS answer_id, title, user, name, role, timestamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        WHERE answers.id IN ({})
                        ORDER BY answers.timestamp", &vec!["?"; ids.len()].iter().join(", ")).as_str(),
                    ids,
                    txn,
                ).await?;

                let form_answer_dtos = answers
                    .iter()
                    .map(|rs| {
                        let answer_id = Uuid::from_str(&rs.try_get::<String>("", "answer_id")?)?;

                        Ok::<_, InfraError>(FormAnswerDto {
                            id: answer_id.to_string(),
                            uuid: rs.try_get("", "user")?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "timestamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                            contents: Vec::new()
                        })
                    }).collect::<Result<Vec<_>, _>>()?;

                let answer_ids = form_answer_dtos.iter().map(|dto| dto.id.to_owned()).collect_vec();

                let contents = if answer_ids.is_empty() {
                    Vec::new()
                } else {
                    query_all_and_values(
                        format!("SELECT id, question_id, answer, answer_id FROM real_answers WHERE answer_id IN ({})", vec!["?"; answer_ids.len()].join(",")).as_str(),
                        answer_ids.into_iter().map(|id| id.to_string().into()),
                        txn,
                    ).await?
                };


                let answer_id_with_content_dto = contents
                    .iter()
                    .map(|rs| {
                        Ok::<_, InfraError>((Uuid::from_str(&rs.try_get::<String>("", "answer_id")?)?, FormAnswerContentDto {
                            id: rs.try_get("", "id")?,
                            question_id: rs.try_get("", "question_id")?,
                            answer: rs.try_get("", "answer")?,
                        }))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let grouped_answer_contents = answer_id_with_content_dto
                    .into_iter()
                    .into_group_map_by(|(answer_id, _)| answer_id.to_owned());


                form_answer_dtos
                    .into_iter()
                    .map(|dto| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            contents: grouped_answer_contents.get(&Uuid::from_str(&dto.id)?).cloned()
                                .map(|contents| contents.into_iter().map(|(_, content_dto)| content_dto).collect_vec())
                                .unwrap_or_default(),
                            ..dto
                        })
                    }).collect()

            })
        }).await
            .map_err(Into::into)
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
                execute_and_values(
                    r#"INSERT INTO answers (id, form_id, user, title)
                    VALUES (?, ?, ?, ?)
                    ON DUPLICATE KEY UPDATE
                    title = VALUES(title)"#,
                    [answer_id.into(), form_id.into(), user.into(), title.into()],
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
                let query = query_one("SELECT COUNT(*) AS count FROM real_answers", txn).await?;

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
