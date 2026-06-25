use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerWebhookField {
    pub name: String,
    pub value: String,
}

impl AnswerWebhookField {
    pub fn new(name: String, value: String) -> Self {
        Self { name, value }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerWebhookNotification {
    pub webhook_url: String,
    pub answer_url: String,
    pub form_id: String,
    pub answer_id: String,
    pub fields: Vec<AnswerWebhookField>,
}

#[async_trait]
pub trait AnswerWebhookNotifier: Send + Sync {
    async fn notify_answer_posted(&self, notification: AnswerWebhookNotification);
}
