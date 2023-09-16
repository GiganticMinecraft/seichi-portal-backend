use async_trait::async_trait;
use chrono::Utc;
use domain::form::models::{
    DefaultAnswerTitle, FormDescription, FormId, FormQuestionUpdateSchema, FormTitle,
    FormUpdateTargets, OffsetAndLimit, PostedAnswers,
};
use entities::{
    answers, default_answer_titles, form_choices, form_meta_data, form_questions, form_webhooks,
    prelude::{
        DefaultAnswerTitles, FormChoices, FormMetaData, FormQuestions, FormWebhooks, RealAnswers,
    },
    real_answers, response_period,
    sea_orm_active_enums::QuestionType,
};
use errors::infra::{InfraError, InfraError::FormNotFound};
use futures::{stream, stream::StreamExt};
use itertools::Itertools;
use num_traits::cast::FromPrimitive;
use regex::Regex;
use sea_orm::{
    sea_query::{Expr, SimpleExpr},
    ActiveEnum, ActiveModelTrait, ActiveValue,
    ActiveValue::Set,
    ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder, QuerySelect,
};

use crate::{
    database::{components::FormDatabase, connection::ConnectionPool},
    dto::{FormDto, QuestionDto},
};

#[async_trait]
impl FormDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
    ) -> Result<FormId, InfraError> {
        let form_id = form_meta_data::ActiveModel {
            id: ActiveValue::NotSet,
            title: Set(title.title().to_owned()),
            description: Set(description.to_owned()),
            created_at: Default::default(),
            updated_at: Default::default(),
        }
        .insert(&self.pool)
        .await?
        .id;

        Ok(form_id.into())
    }

    #[tracing::instrument]
    async fn list(
        &self,
        OffsetAndLimit { offset, limit }: OffsetAndLimit,
    ) -> Result<Vec<FormDto>, InfraError> {
        let forms = FormMetaData::find()
            .order_by_asc(form_meta_data::Column::Id)
            .offset(offset.and_then(u64::from_i32))
            .limit(limit.and_then(u64::from_i32))
            .all(&self.pool)
            .await?;

        let form_ids = forms.iter().map(|form| form.id).collect_vec();

        let all_questions = FormQuestions::find()
            .filter(Expr::col(form_questions::Column::FormId).is_in(form_ids.to_owned()))
            .all(&self.pool)
            .await?;

        let question_ids = all_questions
            .iter()
            .map(|question| question.question_id)
            .collect_vec();

        let all_choices = FormChoices::find()
            .filter(Expr::col(form_choices::Column::QuestionId).is_in(question_ids))
            .all(&self.pool)
            .await?;

        let all_periods = entities::response_period::Entity::find()
            .filter(Expr::col(response_period::Column::FormId).is_in(form_ids.to_owned()))
            .all(&self.pool)
            .await?;

        let all_webhook_urls = FormWebhooks::find()
            .filter(Expr::col(form_webhooks::Column::FormId).is_in(form_ids.to_owned()))
            .all(&self.pool)
            .await?;

        let all_default_answer_title = DefaultAnswerTitles::find()
            .filter(Expr::col(default_answer_titles::Column::FormId).is_in(form_ids.to_owned()))
            .all(&self.pool)
            .await?;

        Ok(forms
            .into_iter()
            .map(|form| {
                let questions = all_questions
                    .iter()
                    .filter(|question| question.form_id == form.id)
                    .map(|question| {
                        let choices = all_choices
                            .iter()
                            .filter(|choice| choice.question_id == question.question_id)
                            .cloned()
                            .map(|choice| choice.choice)
                            .collect_vec();

                        QuestionDto {
                            id: question.question_id.to_owned(),
                            title: question.title.to_owned(),
                            description: question.description.to_owned(),
                            question_type: question.question_type.to_string(),
                            choices,
                            is_required: question.is_required != 0,
                        }
                    })
                    .collect::<Vec<_>>();

                let response_period = all_periods
                    .iter()
                    .filter(|period| period.form_id == form.id)
                    .map(|period| {
                        Some((
                            period.start_at.to_owned().and_utc(),
                            period.end_at.to_owned().and_utc(),
                        ))
                    })
                    .next()
                    .unwrap_or_default();

                let webhook_url = all_webhook_urls
                    .iter()
                    .filter(|webhook_url| webhook_url.form_id == form.id)
                    .map(|webhook_url| Some(webhook_url.url.to_owned()))
                    .next()
                    .unwrap_or_default();

                let default_answer_title = all_default_answer_title
                    .iter()
                    .filter(|default_answer_title| default_answer_title.form_id == form.id)
                    .map(|default_answer_title| default_answer_title.title.to_owned())
                    .next();

                FormDto {
                    id: form.id,
                    title: form.title,
                    description: form.description,
                    questions,
                    metadata: (form.created_at, form.updated_at),
                    response_period,
                    webhook_url,
                    default_answer_title,
                }
            })
            .collect())
    }

    #[tracing::instrument]
    async fn get(&self, form_id: FormId) -> Result<FormDto, InfraError> {
        let target_form = FormMetaData::find()
            .filter(Expr::col(form_meta_data::Column::Id).eq(form_id.to_owned()))
            .all(&self.pool)
            .await?
            .first()
            .ok_or(FormNotFound {
                id: form_id.to_owned(),
            })?
            .to_owned();

        let form_questions = stream::iter(
            FormQuestions::find()
                .filter(Expr::col(form_questions::Column::FormId).eq(form_id.to_owned()))
                .all(&self.pool)
                .await?,
        )
        .then(move |question| async move {
            let choices = FormChoices::find()
                .filter(Expr::col(form_choices::Column::QuestionId).eq(question.question_id))
                .all(&self.pool)
                .await?
                .into_iter()
                .map(|choice| choice.choice)
                .collect_vec();

            Ok::<QuestionDto, InfraError>(QuestionDto {
                id: question.question_id.to_owned(),
                title: question.title.to_owned(),
                description: question.description.to_owned(),
                question_type: question.question_type.to_string(),
                choices,
                is_required: question.is_required != 0,
            })
        })
        .collect::<Vec<Result<QuestionDto, _>>>()
        .await
        .into_iter()
        .collect::<Result<Vec<QuestionDto>, _>>()?;

        let response_period = entities::response_period::Entity::find()
            .filter(Expr::col(response_period::Column::FormId).eq(target_form.id))
            .all(&self.pool)
            .await?
            .first()
            .map(|period| {
                Some((
                    period.start_at.to_owned().and_utc(),
                    period.end_at.to_owned().and_utc(),
                ))
            })
            .unwrap_or_default();

        let webhook_url = FormWebhooks::find()
            .filter(Expr::col(form_webhooks::Column::FormId).eq(target_form.id))
            .all(&self.pool)
            .await?
            .first()
            .map(|webhook_url_model| Some(webhook_url_model.url.to_owned()))
            .unwrap_or_default();

        let default_answer_title = DefaultAnswerTitles::find()
            .filter(Expr::col(default_answer_titles::Column::FormId).eq(target_form.id))
            .all(&self.pool)
            .await?
            .first()
            .map(|answer_title_setting| answer_title_setting.title.to_owned());

        Ok(FormDto {
            id: target_form.id,
            title: target_form.title,
            description: target_form.description,
            questions: form_questions,
            metadata: (target_form.created_at, target_form.updated_at),
            response_period,
            webhook_url,
            default_answer_title,
        })
    }

    #[tracing::instrument]
    async fn delete(&self, form_id: FormId) -> Result<FormId, InfraError> {
        let target_form = FormMetaData::find_by_id(form_id.to_owned())
            .all(&self.pool)
            .await?
            .first()
            .ok_or(FormNotFound {
                id: form_id.to_owned(),
            })?
            .to_owned();

        let question_ids = FormQuestions::find()
            .filter(Expr::col(form_questions::Column::FormId).eq(form_id.to_owned()))
            .all(&self.pool)
            .await?
            .iter()
            .map(|question| question.question_id)
            .collect_vec();

        FormChoices::delete_many()
            .filter(Expr::col(form_choices::Column::QuestionId).is_in(question_ids))
            .exec(&self.pool)
            .await?;

        response_period::Entity::delete_many()
            .filter(Expr::col(response_period::Column::FormId).eq(form_id.to_owned()))
            .exec(&self.pool)
            .await?;

        FormQuestions::delete_many()
            .filter(Expr::col(form_questions::Column::FormId).eq(form_id.to_owned()))
            .exec(&self.pool)
            .await?;

        target_form.delete(&self.pool).await?;

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

        FormMetaData::update_many()
            .filter(form_meta_data::Column::Id.eq(form_id.to_owned()))
            .col_expr(
                form_meta_data::Column::Title,
                Expr::value(
                    title
                        .map(|title| title.into_inner())
                        .unwrap_or(current_form.title),
                ),
            )
            .col_expr(
                form_meta_data::Column::Description,
                Expr::value(
                    description
                        .map(|description| description.into_inner())
                        .unwrap_or(current_form.description),
                ),
            )
            .col_expr(
                form_meta_data::Column::UpdatedAt,
                SimpleExpr::from(Expr::current_timestamp()),
            )
            .exec(&self.pool)
            .await?;

        if let Some(response_period) = response_period {
            response_period::Entity::update_many()
                .filter(response_period::Column::FormId.eq(form_id.to_owned()))
                .col_expr(
                    response_period::Column::StartAt,
                    Expr::value(response_period.start_at),
                )
                .col_expr(
                    response_period::Column::EndAt,
                    Expr::value(response_period.end_at),
                )
                .exec(&self.pool)
                .await?;
        }

        if current_form.webhook_url.is_some() {
            FormWebhooks::update_many()
                .filter(form_webhooks::Column::FormId.eq(form_id.to_owned()))
                .col_expr(
                    form_webhooks::Column::Url,
                    Expr::value(webhook.and_then(|url| url.webhook_url)),
                )
                .exec(&self.pool)
                .await?;
        } else if let Some(webhook_url) = webhook.and_then(|url| url.webhook_url) {
            form_webhooks::ActiveModel {
                id: ActiveValue::NotSet,
                form_id: Set(form_id.to_owned()),
                url: Set(webhook_url),
            }
            .insert(&self.pool)
            .await?;
        }

        if current_form.default_answer_title.is_some() && default_answer_title.is_some() {
            DefaultAnswerTitles::update_many()
                .filter(default_answer_titles::Column::FormId.eq(form_id.to_owned()))
                .col_expr(
                    default_answer_titles::Column::Title,
                    Expr::value(default_answer_title.unwrap().unwrap_or_default()),
                )
                .exec(&self.pool)
                .await?;
        } else if let Some(default_answer_title) = default_answer_title {
            default_answer_titles::ActiveModel {
                id: ActiveValue::NotSet,
                form_id: Set(form_id.to_owned()),
                title: Set(default_answer_title.unwrap_or_default()),
            }
            .insert(&self.pool)
            .await?;
        }

        Ok(())
    }

    async fn post_answer(&self, answer: PostedAnswers) -> Result<(), InfraError> {
        let regex = Regex::new(r"\$\d+").unwrap();

        let default_answer_title = DefaultAnswerTitle {
            default_answer_title: DefaultAnswerTitles::find()
                .filter(
                    Expr::col(default_answer_titles::Column::FormId).eq(answer.form_id.to_owned()),
                )
                .all(&self.pool)
                .await?
                .first()
                .map(|answer_title_setting| answer_title_setting.title.to_owned()),
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

        let id = answers::ActiveModel {
            id: Default::default(),
            user: Set(answer.uuid.to_owned().as_ref().to_vec()),
            title: Set(embed_title),
            time_stamp: Set(Utc::now()),
        }
        .insert(&self.pool)
        .await?
        .id;

        let real_answer_models = answer
            .answers
            .iter()
            .map(|answer| real_answers::ActiveModel {
                id: Default::default(),
                answer_id: Set(id),
                question_id: Set(answer.question_id.to_owned()),
                answer: Set(answer.answer.to_owned()),
            })
            .collect_vec();

        RealAnswers::insert_many(real_answer_models)
            .exec(&self.pool)
            .await?;

        Ok(())
    }

    async fn create_questions(
        &self,
        form_question_update_schema: FormQuestionUpdateSchema,
    ) -> Result<(), InfraError> {
        let question_active_values = form_question_update_schema
            .questions
            .iter()
            .map(|question| {
                QuestionType::try_from_value(&question.question_type.to_string().to_lowercase())
                    .map(|question_type| form_questions::ActiveModel {
                        question_id: ActiveValue::NotSet,
                        form_id: Set(form_question_update_schema.form_id.to_owned()),
                        title: Set(question.title.to_owned()),
                        description: Set(question.description.to_owned()),
                        question_type: Set(question_type),
                        is_required: Set(i8::from(question.is_required().to_owned())),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let last_insert_id = FormQuestions::insert_many(question_active_values)
            .exec(&self.pool)
            .await?
            .last_insert_id;

        let choices_active_values = form_question_update_schema
            .questions
            .iter()
            .rev()
            .zip((0..=last_insert_id).rev())
            .filter(|(q, _)| !q.choices.is_empty())
            .flat_map(|(question, question_id)| {
                question
                    .choices
                    .iter()
                    .cloned()
                    .map(|choice| form_choices::ActiveModel {
                        id: ActiveValue::NotSet,
                        question_id: Set(question_id),
                        choice: Set(choice),
                    })
                    .collect_vec()
            })
            .collect_vec();

        if !choices_active_values.is_empty() {
            // NOTE: insert_manyに渡すvecが空だとinsertに失敗する
            FormChoices::insert_many(choices_active_values)
                .exec(&self.pool)
                .await?;
        }

        Ok(())
    }
}
