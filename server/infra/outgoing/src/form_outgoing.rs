use domain::form::models::Form;
use errors::infra::InfraError;

use crate::webhook::Webhook;

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
            .send()
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
            .send()
            .await?;
    }

    Ok(())
}
