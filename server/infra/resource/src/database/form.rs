use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    form::models::{
        AnswerId, Comment, CommentId, DefaultAnswerTitle, FormDescription, FormId,
        FormQuestionUpdateSchema, FormTitle, FormUpdateTargets, Label, LabelId, LabelSchema,
        OffsetAndLimit, PostedAnswersSchema, PostedAnswersUpdateSchema, ResponsePeriod,
    },
    user::models::{Role, Role::Administrator, User},
};
use errors::infra::{InfraError, InfraError::FormNotFound};
use futures::{future::try_join, try_join};
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
        AnswerDto, CommentDto, FormDto, LabelDto, PostedAnswersDto, QuestionDto, SimpleFormDto,
        UserDto,
    },
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
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let form_id = execute_and_values(
                    "INSERT INTO form_meta_data (title, description, created_by, updated_by)
                        SELECT ?, ?, users.id, users.id FROM users WHERE uuid = ?",
                    [
                        title.title().to_owned().into(),
                        description.into_inner().into(),
                        user.id.to_string().into(),
                    ],
                    txn,
                )
                .await?
                .last_insert_id() as i32;

                let insert_default_answer_title_table = execute_and_values(
                    "INSERT INTO default_answer_titles (form_id, title) VALUES (?, NULL)",
                    [form_id.into()],
                    txn,
                );

                let insert_response_period_table = execute_and_values(
                    "INSERT INTO response_period (form_id, start_at, end_at) VALUES (?, NULL, \
                     NULL)",
                    [form_id.into()],
                    txn,
                );

                try_join(
                    insert_default_answer_title_table,
                    insert_response_period_table,
                )
                .await?;

                Ok::<_, InfraError>(form_id.into())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn list(
        &self,
        OffsetAndLimit { offset, limit }: OffsetAndLimit,
    ) -> Result<Vec<SimpleFormDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let forms = query_all(
                    &format!(r"SELECT form_meta_data.id AS form_id, form_meta_data.title AS form_title, description, start_at, end_at
                            FROM form_meta_data
                            LEFT JOIN response_period ON form_meta_data.id = response_period.form_id
                            ORDER BY form_meta_data.id
                            {} {}",
                             limit.map(|value| format!("LIMIT {}", value)).unwrap_or_default(),
                             offset.map(|value| format!("OFFSET {}", value)).unwrap_or_default()),
                    txn,
                )
                    .await?;

                forms
                    .into_iter()
                    .map(|rs| {
                        let form_id: i32 = rs.try_get("", "form_id")?;

                        let start_at: Option<DateTime<Utc>> = rs.try_get("", "start_at")?;
                        let end_at: Option<DateTime<Utc>> = rs.try_get("", "end_at")?;

                        Ok::<_, InfraError>(SimpleFormDto {
                            id: form_id,
                            title: rs.try_get("", "form_title")?,
                            description: rs.try_get("", "description")?,
                            response_period: start_at.zip(end_at),
                        })
                    })
                    .collect()
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get(&self, form_id: FormId) -> Result<FormDto, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let form_query = query_all_and_values(
                    r"SELECT form_meta_data.id AS form_id, form_meta_data.title AS form_title, description, visibility, created_at, updated_at, url, start_at, end_at, default_answer_titles.title
                            FROM form_meta_data
                            LEFT JOIN form_webhooks ON form_meta_data.id = form_webhooks.form_id
                            LEFT JOIN response_period ON form_meta_data.id = response_period.form_id
                            LEFT JOIN default_answer_titles ON form_meta_data.id = default_answer_titles.form_id
                            WHERE form_meta_data.id = ?",
                    [form_id.into_inner().into()],
                    txn,
                )
                    .await?;

                let form = form_query.first().ok_or(FormNotFound {
                    id: form_id.into_inner(),
                })?;

                let questions = query_all_and_values(
                    r"SELECT question_id, title, description, question_type, is_required FROM form_questions WHERE form_id = ?",
                    [form_id.into_inner().into()],
                    txn,
                )
                    .await?;

                let choices = query_all(r"SELECT question_id, choice FROM form_choices", txn)
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

                Ok::<_, InfraError>(FormDto {
                    id: form_id.into_inner(),
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
    async fn update(
        &self,
        form_id: FormId,
        FormUpdateTargets {
            title,
            description,
            has_response_period,
            response_period,
            webhook,
            default_answer_title,
            visibility,
        }: FormUpdateTargets,
    ) -> Result<(), InfraError> {
        let current_form = self.get(form_id).await?;

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
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
                        form_id.into_inner().into(),
                    ],
                    txn,
                )
                    .await?;

                if let Some(has_response_period) = has_response_period {
                    if has_response_period && response_period.is_some() {
                        let ResponsePeriod { start_at, end_at } = response_period.unwrap();

                        execute_and_values(
                            "UPDATE response_period SET start_at = ?, end_at = ? WHERE form_id = ?",
                            [
                                start_at.or(current_form.response_period.map(|(start_at, _)| start_at)).into(),
                                end_at.or(current_form.response_period.map(|(_, end_at)| end_at)).into(),
                                form_id.into_inner().into(),
                            ],
                            txn,
                        ).await?;
                    } else {
                        execute_and_values(
                            "UPDATE response_period SET start_at = NULL, end_at = NULL WHERE form_id = ?",
                            [form_id.into_inner().into()],
                            txn,
                        )
                            .await?;
                    }
                }

                if current_form.webhook_url.is_some() && webhook.is_some() {
                    execute_and_values(
                        "UPDATE form_webhooks SET url = ? WHERE form_id = ?",
                        [
                            webhook.and_then(|url| url.webhook_url).into(),
                            form_id.into_inner().into(),
                        ],
                        txn,
                    )
                        .await?;
                } else if let Some(webhook_url) = webhook.and_then(|url| url.webhook_url) {
                    execute_and_values(
                        "INSERT INTO form_webhooks (form_id, url) VALUES (?, ?)",
                        [form_id.into_inner().into(), webhook_url.into()],
                        txn,
                    )
                        .await?;
                }

                if current_form.default_answer_title.is_some() && default_answer_title.is_some() {
                    execute_and_values(
                        "UPDATE default_answer_titles SET title = ?, WHERE form_id = ?",
                        [
                            default_answer_title.unwrap().unwrap_or_default().into(),
                            form_id.into_inner().into(),
                        ],
                        txn,
                    )
                        .await?;
                } else if let Some(default_answer_title) = default_answer_title {
                    execute_and_values(
                        "INSERT INTO default_answer_titles (form_id, title) VALUES (?, ?)",
                        [
                            form_id.into_inner().into(),
                            default_answer_title.unwrap_or_default().into(),
                        ],
                        txn,
                    )
                        .await?;
                }

                if let Some(visibility) = visibility {
                    execute_and_values(
                        "UPDATE form_meta_data SET visibility = ? WHERE id = ?",
                        [visibility.to_string().into(), form_id.into_inner().into()],
                        txn,
                    ).await?;
                }

                Ok::<_, InfraError>(())
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn post_answer(
        &self,
        user: &User,
        answer: &PostedAnswersSchema,
    ) -> Result<(), InfraError> {
        let User { id, .. } = user.to_owned();
        let form_id = answer.form_id.to_owned();
        let answers = answer.answers.to_owned();

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

                let default_answer_title = DefaultAnswerTitle {
                    default_answer_title,
                }
                    .unwrap_or_default();

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
                    r"INSERT INTO answers (form_id, user, title) VALUES (?, (SELECT id FROM users WHERE uuid = ?), ?)",
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
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_answers(
        &self,
        answer_id: AnswerId,
    ) -> Result<Option<PostedAnswersDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let fetch_real_answers = query_all_and_values(
                    "SELECT answer_id, question_id, answer FROM real_answers WHERE answer_id = ?",
                    [answer_id.into_inner().into()],
                    txn,
                );

                let fetch_answer_query_result_opt = query_one_and_values(
                    r"SELECT form_id, answers.id AS answer_id, title, uuid, name, role, time_stamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        WHERE answers.id = ?",
                    [answer_id.into_inner().into()],
                    txn,
                );

                let fetch_comments = query_all_and_values(
                    r"SELECT form_answer_comments.id AS comment_id, answer_id, content, timestamp, name, role, uuid FROM form_answer_comments
                        INNER JOIN users ON form_answer_comments.commented_by = users.id
                        WHERE answer_id = ?",
                    [answer_id.into_inner().into()],
                    txn,
                );

                let fetch_labels = query_all_and_values(
                    r"SELECT answer_id, label_id, label FROM label_settings_for_form_answers
                    INNER JOIN label_for_form_answers ON label_for_form_answers.id = label_id
                    WHERE answer_id = ?",
                    [answer_id.into_inner().into()],
                    txn,
                );

                let (answer_query_result_opt, real_answers, comments, labels) = try_join!(fetch_answer_query_result_opt, fetch_real_answers, fetch_comments, fetch_labels)?;

                let answers = real_answers
                    .iter()
                    .map(|rs| {
                        Ok::<AnswerDto, DbErr>(AnswerDto {
                            question_id: rs.try_get("", "question_id")?,
                            answer: rs.try_get("", "answer")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let comments = comments.
                    iter()
                    .map(|rs| {
                        Ok::<CommentDto, InfraError>(CommentDto {
                            comment_id: rs.try_get("", "comment_id")?,
                            content: rs.try_get("", "content")?,
                            timestamp: rs.try_get("", "timestamp")?,
                            commented_by: UserDto {
                                name: rs.try_get("", "name")?,
                                id: uuid::Uuid::from_str(&rs.try_get::<String>("", "uuid")?)?,
                                role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let labels = labels
                    .iter()
                    .map(|rs| {
                        Ok::<LabelDto, InfraError>(LabelDto {
                            id: rs.try_get("", "label_id")?,
                            name: rs.try_get("", "label")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;


                answer_query_result_opt
                    .map(|rs| {
                        Ok::<_, InfraError>(PostedAnswersDto {
                            id: answer_id.into_inner(),
                            uuid: uuid::Uuid::from_str(&rs.try_get::<String>("", "uuid")?)?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "time_stamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                            answers,
                            comments,
                            labels,
                        })
                    })
                    .transpose()
            })
        })
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_all_answers(&self) -> Result<Vec<PostedAnswersDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let fetch_answers = query_all(
                    r"SELECT form_id, answers.id AS answer_id, title, uuid, name, role, time_stamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        ORDER BY answers.time_stamp",
                    txn,
                );

                let fetch_real_answers = query_all(
                    "SELECT answer_id, question_id, answer FROM real_answers",
                    txn,
                );

                let fetch_comments = query_all(
                    r"SELECT form_answer_comments.id AS comment_id, answer_id, content, timestamp, name, role, uuid FROM form_answer_comments
                        INNER JOIN users ON form_answer_comments.commented_by = users.id",
                    txn,
                );

                let fetch_labels = query_all(
                    r"SELECT answer_id, label_id, label FROM label_settings_for_form_answers
                    INNER JOIN label_for_form_answers ON label_for_form_answers.id = label_id",
                    txn,
                );

                let (answers, real_answers, comments, labels) = try_join!(fetch_answers, fetch_real_answers, fetch_comments, fetch_labels)?;

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

                        let comments = comments
                            .iter()
                            .filter(|rs| {
                                rs.try_get::<i32>("", "answer_id").is_ok_and(|id| id == answer_id)
                            })
                            .map(|rs| {
                                Ok::<CommentDto, InfraError>(CommentDto {
                                    comment_id: rs.try_get("", "comment_id")?,
                                    content: rs.try_get("", "content")?,
                                    timestamp: rs.try_get("", "timestamp")?,
                                    commented_by: UserDto {
                                        name: rs.try_get("", "name")?,
                                        id: uuid::Uuid::from_str(&rs.try_get::<String>("", "uuid")?)?,
                                        role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                                    },
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?;

                        let labels = labels
                            .iter()
                            .filter(|rs| {
                                rs.try_get::<i32>("", "answer_id").is_ok_and(|id| id == answer_id)
                            })
                            .map(|rs| {
                                Ok::<LabelDto, InfraError>(LabelDto {
                                    id: rs.try_get("", "label_id")?,
                                    name: rs.try_get("", "label")?,
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?;

                        Ok::<_, InfraError>(PostedAnswersDto {
                            id: answer_id,
                            uuid: uuid::Uuid::from_str(&rs.try_get::<String>("", "uuid")?)?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "time_stamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                            answers,
                            comments,
                            labels,
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
        posted_answers_update_schema: &PostedAnswersUpdateSchema,
    ) -> Result<(), InfraError> {
        let title = posted_answers_update_schema.title.to_owned();

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
        form_question_update_schema: &FormQuestionUpdateSchema,
    ) -> Result<(), InfraError> {
        let form_id = form_question_update_schema.form_id.to_owned();
        let questions = form_question_update_schema.questions.to_owned();

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

    async fn put_questions(&self, questions: &FormQuestionUpdateSchema) -> Result<(), InfraError> {
        let form_id = questions.form_id.to_owned();
        let questions = questions.questions.to_owned();

        self.read_write_transaction(|txn| Box::pin(async move {
            batch_insert(
                r"INSERT INTO form_questions (question_id, form_id, title, description, question_type, is_required)
                VALUES (?, ?, ?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                title = VALUES(title),
                description = VALUES(description),
                question_type = VALUES(question_type),
                is_required = VALUES(is_required)",
                questions.iter().flat_map(|question| vec![
                    question.id.into_inner().into(),
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

            // TODO: 現在の API の仕様上、form_choices で割り当てられているidをバックエンドから送信することはないため、
            //  ON DUPLICATE KEY UPDATE を使用せずに完全に選択肢を上書きしているが、API の仕様を変更して choice_id を公開し、
            //  それを使って選択肢の更新を行うべきか検討する
            multiple_delete(
                "DELETE FROM form_choices WHERE question_id IN (?)",
                questions.iter().map(|question| question.id.into_inner().into()),
                txn,
            ).await?;

            batch_insert(
                r"INSERT INTO form_choices (question_id, choice) VALUES (?, ?)",
                choices_active_values.into_iter().map(|value| value.into()),
                txn,
            )
                .await?;

            let form_question_ids = query_all_and_values(
                r"SELECT question_id FROM form_questions WHERE form_id = ?",
                [form_id.into_inner().into()],
                txn,
            ).await?;

            let already_exists_question_ids = form_question_ids
                .into_iter()
                .map(|rs| rs.try_get::<i32>("", "question_id"))
                .collect::<Result<Vec<_>, DbErr>>()?;

            let delete_target_question_ids = already_exists_question_ids
                .into_iter()
                .filter(|question_id| {
                    !questions.iter().any(|question| question.id.into_inner() == *question_id)
                }).collect_vec();

            multiple_delete(
                r"DELETE FROM form_questions WHERE question_id IN (?)",
                delete_target_question_ids.into_iter().map(|id| id.into()),
                txn,
            ).await?;

            Ok::<_, InfraError>(())
        })).await.map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<QuestionDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let questions_rs = query_all_and_values(
                    r"SELECT question_id, title, description, question_type, is_required FROM form_questions WHERE form_id = ?",
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
                            id: question_id,
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
    async fn has_permission(&self, answer_id: AnswerId, user: &User) -> Result<bool, InfraError> {
        if user.role == Administrator {
            return Ok(true);
        }

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rs_opt = query_one_and_values(
                    r"SELECT visibility FROM form_meta_data INNER JOIN 
                                ON answers.form_id = form_meta_data.id
                                WHERE answers.id = ?",
                    [answer_id.into_inner().into()],
                    txn,
                )
                .await?;

                rs_opt
                    .map(|rs| rs.try_get::<bool>("", "visibility"))
                    .unwrap_or_else(|| Ok(false))
                    .map_err(Into::<InfraError>::into)
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn post_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), InfraError> {
        let params = [
            answer_id.into_inner().into(),
            comment.content.to_owned().into(),
            comment.commented_by.id.to_string().into(),
        ];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r"INSERT INTO form_answer_comments (answer_id, commented_by, content)
                        SELECT ?, users.id, ? FROM users WHERE uuid = ?",
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

    async fn create_label_for_answers(&self, label: &LabelSchema) -> Result<(), InfraError> {
        let params = [label.name.to_owned().into()];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "INSERT INTO label_for_form_answers (label) VALUES (?)",
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
                    query_all("SELECT id, label FROM label_for_form_answers", txn).await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(LabelDto {
                            id: rs.try_get("", "id")?,
                            name: rs.try_get("", "label")?,
                        })
                    })
                    .collect::<Result<Vec<LabelDto>, _>>()
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
                    "UPDATE label_for_form_answers SET label = ? WHERE id = ?",
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

    async fn create_label_for_forms(&self, label: &LabelSchema) -> Result<(), InfraError> {
        let params = [label.name.to_owned().into()];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "INSERT INTO label_for_forms (label) VALUES (?)",
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

    async fn get_labels_for_forms(&self) -> Result<Vec<LabelDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let labels_rs = query_all("SELECT id, label FROM label_for_forms", txn).await?;

                labels_rs
                    .into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(LabelDto {
                            id: rs.try_get("", "id")?,
                            name: rs.try_get("", "label")?,
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
                    "UPDATE label_for_forms SET label = ? WHERE id = ?",
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
}
