use domain::form::models::Form;

use crate::webhook::Webhook;

pub async fn create(form: Form) -> anyhow::Result<()> {
    if let Some(url) = form.settings.webhook_url() {
        Webhook::new(url.to_string(), "フォームが作成されました".to_string())
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

pub async fn delete(form: Form) -> anyhow::Result<()> {
    if let Some(url) = form.settings.webhook_url() {
        Webhook::new(url.to_string(), "フォームが削除されました".to_string())
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
