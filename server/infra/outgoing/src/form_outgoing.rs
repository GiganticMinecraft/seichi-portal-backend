use domain::{
    form::models::{Comment, DefaultAnswerTitle, Form, FormAnswer, FormAnswerContent, Question},
    user::models::User,
};
use errors::infra::InfraError;
use itertools::Itertools;

use crate::webhook::{Color, Webhook};

#[tracing::instrument]
pub async fn create(form: Form) -> Result<(), InfraError> {
    if let Some(url) = form.settings().webhook_url().to_owned().into_inner() {
        Webhook::new(url, "フォームが作成されました".to_string())
            .field(
                "フォーム名".to_string(),
                form.title().to_owned().into_inner(),
                false,
            )
            .field(
                "フォーム説明".to_owned(),
                form.description()
                    .to_owned()
                    .into_inner()
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
    questions: &[Question],
) -> Result<(), InfraError> {
    if let Some(url) = form.settings().webhook_url().to_owned().into_inner() {
        Webhook::new(url, "回答が送信されました".to_string())
            .field(
                "フォーム名".to_string(),
                form.title().to_owned().into_inner().to_owned(),
                false,
            )
            .field(
                "タイトル".to_string(),
                title
                    .to_owned()
                    .into_inner()
                    .unwrap_or("タイトルなし".to_string()),
                false,
            )
            .fields(
                answers
                    .iter()
                    .map(|answer| {
                        (
                            questions
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
    if let Some(url) = form.settings().webhook_url().to_owned().into_inner() {
        Webhook::new(url, "回答に対してコメントが投稿されました".to_string())
            .field(
                "回答".to_string(),
                answer
                    .title
                    .to_owned()
                    .unwrap_or("タイトルなし".to_string()),
                false,
            )
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
    if let Some(url) = form.settings().webhook_url().to_owned().into_inner() {
        Webhook::new(url, "フォームが削除されました".to_string())
            .field(
                "フォーム名".to_string(),
                form.title().to_owned().into_inner(),
                false,
            )
            .send(Color::Red)
            .await?;
    }

    Ok(())
}
