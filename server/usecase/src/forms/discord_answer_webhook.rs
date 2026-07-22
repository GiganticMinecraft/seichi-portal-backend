use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscordAnswerWebhookField {
    pub name: String,
    pub value: String,
}

impl DiscordAnswerWebhookField {
    pub fn new(name: String, value: String) -> Self {
        Self { name, value }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DiscordAnswerWebhookNotification {
    pub discord_webhook_url: String,
    pub title: String,
    pub answer_url: String,
    pub form_id: String,
    pub answer_id: String,
    pub fields: Vec<DiscordAnswerWebhookField>,
}

impl std::fmt::Debug for DiscordAnswerWebhookNotification {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DiscordAnswerWebhookNotification")
            .field("discord_webhook_url", &"[REDACTED]")
            .field("form_id", &self.form_id)
            .field("answer_id", &self.answer_id)
            .finish_non_exhaustive()
    }
}

#[async_trait]
pub trait DiscordAnswerWebhookNotifier: Send + Sync {
    async fn notify_answer_posted(&self, notification: DiscordAnswerWebhookNotification);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_debug_redacts_the_webhook_token() {
        let secret = "super-secret-token";
        let notification = DiscordAnswerWebhookNotification {
            discord_webhook_url: format!("https://discord.com/api/webhooks/123/{secret}"),
            title: "title".to_string(),
            answer_url: "https://example.com/answer".to_string(),
            form_id: "form".to_string(),
            answer_id: "answer".to_string(),
            fields: Vec::new(),
        };

        assert!(!format!("{notification:?}").contains(secret));
    }
}
