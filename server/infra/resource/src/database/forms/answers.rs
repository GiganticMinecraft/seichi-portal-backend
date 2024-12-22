use std::str::FromStr;

use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerId, FormAnswer, FormAnswerContent},
        models::FormId,
    },
    user::models::Role,
};
use errors::infra::InfraError;
use types::non_empty_string::NonEmptyString;

use crate::{
    database::{
        components::FormAnswerDatabase,
        connection::{
            batch_insert, execute_and_values, query_all, query_all_and_values,
            query_one_and_values, ConnectionPool,
        },
    },
    dto::{FormAnswerContentDto, FormAnswerDto},
};

#[async_trait]
impl FormAnswerDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn post_answer(
        &self,
        answer: &FormAnswer,
        content: Vec<FormAnswerContent>,
    ) -> Result<(), InfraError> {
        let answer_id = answer.id().to_owned().into_inner();
        let form_id = answer.form_id().to_owned().into_inner();
        let user_id = answer.user().to_owned().id.to_owned();
        let title =
            <std::option::Option<NonEmptyString> as Clone>::clone(&answer.title().to_owned())
                .map(|title| title.into_inner());
        let timestamp = answer.timestamp().to_owned();

        self.read_write_transaction(|txn| {
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

                let contents = content
                    .iter()
                    .flat_map(|content| {
                        [
                            answer_id.into(),
                            content.question_id.into_inner().into(),
                            content.answer.clone().into(),
                        ]
                    })
                    .collect::<Vec<_>>();

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
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, time_stamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        WHERE answers.id = ?",
                    [answer_id.into_inner().into()],
                    txn,
                ).await?;

                answer_query_result_opt
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            id: answer_id.into_inner(),
                            uuid: uuid::Uuid::from_str(&rs.try_get::<String>("", "user")?)?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "time_stamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                        })
                    })
                    .transpose()
            })
        })
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_answer_contents(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<FormAnswerContentDto>, InfraError> {
        let answer_id = answer_id.into_inner();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let contents = query_all_and_values(
                    r"SELECT question_id, answer FROM real_answers WHERE answer_id = ?",
                    [answer_id.into()],
                    txn,
                )
                .await?;

                contents
                    .iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerContentDto {
                            answer_id,
                            question_id: rs.try_get("", "question_id")?,
                            answer: rs.try_get("", "answer")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
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
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, time_stamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        WHERE form_id = ?
                        ORDER BY answers.time_stamp",
                    [form_id.into_inner().into()],
                    txn,
                ).await?;

                answers
                    .iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            id: rs.try_get("", "answer_id")?,
                            uuid: uuid::Uuid::from_str(&rs.try_get::<String>("", "user")?)?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "time_stamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_all_answers(&self) -> Result<Vec<FormAnswerDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answers = query_all(
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, time_stamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        ORDER BY answers.time_stamp",
                    txn,
                ).await?;

                answers
                    .iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            id: rs.try_get("", "answer_id")?,
                            uuid: uuid::Uuid::from_str(&rs.try_get::<String>("", "user")?)?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "time_stamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        })
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), InfraError> {
        let title = title.to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                if let Some(title) = title {
                    execute_and_values(
                        r"UPDATE answers SET title = ? WHERE id = ?",
                        [title.into(), answer_id.into_inner().into()],
                        txn,
                    )
                    .await?;
                }

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }
}
