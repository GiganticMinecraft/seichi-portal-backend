use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    form::models::{
        DefaultAnswerTitle, FormDescription, FormId, FormQuestionUpdateSchema, FormTitle,
        FormUpdateTargets, OffsetAndLimit, PostedAnswers,
    },
    user::models::User,
};
use errors::infra::{InfraError, InfraError::FormNotFound};
use itertools::Itertools;
use regex::Regex;
use sea_orm::DbErr;

use crate::{
    database::{components::FormDatabase, connection::ConnectionPool},
    dto::{AnswerDto, FormDto, PostedAnswersDto, QuestionDto, SimpleFormDto},
};

#[async_trait]
impl FormDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
        user: User,
    ) -> Result<FormId, InfraError> {
        let form_id = self
            .execute_and_values(
                "INSERT INTO form_meta_data (title, description, created_by, updated_by)
                        SELECT ?, ?, users.id, users.id FROM users WHERE uuid = ?",
                [
                    title.title().to_owned().into(),
                    description.to_owned().into(),
                    user.id.to_string().into(),
                ],
            )
            .await?
            .last_insert_id() as i32;

        Ok(form_id.into())
    }

    #[tracing::instrument]
    async fn list(
        &self,
        OffsetAndLimit { offset, limit }: OffsetAndLimit,
    ) -> Result<Vec<SimpleFormDto>, InfraError> {
        let forms = self
            .query_all(
                &format!(r"SELECT form_meta_data.id AS form_id, form_meta_data.title AS form_title, description, start_at, end_at
                            FROM form_meta_data
                            LEFT JOIN response_period ON form_meta_data.id = response_period.form_id
                            ORDER BY form_meta_data.id
                            {} {}", 
                        limit.map(|value| format!("LIMIT {}", value)).unwrap_or_default(), 
                        offset.map(|value| format!("OFFSET {}", value)).unwrap_or_default()),
            )
            .await?;

        forms
            .into_iter()
            .map(|rs| {
                let form_id: i32 = rs.try_get("", "form_id")?;

                let start_at: Option<DateTime<Utc>> = rs.try_get("", "start_at")?;
                let end_at: Option<DateTime<Utc>> = rs.try_get("", "end_at")?;

                Ok(SimpleFormDto {
                    id: form_id,
                    title: rs.try_get("", "form_title")?,
                    description: rs.try_get("", "description")?,
                    response_period: start_at.zip(end_at),
                })
            })
            .collect()
    }

    #[tracing::instrument]
    async fn get(&self, form_id: FormId) -> Result<FormDto, InfraError> {
        let form_query = self
            .query_all_and_values(
                r"SELECT form_meta_data.id AS form_id, form_meta_data.title AS form_title, description, visibility, created_at, updated_at, url, start_at, end_at, default_answer_titles.title
                            FROM form_meta_data
                            LEFT JOIN form_webhooks ON form_meta_data.id = form_webhooks.form_id
                            LEFT JOIN response_period ON form_meta_data.id = response_period.form_id
                            LEFT JOIN default_answer_titles ON form_meta_data.id = default_answer_titles.form_id
                            WHERE form_meta_data.id = ?",
                [form_id.to_owned().into()]
            )
            .await?;

        let form = form_query.first().ok_or(FormNotFound {
            id: form_id.to_owned(),
        })?;

        let questions = self
            .query_all_and_values(
                r"SELECT question_id, title, description, question_type, is_required FROM form_questions WHERE form_id = ?",
                [form_id.to_owned().into()]
            )
            .await?;

        let choices = self
            .query_all(r"SELECT question_id, choice FROM form_choices")
            .await?;

        let questions = questions
            .into_iter()
            .map(|rs| {
                let question_id: i32 = rs.try_get("", "question_id")?;

                let choices = choices
                    .iter()
                    .filter_map(|rs| {
                        let choice_question_id: i32 = rs.try_get("", "question_id").ok()?;

                        if choice_question_id == question_id {
                            let choice: Result<String, _> = rs.try_get("", "choice");

                            choice.ok()
                        } else {
                            None
                        }
                    })
                    .collect_vec();

                Ok(QuestionDto {
                    id: question_id,
                    title: rs.try_get("", "title")?,
                    description: rs.try_get("", "description")?,
                    question_type: rs.try_get("", "question_type")?,
                    choices,
                    is_required: rs.try_get("", "is_required")?,
                })
            })
            .collect::<Result<Vec<_>, DbErr>>()?;

        let start_at: Option<DateTime<Utc>> = form.try_get("", "start_at")?;
        let end_at: Option<DateTime<Utc>> = form.try_get("", "end_at")?;

        Ok(FormDto {
            id: form_id.to_owned(),
            title: form.try_get("", "form_title")?,
            description: form.try_get("", "description")?,
            questions,
            metadata: (
                form.try_get("", "created_at")?,
                form.try_get("", "updated_at")?,
            ),
            response_period: start_at.zip(end_at),
            webhook_url: form.try_get("", "url")?,
            default_answer_title: form.try_get("", "default_answer_titles.title")?,
            visibility: form.try_get("", "visibility")?,
        })
    }

    #[tracing::instrument]
    async fn delete(&self, form_id: FormId) -> Result<FormId, InfraError> {
        self.execute_and_values(
            "DELETE FROM form_meta_data WHERE id = ?",
            [form_id.to_owned().into()],
        )
        .await?;

        Ok(form_id)
    }

    async fn update(
        &self,
        form_id: FormId,
        FormUpdateTargets {
            title,
            description,
            response_period,
            webhook,
            default_answer_title,
        }: FormUpdateTargets,
    ) -> Result<(), InfraError> {
        let current_form = self.get(form_id.to_owned().into()).await?;

        self.execute_and_values(
            r"UPDATE form_meta_data SET title = ?, description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            [
                title
                    .map(|title| title.into_inner())
                    .unwrap_or(current_form.title)
                    .into(),
                description
                    .map(|description| description.into_inner())
                    .unwrap_or(current_form.description)
                    .into(),
                form_id.to_owned().into(),
            ],
        )
        .await?;

        if let Some(response_period) = response_period {
            self.execute_and_values(
                "UPDATE response_period SET start_at = ?, end_at = ? WHERE form_id = ?",
                [
                    response_period.start_at.into(),
                    response_period.end_at.into(),
                    form_id.to_owned().into(),
                ],
            )
            .await?;
        }

        if current_form.webhook_url.is_some() {
            self.execute_and_values(
                "UPDATE form_webhooks SET url = ? WHERE form_id = ?",
                [
                    webhook.and_then(|url| url.webhook_url).into(),
                    form_id.to_owned().into(),
                ],
            )
            .await?;
        } else if let Some(webhook_url) = webhook.and_then(|url| url.webhook_url) {
            self.execute_and_values(
                "INSERT INTO form_webhooks (form_id, url) VALUES (?, ?)",
                [form_id.to_owned().into(), webhook_url.into()],
            )
            .await?;
        }

        if current_form.default_answer_title.is_some() && default_answer_title.is_some() {
            self.execute_and_values(
                "UPDATE default_answer_titles SET title = ?, WHERE form_id = ?",
                [
                    default_answer_title.unwrap().unwrap_or_default().into(),
                    form_id.to_owned().into(),
                ],
            )
            .await?;
        } else if let Some(default_answer_title) = default_answer_title {
            self.execute_and_values(
                "INSERT INTO default_answer_titles (form_id, title) VALUES (?, ?)",
                [
                    form_id.to_owned().into(),
                    default_answer_title.unwrap_or_default().into(),
                ],
            )
            .await?;
        }

        Ok(())
    }

    async fn post_answer(&self, answer: PostedAnswers) -> Result<(), InfraError> {
        let regex = Regex::new(r"\$\d+").unwrap();

        let default_answer_title_query_result = self
            .query_all_and_values(
                r"SELECT title FROM default_answer_titles WHERE form_id = ?",
                [answer.form_id.to_owned().into()],
            )
            .await?;

        let default_answer_title: Option<String> = default_answer_title_query_result
            .first()
            .ok_or(FormNotFound {
                id: answer.form_id.to_owned(),
            })?
            .try_get("", "title")?;

        let default_answer_title = DefaultAnswerTitle {
            default_answer_title,
        }
        .unwrap_or_default();

        let embed_title = regex.find_iter(&default_answer_title.to_owned()).fold(
            default_answer_title,
            |replaced_title, question_id| {
                let answer_opt = answer.answers.iter().find(|answer| {
                    answer.question_id.to_string() == question_id.as_str().replace('$', "")
                });
                replaced_title.replace(
                    question_id.as_str(),
                    &answer_opt
                        .map(|answer| answer.answer.to_owned())
                        .unwrap_or_default(),
                )
            },
        );

        let id = self
            .execute_and_values(
                r"INSERT INTO answers (form_id, user, title) VALUES (?, (SELECT id FROM users WHERE uuid = ?), ?)",
                [
                    answer.form_id.to_owned().into(),
                    answer.uuid.to_string().into(),
                    embed_title.into(),
                ],
            )
            .await?
            .last_insert_id();

        let params = answer
            .answers
            .into_iter()
            .flat_map(|answer| {
                vec![
                    id.to_string(),
                    answer.question_id.to_string(),
                    answer.answer,
                ]
            })
            .collect_vec();

        self.batch_insert(
            "INSERT INTO real_answers (answer_id, question_id, answer) VALUES (?, ?, ?)",
            params.iter().map(|value| value.into()),
        )
        .await?;

        Ok(())
    }

    async fn get_all_answers(&self) -> Result<Vec<PostedAnswersDto>, InfraError> {
        let answers = self
            .query_all(
                "SELECT form_id, answers.id AS answer_id, title, uuid, time_stamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        ORDER BY answers.time_stamp",
            )
            .await?;

        let real_answers = self
            .query_all("SELECT answer_id, question_id, answer FROM real_answers")
            .await?;

        answers
            .iter()
            .map(|rs| {
                let answer_id: i32 = rs.try_get("", "answer_id")?;
                let answers = real_answers
                    .iter()
                    .filter(|rs| {
                        rs.try_get::<i32>("", "answer_id")
                            .is_ok_and(|id| id == answer_id)
                    })
                    .map(|rs| {
                        Ok::<AnswerDto, DbErr>(AnswerDto {
                            question_id: rs.try_get("", "question_id")?,
                            answer: rs.try_get("", "answer")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(PostedAnswersDto {
                    uuid: rs.try_get("", "uuid")?,
                    timestamp: rs.try_get("", "time_stamp")?,
                    form_id: rs.try_get("", "form_id")?,
                    title: rs.try_get("", "title")?,
                    answers,
                })
            })
            .collect::<Result<Vec<_>, _>>()
    }

    async fn create_questions(
        &self,
        form_question_update_schema: FormQuestionUpdateSchema,
    ) -> Result<(), InfraError> {
        let last_insert_id = self.batch_insert(
            r"INSERT INTO form_questions (form_id, title, description, question_type, is_required) VALUES (?, ?, ?, ?, ?)",
            form_question_update_schema
                .questions
                .iter()
                .flat_map(|question|
                    vec![
                        form_question_update_schema.form_id.to_owned().into(),
                        question.title.to_owned().into(),
                        question.description.to_owned().into(),
                        question.question_type.to_string().into(),
                        question.is_required().to_owned().into()
                    ]
                ).collect_vec()
        )
            .await?
            .unwrap()
            .last_insert_id();

        let choices_active_values = form_question_update_schema
            .questions
            .iter()
            .rev()
            .zip((0..=last_insert_id).rev())
            .filter(|(q, _)| {
                !q.choices.is_empty() && q.question_type != domain::form::models::QuestionType::TEXT
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

        self.batch_insert(
            "INSERT INTO form_choices (question_id, choice) VALUES (?, ?)",
            choices_active_values.into_iter().map(|value| value.into()),
        )
        .await?;

        Ok(())
    }

    async fn get_questions(&self, form_id: FormId) -> Result<Vec<QuestionDto>, InfraError> {
        let questions_rs = self.query_all_and_values(
            r"SELECT question_id, title, description, question_type, is_required FROM form_questions WHERE form_id = ?",
            [form_id.to_owned().into()]
        ).await?;

        let choices_rs = self
            .query_all_and_values(
                r"SELECT form_choices.question_id, choice FROM form_choices 
            INNER JOIN form_questions ON form_choices.question_id = form_questions.question_id
            WHERE form_id = ?",
                [form_id.to_owned().into()],
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

                Ok(QuestionDto {
                    id: question_id,
                    title: question_rs.try_get("", "title")?,
                    description: question_rs.try_get("", "description")?,
                    question_type: question_rs.try_get("", "question_type")?,
                    choices,
                    is_required: question_rs.try_get("", "is_required")?,
                })
            })
            .collect::<Result<Vec<QuestionDto>, _>>()
    }
}
