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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscordAnswerWebhookNotification {
    pub discord_webhook_url: String,
    pub title: String,
    pub answer_url: String,
    pub form_id: String,
    pub answer_id: String,
    pub fields: Vec<DiscordAnswerWebhookField>,
}

#[async_trait]
pub trait DiscordAnswerWebhookNotifier: Send + Sync {
    async fn notify_answer_posted(&self, notification: DiscordAnswerWebhookNotification);
}
