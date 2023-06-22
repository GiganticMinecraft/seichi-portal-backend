use domain::form::models::Form;
use webhook::client::WebhookClient;

async fn create(form: Form) -> anyhow::Result<()> {
    match form.settings().webhook_url() {
        Some(url) => {
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
                                    .unwrap_or("フォームの説明は設定されていません。".to_string()),
                                false,
                            )
                    })
                })
                .await?
        }
        _ => (),
    }

    Ok(())
}

async fn delete(form: Form) -> anyhow::Result<()> {
    match form.settings().webhook_url() {
        Some(url) => {
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
                .await?
        }
        _ => (),
    }

    Ok(())
}
