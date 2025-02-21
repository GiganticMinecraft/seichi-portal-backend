use std::str::FromStr;

use async_trait::async_trait;
use domain::form::{models::FormId, question::models::Question};
use errors::infra::InfraError;
use itertools::Itertools;
use sea_orm::DbErr;
use uuid::Uuid;

use crate::{
    database::{
        components::FormQuestionDatabase,
        connection::{
            ConnectionPool, batch_insert, multiple_delete, query_all_and_values, query_one,
        },
    },
    dto::QuestionDto,
};

#[async_trait]
impl FormQuestionDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), InfraError> {
        let form_id = form_id.to_owned();
        let questions = questions.to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                batch_insert(
                    r"INSERT INTO form_questions (form_id, title, description, question_type, is_required) VALUES (?, ?, ?, ?, ?)",
                    questions
                        .clone()
                        .into_iter()
                        .flat_map(|question|
                            vec![
                                form_id.into_inner().into(),
                                question.title.clone().into(),
                                question.description.clone().into(),
                                question.question_type.to_string().into(),
                                (*question.is_required()).into()
                            ]
                        ).collect_vec(),
                    txn,
                ).await?;

                let last_insert_id = query_one(
                    "SELECT question_id FROM form_questions ORDER BY question_id DESC LIMIT 1",
                    txn,
                )
                    .await?
                    .unwrap()
                    .try_get("", "question_id")?;

                let choices_active_values = questions
                    .iter()
                    .rev()
                    .zip((1..=last_insert_id).rev())
                    .filter(|(q, _)| {
                        !q.choices.is_empty() && q.question_type != domain::form::question::models::QuestionType::TEXT
                    })
                    .flat_map(|(question, question_id)| {
                        question
                            .choices
                            .iter()
                            .cloned()
                            .flat_map(|choice| vec![question_id.to_string(), choice])
                            .collect_vec()
                    })
                    .collect_vec();

                batch_insert(
                    "INSERT INTO form_choices (question_id, choice) VALUES (?, ?)",
                    choices_active_values.into_iter().map(|value| value.into()),
                    txn,
                )
                    .await?;

                Ok::<_, InfraError>(())
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn put_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| Box::pin(async move {
            let current_form_question_ids = query_all_and_values(
                r"SELECT question_id FROM form_questions WHERE form_id = ?",
                [form_id.into_inner().into()],
                txn,
            ).await?
                .into_iter()
                .map(|rs| rs.try_get::<i32>("", "question_id"))
                .collect::<Result<Vec<_>, DbErr>>()?;

            let delete_target_question_ids = current_form_question_ids
                .into_iter()
                .filter(|question_id| {
                    !questions.iter().any(|question| question.id.map(|id| id.into_inner()) == Some(*question_id))
                }).collect_vec();

            multiple_delete(
                r"DELETE FROM form_questions WHERE question_id IN (?)",
                delete_target_question_ids.into_iter().map(|id| id.into()),
                txn,
            ).await?;

            batch_insert(
                r"INSERT INTO form_questions (question_id, form_id, title, description, question_type, is_required)
                VALUES (?, ?, ?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                title = VALUES(title),
                description = VALUES(description),
                question_type = VALUES(question_type),
                is_required = VALUES(is_required)",
                questions.iter().flat_map(|question| vec![
                    question.id.map(|id| id.into_inner()).into(),
                    form_id.into_inner().into(),
                    question.title.clone().into(),
                    question.description.clone().into(),
                    question.question_type.to_string().into(),
                    question.is_required.to_owned().into()]),
                txn,
            ).await?;

            let last_insert_id = query_one(
                "SELECT question_id FROM form_questions ORDER BY question_id DESC LIMIT 1",
                txn,
            )
                .await?
                .unwrap()
                .try_get("", "question_id")?;

            let choices_active_values = questions
                .iter()
                .rev()
                .zip((1..=last_insert_id).rev())
                .filter(|(q, _)| {
                    !q.choices.is_empty() && q.question_type != domain::form::question::models::QuestionType::TEXT
                })
                .flat_map(|(question, question_id)| {
                    question
                        .choices
                        .iter()
                        .cloned()
                        .flat_map(|choice| vec![question_id.to_string(), choice])
                        .collect_vec()
                })
                .collect_vec();

            // TODO: 現在の API の仕様上、form_choices で割り当てられているidをバックエンドから送信することはないため、
            //  ON DUPLICATE KEY UPDATE を使用せずに完全に選択肢を上書きしているが、API の仕様を変更して choice_id を公開し、
            //  それを使って選択肢の更新を行うべきか検討する
            multiple_delete(
                "DELETE FROM form_choices WHERE question_id IN (?)",
                questions.iter().map(|question| question.id.map(|id| id.into_inner()).into()),
                txn,
            ).await?;

            batch_insert(
                r"INSERT INTO form_choices (question_id, choice) VALUES (?, ?)",
                choices_active_values.into_iter().map(|value| value.into()),
                txn,
            )
                .await?;

            Ok::<_, InfraError>(())
        })).await.map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<QuestionDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let questions_rs = query_all_and_values(
                    r"SELECT question_id, form_id, title, description, question_type, is_required FROM form_questions WHERE form_id = ?",
                    [form_id.into_inner().into()],
                    txn,
                ).await?;

                let choices_rs = query_all_and_values(
                    r"SELECT form_choices.question_id, choice FROM form_choices
                                INNER JOIN form_questions ON form_choices.question_id = form_questions.question_id
                                WHERE form_id = ?",
                    [form_id.into_inner().into()],
                    txn,
                )
                    .await?;

                questions_rs
                    .into_iter()
                    .map(|question_rs| {
                        let question_id: i32 = question_rs.try_get("", "question_id")?;

                        let choices = choices_rs
                            .iter()
                            .filter_map(|choice_rs| {
                                if choice_rs
                                    .try_get::<i32>("", "question_id")
                                    .is_ok_and(|id| id == question_id)
                                {
                                    choice_rs.try_get::<String>("", "choice").ok()
                                } else {
                                    None
                                }
                            })
                            .collect_vec();

                        Ok::<_, InfraError>(QuestionDto {
                            id: Some(question_id),
                            form_id: Uuid::from_str(question_rs.try_get::<String>("", "form_id")?.as_str())?,
                            title: question_rs.try_get("", "title")?,
                            description: question_rs.try_get("", "description")?,
                            question_type: question_rs.try_get("", "question_type")?,
                            choices,
                            is_required: question_rs.try_get("", "is_required")?,
                        })
                    })
                    .collect::<Result<Vec<QuestionDto>, _>>()
            })
        }).await
            .map_err(Into::into)
    }
}
