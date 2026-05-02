use async_trait::async_trait;
use domain::form::{models::FormId, question::models::Question};
use errors::infra::InfraError;
use itertools::Itertools;
use sqlx::{Row, query};

use crate::{
    database::{
        components::FormQuestionDatabase,
        connection::{ConnectionPool, DbErr, query_all_and_values},
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
        let questions = questions.to_owned();
        let form_id = form_id.into_inner().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                if !questions.is_empty() {
                    let sql = format!(
                        "INSERT INTO form_questions (form_id, title, description, question_type, is_required) VALUES {}",
                        std::iter::repeat_n("(?, ?, ?, ?, ?)", questions.len()).join(", ")
                    );

                    questions
                        .iter()
                        .fold(query(&sql), |query, question| {
                            query
                                .bind(form_id.clone())
                                .bind(question.title.clone())
                                .bind(question.description.clone())
                                .bind(question.question_type.to_string())
                                .bind(*question.is_required())
                        })
                        .execute(&mut **txn)
                        .await?;

                    let last_insert_id = sqlx::query_scalar!(
                        "SELECT question_id AS `question_id!: i32` FROM form_questions ORDER BY question_id DESC LIMIT 1"
                    )
                    .fetch_one(&mut **txn)
                    .await?;

                    let choices_active_values = questions
                        .iter()
                        .rev()
                        .zip((1..=last_insert_id).rev())
                        .filter(|(q, _)| {
                            !q.choices.is_empty()
                                && q.question_type
                                    != domain::form::question::models::QuestionType::TEXT
                        })
                        .flat_map(|(question, question_id)| {
                            question
                                .choices
                                .iter()
                                .cloned()
                                .map(move |choice| (question_id.to_string(), choice))
                        })
                        .collect_vec();

                    if !choices_active_values.is_empty() {
                        let sql = format!(
                            "INSERT INTO form_choices (question_id, choice) VALUES {}",
                            std::iter::repeat_n("(?, ?)", choices_active_values.len()).join(", ")
                        );

                        choices_active_values
                            .into_iter()
                            .flat_map(|(question_id, choice)| [question_id, choice])
                            .fold(query(&sql), |query, value| query.bind(value))
                            .execute(&mut **txn)
                            .await?;
                    }
                }

                Ok::<_, InfraError>(())
            })
        }).await
    }

    #[tracing::instrument]
    async fn put_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let current_form_question_ids = query_all_and_values(
                    r"SELECT question_id FROM form_questions WHERE form_id = ?",
                    [form_id.clone().into()],
                    txn,
                )
                .await?
                .into_iter()
                .map(|rs| rs.try_get::<i32, _>("question_id"))
                .collect::<Result<Vec<_>, DbErr>>()?;

                let delete_target_question_ids = current_form_question_ids
                    .into_iter()
                    .filter(|question_id| {
                        !questions
                            .iter()
                            .any(|question| question.id.map(|id| id.into_inner()) == Some(*question_id))
                    })
                    .collect_vec();

                if !delete_target_question_ids.is_empty() {
                    let sql = format!(
                        "DELETE FROM form_questions WHERE question_id IN ({})",
                        std::iter::repeat_n("?", delete_target_question_ids.len()).join(", ")
                    );

                    delete_target_question_ids
                        .iter()
                        .fold(query(&sql), |query, question_id| query.bind(question_id))
                        .execute(&mut **txn)
                        .await?;
                }

                if !questions.is_empty() {
                    let sql = format!(
                        r"INSERT INTO form_questions (question_id, form_id, title, description, question_type, is_required)
                VALUES {}
                ON DUPLICATE KEY UPDATE
                title = VALUES(title),
                description = VALUES(description),
                question_type = VALUES(question_type),
                is_required = VALUES(is_required)",
                        std::iter::repeat_n("(?, ?, ?, ?, ?, ?)", questions.len()).join(", ")
                    );

                    questions
                        .iter()
                        .fold(query(&sql), |query, question| {
                            query
                                .bind(question.id.map(|id| id.into_inner()))
                                .bind(form_id.clone())
                                .bind(question.title.clone())
                                .bind(question.description.clone())
                                .bind(question.question_type.to_string())
                                .bind(*question.is_required())
                        })
                        .execute(&mut **txn)
                        .await?;

                    let last_insert_id = sqlx::query_scalar!(
                        "SELECT question_id AS `question_id!: i32` FROM form_questions ORDER BY question_id DESC LIMIT 1"
                    )
                    .fetch_one(&mut **txn)
                    .await?;

                    let choices_active_values = questions
                        .iter()
                        .rev()
                        .zip((1..=last_insert_id).rev())
                        .filter(|(q, _)| {
                            !q.choices.is_empty()
                                && q.question_type
                                    != domain::form::question::models::QuestionType::TEXT
                        })
                        .flat_map(|(question, question_id)| {
                            question
                                .choices
                                .iter()
                                .cloned()
                                .map(move |choice| (question_id.to_string(), choice))
                        })
                        .collect_vec();

                    let current_question_ids = questions
                        .iter()
                        .filter_map(|question| question.id.map(|id| id.into_inner()))
                        .collect_vec();

                    // TODO: 現在の API の仕様上、form_choices で割り当てられているidをバックエンドから送信することはないため、
                    //  ON DUPLICATE KEY UPDATE を使用せずに完全に選択肢を上書きしているが、API の仕様を変更して choice_id を公開し、
                    //  それを使って選択肢の更新を行うべきか検討する
                    if !current_question_ids.is_empty() {
                        let sql = format!(
                            "DELETE FROM form_choices WHERE question_id IN ({})",
                            std::iter::repeat_n("?", current_question_ids.len()).join(", ")
                        );

                        current_question_ids
                            .iter()
                            .fold(query(&sql), |query, question_id| query.bind(question_id))
                            .execute(&mut **txn)
                            .await?;
                    }

                    if !choices_active_values.is_empty() {
                        let sql = format!(
                            "INSERT INTO form_choices (question_id, choice) VALUES {}",
                            std::iter::repeat_n("(?, ?)", choices_active_values.len()).join(", ")
                        );

                        choices_active_values
                            .into_iter()
                            .flat_map(|(question_id, choice)| [question_id, choice])
                            .fold(query(&sql), |query, value| query.bind(value))
                            .execute(&mut **txn)
                            .await?;
                    }
                }

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<QuestionDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let questions_rs = query_all_and_values(
                    r"SELECT question_id, form_id, title, description, question_type, is_required FROM form_questions WHERE form_id = ?",
                    [form_id.into_inner().to_string().into()],
                    txn,
                ).await?;

                let choices_rs = query_all_and_values(
                    r"SELECT form_choices.question_id, choice FROM form_choices
                                INNER JOIN form_questions ON form_choices.question_id = form_questions.question_id
                                WHERE form_id = ?",
                    [form_id.into_inner().to_string().into()],
                    txn,
                )
                    .await?;

                questions_rs
                    .into_iter()
                    .map(|question_rs| {
                        let question_id: i32 = question_rs.try_get("question_id")?;

                        let choices = choices_rs
                            .iter()
                            .filter_map(|choice_rs| {
                                if choice_rs
                                    .try_get::<i32, _>("question_id")
                                    .is_ok_and(|id| id == question_id)
                                {
                                    choice_rs.try_get::<String, _>("choice").ok()
                                } else {
                                    None
                                }
                            })
                            .collect_vec();

                        Ok::<_, InfraError>(QuestionDto {
                            id: Some(question_id),
                            form_id: question_rs.try_get("form_id")?,
                            title: question_rs.try_get("title")?,
                            description: question_rs.try_get("description")?,
                            question_type: question_rs.try_get("question_type")?,
                            choices,
                            is_required: question_rs.try_get("is_required")?,
                        })
                    })
                    .collect::<Result<Vec<QuestionDto>, _>>()
            })
        }).await
    }
}
