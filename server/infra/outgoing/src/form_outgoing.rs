use domain::form::models::{Answer, Comment, Form, PostedAnswersSchema};
use domain::user::models::User;
use errors::infra::InfraError;
use itertools::Itertools;

use crate::webhook::{Color, Webhook};

#[tracing::instrument]
pub async fn create(form: Form) -> Result<(), InfraError> {
    if let Some(url) = form.settings.webhook_url.into() {
        Webhook::new(url, "フォームが作成されました".to_string())
            .field(
                "フォーム名".to_string(),
                form.title.title().to_owned(),
                false,
            )
            .field(
                "フォーム説明".to_owned(),
                form.description
                    .description()
                    .to_owned()
                    .unwrap_or("フォームの説明は設定されていません。".to_string()),
                false,
            )
            .send(Color::Aqua)
            .await?;
    }

    Ok(())
}

pub async fn post_answer(
    form: &Form,
    user: &User,
    answers: &PostedAnswersSchema,
) -> Result<(), InfraError> {
    if let Some(url) = form.settings.webhook_url.to_owned() {
        Webhook::new(url, "回答が送信されました".to_string())
            .field(
                "フォーム名".to_string(),
                form.title.title().to_owned(),
                false,
            )
            .field(
                "タイトル".to_string(),
                answers
                    .title
                    .default_answer_title
                    .to_owned()
                    .unwrap_or_default(),
                false,
            )
            .fields(
                answers
                    .answers
                    .iter()
                    .map(|answer| {
                        (
                            form.questions
                                .iter()
                                .find(|question| question.id == answer.question_id)
                                .map(|question| question.title.to_owned())
                                .unwrap_or("不明な質問".to_string()),
                            answer.answer.to_owned(),
                        )
                    })
                    .collect_vec(),
                false,
            )
            .field("回答者".to_string(), user.name.to_owned(), false)
            .send(Color::Lime)
            .await?;
    }

    Ok(())
}

#[tracing::instrument]
pub async fn post_comment(
    form: &Form,
    comment: &Comment,
    answer: Answer,
) -> Result<(), InfraError> {
    todo!()
    // if let Some(url) = form.settings.webhook_url.into() {}
}

#[tracing::instrument]
pub async fn delete(form: Form) -> Result<(), InfraError> {
    if let Some(url) = form.settings.webhook_url.into() {
        Webhook::new(url, "フォームが削除されました".to_string())
            .field(
                "フォーム名".to_string(),
                form.title.title().to_owned(),
                false,
            )
            .send(Color::Red)
            .await?;
    }

    Ok(())
}
