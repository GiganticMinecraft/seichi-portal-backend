use anyhow::anyhow;
use domain::form::models::Form;
use webhook::client::WebhookClient;

pub async fn create(form: Form) -> anyhow::Result<()> {
    if let Some(url) = form.settings().webhook_url() {
        let client = WebhookClient::new(url);
        client
            .send(|message| {
                message.username("seichi-portal-backend").embed(|embed| {
                    embed
                        .title("フォームが作成されました")
                        .field("フォーム名", form.title().title(), false)
                        .field(
                            "フォーム説明",
                            &form
                                .description()
                                .description()
                                .to_owned()
                                .unwrap_or("フォームの説明は設定されていません。".to_string()),
                            false,
                        )
                })
            })
            .await
            .map_err(|_| anyhow!("Failed to notify form creation via webhook."))?;
    }

    Ok(())
}

pub async fn delete(form: Form) -> anyhow::Result<()> {
    if let Some(url) = form.settings().webhook_url() {
        let client = WebhookClient::new(url);
        client
            .send(|message| {
                message.username("seichi-portal-backend").embed(|embed| {
                    embed.title("フォームが削除されました。").field(
                        "フォーム名",
                        form.title().title(),
                        false,
                    )
                })
            })
            .await
            .map_err(|_| anyhow!("Failed to notify form deletion via webhook."))?;
    }

    Ok(())
}
