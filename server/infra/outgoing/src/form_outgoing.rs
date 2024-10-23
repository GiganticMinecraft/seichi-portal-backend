use domain::{
    form::models::{Comment, DefaultAnswerTitle, Form, FormAnswer, FormAnswerContent},
    user::models::User,
};
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

#[tracing::instrument]
pub async fn post_answer(
    form: &Form,
    user: &User,
    title: DefaultAnswerTitle,
    answers: &Vec<FormAnswerContent>,
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
                title.default_answer_title.to_owned().unwrap_or_default(),
                false,
            )
            .fields(
                answers
                    .iter()
                    .map(|answer| {
                        (
                            form.questions
                                .iter()
                                .find(|question| question.id == Some(answer.question_id))
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
    answer: &FormAnswer,
) -> Result<(), InfraError> {
    if let Some(url) = form.settings.webhook_url.webhook_url.to_owned() {
        Webhook::new(url, "回答に対してコメントが投稿されました".to_string())
            .field("回答".to_string(), answer.title.unwrap_or_default(), false)
            .field("内容".to_string(), comment.content.to_owned(), false)
            .field(
                "発言者".to_string(),
                comment.commented_by.name.to_owned(),
                false,
            )
            .send(Color::Lime)
            .await?;
    }

    Ok(())
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
