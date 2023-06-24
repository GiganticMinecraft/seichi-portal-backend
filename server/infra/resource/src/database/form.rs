use async_trait::async_trait;
use domain::form::models::{
    Form, FormDescription, FormId, FormMeta, FormSettings, FormTitle, FormUpdateTargets, Question,
    ResponsePeriod, WebhookUrl,
};
use entities::{
    form_choices, form_meta_data, form_questions, form_webhooks,
    prelude::{FormChoices, FormMetaData, FormQuestions, FormWebhooks},
    response_period,
};
use errors::presentation::PresentationError::FormNotFound;
use futures::{stream, stream::StreamExt};
use itertools::Itertools;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ActiveValue, ActiveValue::Set, EntityTrait, ModelTrait,
    QueryFilter, QueryOrder, QuerySelect,
};

use crate::database::{components::FormDatabase, connection::ConnectionPool};

#[async_trait]
impl FormDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
    ) -> anyhow::Result<FormId> {
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
    async fn list(&self, offset: i32, limit: i32) -> anyhow::Result<Vec<Form>> {
        let forms = FormMetaData::find()
            .order_by_asc(form_meta_data::Column::Id)
            .offset(offset as u64)
            .limit(limit as u64)
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

        forms
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

                        anyhow::Ok(
                            Question::builder()
                                .title(question.title.to_owned())
                                .description(question.description.to_owned())
                                .question_type(question.question_type.to_string().try_into()?)
                                .choices(choices)
                                .build(),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let response_period = all_periods
                    .iter()
                    .filter(|period| period.form_id == form.id)
                    .map(|period| {
                        ResponsePeriod::new(Some((
                            period.start_at.to_owned().and_utc(),
                            period.end_at.to_owned().and_utc(),
                        )))
                    })
                    .next()
                    .ok_or(FormNotFound)?;

                let webhook_url = all_webhook_urls
                    .iter()
                    .filter(|webhook_url| webhook_url.form_id == form.id)
                    .map(|webhook_url| WebhookUrl {
                        webhook_url: Some(webhook_url.url.to_owned()),
                    })
                    .next()
                    .ok_or(FormNotFound)?;

                anyhow::Ok(
                    Form::builder()
                        .id(FormId(form.id))
                        .title(FormTitle::builder().title(form.title).build())
                        .description(
                            FormDescription::builder()
                                .description(form.description)
                                .build(),
                        )
                        .questions(questions)
                        .metadata(
                            FormMeta::builder()
                                .created_at(form.created_at)
                                .update_at(form.updated_at)
                                .build(),
                        )
                        .settings(FormSettings {
                            response_period,
                            webhook_url,
                        })
                        .build(),
                )
            })
            .collect()
    }

    #[tracing::instrument]
    async fn get(&self, form_id: FormId) -> anyhow::Result<Form> {
        let target_form = FormMetaData::find()
            .filter(Expr::col(form_meta_data::Column::Id).eq(form_id.0))
            .all(&self.pool)
            .await?
            .first()
            .ok_or(FormNotFound)?
            .to_owned();

        let form_questions = stream::iter(
            FormQuestions::find()
                .filter(Expr::col(form_questions::Column::FormId).eq(form_id.0))
                .all(&self.pool)
                .await?,
        )
        .then(|question| async {
            let choices = FormChoices::find()
                .filter(
                    Expr::col(form_choices::Column::QuestionId).eq(question.to_owned().question_id),
                )
                .all(&self.pool)
                .await?
                .into_iter()
                .map(|choice| choice.choice)
                .collect_vec();

            Ok(Question::builder()
                .title(question.title)
                .description(question.description)
                .question_type(question.question_type.to_string().try_into()?)
                .choices(choices)
                .build())
        })
        .collect::<Vec<anyhow::Result<Question>>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<Question>>>()?;

        let response_period = entities::response_period::Entity::find()
            .filter(Expr::col(response_period::Column::FormId).eq(target_form.id))
            .all(&self.pool)
            .await?
            .first()
            .map(|period| {
                ResponsePeriod::new(Some((
                    period.start_at.to_owned().and_utc(),
                    period.end_at.to_owned().and_utc(),
                )))
            })
            .ok_or(FormNotFound)?;

        let webhook_url = FormWebhooks::find()
            .filter(Expr::col(form_webhooks::Column::FormId).eq(target_form.id))
            .all(&self.pool)
            .await?
            .first()
            .map(|webhook_url_model| WebhookUrl {
                webhook_url: Some(webhook_url_model.url.to_owned()),
            })
            .ok_or(FormNotFound)?;

        let form_settings = FormSettings {
            response_period,
            webhook_url,
        };

        Ok(Form::builder()
            .id(FormId(target_form.id.to_owned()))
            .title(
                FormTitle::builder()
                    .title(target_form.title.to_owned())
                    .build(),
            )
            .description(
                FormDescription::builder()
                    .description(target_form.description.to_owned())
                    .build(),
            )
            .questions(form_questions)
            .metadata(
                FormMeta::builder()
                    .created_at(target_form.created_at)
                    .update_at(target_form.updated_at)
                    .build(),
            )
            .settings(form_settings)
            .build())
    }

    #[tracing::instrument]
    async fn delete(&self, form_id: FormId) -> anyhow::Result<FormId> {
        let target_form = FormMetaData::find_by_id(form_id.0)
            .all(&self.pool)
            .await?
            .first()
            .ok_or(FormNotFound)?
            .to_owned();

        let question_ids = FormQuestions::find()
            .filter(Expr::col(form_questions::Column::FormId).eq(form_id.0))
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
            .filter(Expr::col(response_period::Column::FormId).eq(form_id.0))
            .exec(&self.pool)
            .await?;

        FormQuestions::delete_many()
            .filter(Expr::col(form_questions::Column::FormId).eq(form_id.0))
            .exec(&self.pool)
            .await?;

        target_form.delete(&self.pool).await?;

        Ok(form_id)
    }

    async fn update(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> anyhow::Result<Form> {
        let current_form = self.get(form_id).await?;

        let updated_form = Form {
            id: form_id,
            title: match form_update_targets.title {
                Some(title) => title,
                None => current_form.title,
            },
            description: match form_update_targets.description {
                Some(description) => description,
                None => current_form.description,
            },
            settings: FormSettings {
                response_period: match form_update_targets.response_period {
                    Some(response_period) => response_period,
                    None => current_form.settings.response_period,
                },
                webhook_url: match form_update_targets.webhook {
                    Some(webhook_url) => webhook_url,
                    None => current_form.settings.webhook_url,
                },
            },
            ..current_form
        };

        Ok(updated_form)
    }
}
