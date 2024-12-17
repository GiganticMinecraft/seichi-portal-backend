use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    form::{
        answer::models::{AnswerId, FormAnswer, FormAnswerContent},
        comment::models::{Comment, CommentId},
        message::models::{Message, MessageId},
        models::{
            DefaultAnswerTitle, Form, FormDescription, FormId, FormTitle, Label, LabelId,
            ResponsePeriod, Visibility, WebhookUrl,
        },
        question::models::Question,
    },
    user::models::{Role, User},
};
use errors::infra::{InfraError, InfraError::FormNotFound};
use futures::future::try_join;
use itertools::Itertools;
use regex::Regex;
use sea_orm::DbErr;

use crate::{
    database::{
        components::FormDatabase,
        connection::{
            batch_insert, execute_and_values, multiple_delete, query_all, query_all_and_values,
            query_one, query_one_and_values, ConnectionPool,
        },
    },
    dto::{
        AnswerLabelDto, CommentDto, FormAnswerContentDto, FormAnswerDto, FormDto, LabelDto,
        MessageDto, QuestionDto, UserDto,
    },
};

#[async_trait]
impl FormDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create(&self, form: &Form, user: &User) -> Result<(), InfraError> {
        let form_id = form.id().to_owned();
        let form_title = form.title().to_owned();
        let description = form.description().to_owned();
        let user_id = user.id.to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r#"INSERT INTO form_meta_data (title, description, created_by, updated_by)
                            VALUES (?, ?, ?, ?)"#,
                    [
                        form_title.into_inner().into(),
                        description.into_inner().into(),
                        user_id.to_string().into(),
                        user_id.to_string().into(),
                    ],
                    txn,
                )
                .await?;

                let insert_default_answer_title_table = execute_and_values(
                    "INSERT INTO default_answer_titles (form_id, title) VALUES (?, NULL)",
                    [form_id.to_owned().into_inner().into()],
                    txn,
                );

                let insert_response_period_table = execute_and_values(
                    "INSERT INTO response_period (form_id, start_at, end_at) VALUES (?, NULL, \
                     NULL)",
                    [form_id.to_owned().into_inner().into()],
                    txn,
                );

                try_join(
                    insert_default_answer_title_table,
                    insert_response_period_table,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<FormDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let form_query = query_all_and_values(
                    r"SELECT form_meta_data.id AS form_id, form_meta_data.title AS form_title, description, visibility, answer_visibility, created_at, updated_at, url, start_at, end_at, default_answer_titles.title
                            FROM form_meta_data
                            LEFT JOIN form_webhooks ON form_meta_data.id = form_webhooks.form_id
                            LEFT JOIN response_period ON form_meta_data.id = response_period.form_id
                            LEFT JOIN default_answer_titles ON form_meta_data.id = default_answer_titles.form_id
                            ORDER BY form_meta_data.id
                            LIMIT ? OFFSET ?",
                    [limit.unwrap_or(u32::MAX).into(), offset.unwrap_or(0).into()],
                    txn,
                )
                    .await?;

                form_query
                    .into_iter()
                    .map(|query_rs| {
                        Ok::<_, InfraError>(FormDto {
                            id: query_rs.try_get("", "form_id")?,
                            title: query_rs.try_get("", "form_title")?,
                            description: query_rs.try_get("", "description")?,
                            metadata: (
                                query_rs.try_get("", "created_at")?,
                                query_rs.try_get("", "updated_at")?,
                            ),
                            start_at: query_rs.try_get("", "start_at")?,
                            end_at: query_rs.try_get("", "end_at")?,
                            webhook_url: query_rs.try_get("", "url")?,
                            default_answer_title: query_rs.try_get("", "default_answer_titles.title")?,
                            visibility: query_rs.try_get("", "visibility")?,
                            answer_visibility: query_rs.try_get("", "answer_visibility")?,
                        })
                    }).collect::<Result<Vec<_>, _>>()
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get(&self, form_id: FormId) -> Result<Option<FormDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let form_query = query_all_and_values(
                    r"SELECT form_meta_data.id AS form_id, form_meta_data.title AS form_title, description, visibility, answer_visibility, created_at, updated_at, url, start_at, end_at, default_answer_titles.title
                            FROM form_meta_data
                            LEFT JOIN form_webhooks ON form_meta_data.id = form_webhooks.form_id
                            LEFT JOIN response_period ON form_meta_data.id = response_period.form_id
                            LEFT JOIN default_answer_titles ON form_meta_data.id = default_answer_titles.form_id
                            WHERE form_meta_data.id = ?",
                    [form_id.into_inner().into()],
                    txn,
                )
                    .await?;

                let form = match form_query.first() {
                    Some(form) => form,
                    None => return Ok(None),
                };

                Ok::<_, InfraError>(Some(FormDto {
                    id: form_id.into_inner(),
                    title: form.try_get("", "form_title")?,
                    description: form.try_get("", "description")?,
                    metadata: (
                        form.try_get("", "created_at")?,
                        form.try_get("", "updated_at")?,
                    ),
                    start_at: form.try_get("", "start_at")?,
                    end_at: form.try_get("", "end_at")?,
                    webhook_url: form.try_get("", "url")?,
                    default_answer_title: form.try_get("", "default_answer_titles.title")?,
                    visibility: form.try_get("", "visibility")?,
                    answer_visibility: form.try_get("", "answer_visibility")?,
                }))
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn delete(&self, form_id: FormId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM form_meta_data WHERE id = ?",
                    [form_id.into_inner().into()],
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
    async fn update_form_title(
        &self,
        form_id: &FormId,
        form_title: &FormTitle,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let form_title = form_title.to_owned().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r"UPDATE form_meta_data SET title = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                    [
                        form_title.into(),
                        form_id.into(),
                    ],
                    txn,
                ).await?;

                Ok::<_, InfraError>(())
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn update_form_description(
        &self,
        form_id: &FormId,
        form_description: &FormDescription,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let form_description = form_description.to_owned().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r"UPDATE form_meta_data SET description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                    [
                        form_description.to_owned().into(),
                        form_id.into(),
                    ],
                    txn,
                ).await?;

                Ok::<_, InfraError>(())
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn update_form_response_period(
        &self,
        form_id: &FormId,
        response_period: &ResponsePeriod,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let start_at = response_period.start_at().to_owned();
        let end_at = response_period.end_at().to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE response_period SET start_at = ?, end_at = ? WHERE form_id = ?",
                    [start_at.into(), end_at.into(), form_id.into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn update_form_webhook_url(
        &self,
        form_id: &FormId,
        webhook_url: &WebhookUrl,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let webhook_url = webhook_url.to_owned().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE form_webhooks SET url = ? WHERE form_id = ?",
                    [webhook_url.into(), form_id.into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn update_form_default_answer_title(
        &self,
        form_id: &FormId,
        default_answer_title: &DefaultAnswerTitle,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let default_answer_title = default_answer_title.to_owned().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE default_answer_titles SET title = ? WHERE form_id = ?",
                    [default_answer_title.into(), form_id.into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn update_form_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let visibility = visibility.to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE form_meta_data SET visibility = ? WHERE id = ?",
                    [visibility.into(), form_id.into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn update_form_answer_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), InfraError> {
        let form_id = form_id.into_inner();
        let visibility = visibility.to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE form_meta_data SET answer_visibility = ? WHERE id = ?",
                    [visibility.into(), form_id.into()],
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
    async fn post_answer(
        &self,
        user: &User,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), InfraError> {
        let User { id, .. } = user.to_owned();
        let form_id = form_id.to_owned();
        let answers = answers.to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let regex = Regex::new(r"\$\d+").unwrap();

                let default_answer_title_query_result = query_all_and_values(
                    r"SELECT title FROM default_answer_titles WHERE form_id = ?",
                    [form_id.to_owned().into_inner().into()],
                    txn,
                )
                .await?;

                let default_answer_title: Option<String> = default_answer_title_query_result
                    .first()
                    .ok_or(FormNotFound {
                        id: form_id.to_owned().into_inner(),
                    })?
                    .try_get("", "title")?;

                // FIXME: ここにドメイン知識が漏れてしまっていることで
                //   ここでのドメインエラーが正しくハンドリングできない
                let default_answer_title = DefaultAnswerTitle::try_new(default_answer_title)
                    .unwrap()
                    .to_owned()
                    .into_inner()
                    .unwrap_or("未設定".to_string());

                let embed_title = regex.find_iter(&default_answer_title.to_owned()).fold(
                    default_answer_title,
                    |replaced_title, question_id| {
                        let answer_opt = answers.iter().find(|answer| {
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

                let id = execute_and_values(
                    r"INSERT INTO answers (form_id, user, title) VALUES (?, ?, ?)",
                    [
                        form_id.to_owned().into_inner().into(),
                        id.to_owned().to_string().into(),
                        embed_title.into(),
                    ],
                    txn,
                )
                .await?
                .last_insert_id();

                let params = answers
                    .iter()
                    .flat_map(|answer| {
                        vec![
                            id.to_string(),
                            answer.question_id.to_string(),
                            answer.answer.to_owned(),
                        ]
                    })
                    .collect_vec();

                batch_insert(
                    "INSERT INTO real_answers (answer_id, question_id, answer) VALUES (?, ?, ?)",
                    params.into_iter().map(|value| value.into()),
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
                            form_id: question_rs.try_get("", "form_id")?,
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

    #[tracing::instrument]
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<CommentDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let comments = query_all_and_values(
                    r"SELECT form_answer_comments.id AS content_id, answer_id, commented_by, name, role, content, timestamp FROM form_answer_comments
                    INNER JOIN users ON form_answer_comments.commented_by = users.id
                    WHERE answer_id = ?",
                    [answer_id.into_inner().into()],
                    txn,
                ).await?;

                comments.into_iter().map(|rs| {
                    Ok::<_, InfraError>(CommentDto {
                        answer_id: rs.try_get("", "answer_id")?,
                        comment_id: rs.try_get("", "id")?,
                        content: rs.try_get("", "content")?,
                        timestamp: rs.try_get("", "time_stamp")?,
                        commented_by: UserDto {
                            name: rs.try_get("", "name")?,
                            id: rs.try_get("", "commented_by")?,
                            role: Role::from_str(rs.try_get::<String>("", "role")?.as_str())?,
                        },
                    })
                }).collect::<Result<Vec<_>, _>>()
            })
        })
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn post_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), InfraError> {
        let params = [
            answer_id.into_inner().into(),
            comment.commented_by.id.to_string().into(),
            comment.content.to_owned().into(),
        ];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r"INSERT INTO form_answer_comments (answer_id, commented_by, content)
                        VALUES (?, ?, ?)",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM form_answer_comments WHERE id = ?",
                    [comment_id.into_inner().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn create_label_for_answers(&self, label_name: String) -> Result<(), InfraError> {
        let params = [label_name.to_owned().into()];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "INSERT INTO label_for_form_answers (name) VALUES (?)",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn get_labels_for_answers(&self) -> Result<Vec<LabelDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs =
                    query_all("SELECT id, name FROM label_for_form_answers", txn).await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(LabelDto {
                            id: rs.try_get("", "id")?,
                            name: rs.try_get("", "name")?,
                        })
                    })
                    .collect::<Result<Vec<LabelDto>, _>>()
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerLabelDto>, InfraError> {
        let answer_id = answer_id.into_inner();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs = query_all_and_values(
                    r"SELECT label_for_form_answers.id AS label_id, name FROM label_for_form_answers
                    INNER JOIN label_settings_for_form_answers ON label_for_form_answers.id = label_settings_for_form_answers.label_id
                    WHERE answer_id = ?",
                    [answer_id.into()],
                    txn,
                )
                    .await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(AnswerLabelDto {
                            id: rs.try_get("", "label_id")?,
                            answer_id,
                            name: rs.try_get("", "name")?,
                        })
                    })
                    .collect::<Result<Vec<AnswerLabelDto>, _>>()
            })
        })
            .await
            .map_err(Into::into)
    }

    async fn delete_label_for_answers(&self, label_id: LabelId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM label_for_form_answers WHERE id = ?",
                    [label_id.to_string().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn edit_label_for_answers(&self, label: &Label) -> Result<(), InfraError> {
        let params = [label.name.to_owned().into(), label.id.to_string().into()];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE label_for_form_answers SET name = ? WHERE id = ?",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                multiple_delete(
                    "DELETE FROM label_settings_for_form_answers WHERE answer_id = ?",
                    vec![answer_id.into_inner().into()],
                    txn,
                )
                .await?;

                let params = label_ids
                    .into_iter()
                    .flat_map(|label_id| [answer_id.into_inner(), label_id.into_inner()])
                    .collect_vec();

                batch_insert(
                    "INSERT INTO label_settings_for_form_answers (answer_id, label_id) VALUES (?, \
                     ?)",
                    params.into_iter().map(|value| value.into()),
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn create_label_for_forms(&self, label_name: String) -> Result<(), InfraError> {
        let params = [label_name.to_owned().into()];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values("INSERT INTO label_for_forms (name) VALUES (?)", params, txn)
                    .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn get_labels_for_forms(&self) -> Result<Vec<LabelDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs = query_all("SELECT id, name FROM label_for_forms", txn).await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(LabelDto {
                            id: rs.try_get("", "id")?,
                            name: rs.try_get("", "name")?,
                        })
                    })
                    .collect::<Result<Vec<LabelDto>, _>>()
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn delete_label_for_forms(&self, label_id: LabelId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM label_for_forms WHERE id = ?",
                    [label_id.to_string().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn edit_label_for_forms(&self, label: &Label) -> Result<(), InfraError> {
        let params = [label.name.to_owned().into(), label.id.to_string().into()];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE label_for_forms SET name = ? WHERE id = ?",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                multiple_delete(
                    "DELETE FROM label_settings_for_forms WHERE form_id = ?",
                    vec![form_id.into_inner().into()],
                    txn,
                )
                .await?;

                let params = label_ids
                    .into_iter()
                    .flat_map(|label_id| {
                        [form_id.into_inner().into(), label_id.into_inner().into()]
                    })
                    .collect_vec();

                batch_insert(
                    "INSERT INTO label_settings_for_forms (form_id, label_id) VALUES (?, ?)",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn post_message(&self, message: &Message) -> Result<(), InfraError> {
        let id = message.id().to_string().to_owned();
        let related_answer_id = message.related_answer().id.into_inner().to_owned();
        let sender = message.sender().id.to_string().to_owned();
        let body = message.body().to_owned();
        let timestamp = message.timestamp().to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(r"INSERT INTO messages (id, related_answer_id, sender, body, timestamp) VALUES (?, ?, ?, ?, ?)", [
                    id.into(),
                    related_answer_id.into(),
                    sender.into(),
                    body.into(),
                    timestamp.into(),
                ], txn).await?;

                Ok::<_, InfraError>(())
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_message_body(
        &self,
        message_id: MessageId,
        body: String,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE messages SET body = ? WHERE id = ?",
                    [body.into(), message_id.into_inner().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_messages_by_form_answer(
        &self,
        answers: &FormAnswer,
    ) -> Result<Vec<MessageDto>, InfraError> {
        let answer_id = answers.id.into_inner().to_owned();

        Ok(self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let rs = query_all_and_values(
                        r"SELECT messages.id AS message_id, sender, name, role, body, timestamp FROM messages
                    INNER JOIN users ON users.id = messages.sender
                    WHERE related_answer_id = ?",
                        [answer_id.into()],
                        txn,
                    )
                        .await?;

                    Ok::<_, InfraError>(
                        rs.into_iter()
                            .map(|rs| {
                                let user = Ok::<_, InfraError>(UserDto {
                                    name: rs.try_get("", "name")?,
                                    id: uuid::Uuid::from_str(&rs.try_get::<String>("", "sender")?)?,
                                    role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                                });

                                Ok::<_, InfraError>((
                                    user?,
                                    uuid::Uuid::from_str(&rs.try_get::<String>("", "message_id")?)?,
                                    rs.try_get::<String>("", "body")?,
                                    rs.try_get::<DateTime<Utc>>("", "timestamp")?,
                                ))
                            })
                            .collect_vec(),
                    )
                })
            })
            .await?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(user, message_id, body, timestamp)| MessageDto {
                id: message_id,
                related_answer: FormAnswerDto {
                    id: answers.id.into_inner().to_owned(),
                    user_name: answers.user.name.to_owned(),
                    uuid: answers.user.id,
                    user_role: answers.user.role.to_owned(),
                    timestamp: answers.timestamp,
                    form_id: answers.form_id.into_inner().to_owned(),
                    title: answers.title.to_owned(),
                },
                sender: user,
                body,
                timestamp,
            })
            .collect_vec())
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_message(
        &self,
        message_id: &MessageId,
    ) -> Result<Option<MessageDto>, InfraError> {
        let message_id = message_id.into_inner();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rs = query_one_and_values(
                    r"SELECT sender, message_senders.name, message_senders.role, body, timestamp,
                    answers.id AS answer_id,
                    time_stamp,
                    form_id,
                    user AS respondent_id,
                    respondents.name AS respondent_name,
                    respondents.role AS respondent_role
                    FROM messages
                    INNER JOIN answers ON related_answer_id = answers.id
                    INNER JOIN users AS message_senders ON message_senders.id = messages.sender
                    INNER JOIN users AS respondents ON respondents.id = answers.user
                    WHERE messages.id = ?",
                    [message_id.to_string().into()],
                    txn,
                )
                .await?;

                rs.map(|rs| {
                    let user = Ok::<_, InfraError>(UserDto {
                        name: rs.try_get("", "name")?,
                        id: uuid::Uuid::from_str(&rs.try_get::<String>("", "sender")?)?,
                        role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                    })?;

                    let related_answer = Ok::<_, InfraError>(FormAnswerDto {
                        id: rs.try_get("", "answer_id")?,
                        user_name: rs.try_get("", "respondent_name")?,
                        uuid: uuid::Uuid::from_str(&rs.try_get::<String>("", "respondent_id")?)?,
                        user_role: Role::from_str(&rs.try_get::<String>("", "respondent_role")?)?,
                        timestamp: rs.try_get("", "time_stamp")?,
                        form_id: rs.try_get("", "form_id")?,
                        title: rs.try_get("", "title")?,
                    })?;

                    Ok::<_, InfraError>(MessageDto {
                        id: message_id.to_owned(),
                        related_answer,
                        sender: user,
                        body: rs.try_get("", "body")?,
                        timestamp: rs.try_get("", "timestamp")?,
                    })
                })
                .transpose()
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn delete_message(&self, message_id: MessageId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM messages WHERE id = ?",
                    [message_id.to_string().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }
}
