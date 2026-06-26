use std::time::Duration;

use common::retry::{RetryPolicy, retry_async_if};
use errors::infra::InfraError;
use reqwest::StatusCode;
use serde::Serialize;

const DISCORD_EMBED_COLOR_LIME: i32 = 65_280;
const DISCORD_MAX_EMBED_FIELDS: usize = 25;
const DISCORD_FIELD_NAME_LIMIT: usize = 256;
const DISCORD_FIELD_VALUE_LIMIT: usize = 1024;
const DISCORD_EMBED_TITLE_LIMIT: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscordWebhookField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

impl DiscordWebhookField {
    pub fn new(name: String, value: String, inline: bool) -> Self {
        Self {
            name,
            value,
            inline,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscordWebhookMessage {
    pub discord_webhook_url: String,
    pub title: String,
    pub link_url: String,
    pub fields: Vec<DiscordWebhookField>,
}

#[derive(Clone)]
pub struct DiscordWebhookSender {
    client: reqwest::Client,
}

impl DiscordWebhookSender {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub fn retry_policy() -> RetryPolicy {
        RetryPolicy::new(5, Duration::from_secs(1), 2)
    }

    pub async fn send_with_retry(&self, message: DiscordWebhookMessage) -> Result<(), InfraError> {
        retry_async_if(
            Self::retry_policy(),
            |_| {
                let sender = self.clone();
                let message = message.clone();
                async move { sender.send(message).await }
            },
            DiscordWebhookSendError::is_retryable,
        )
        .await
        .map_err(Into::into)
    }

    async fn send(&self, message: DiscordWebhookMessage) -> Result<(), DiscordWebhookSendError> {
        let discord_webhook_url = message.discord_webhook_url.clone();
        let request = DiscordWebhookRequest::from(message);
        let response = self
            .client
            .post(discord_webhook_url)
            .json(&request)
            .send()
            .await
            .map_err(|error| DiscordWebhookSendError::Retryable(error.into()))?;
        let status = response.status();

        match status {
            status if status.is_success() => Ok(()),
            status if is_retryable_status(status) => {
                Err(DiscordWebhookSendError::Retryable(status_error(status)))
            }
            status => Err(DiscordWebhookSendError::Fatal(status_error(status))),
        }
    }
}

impl Default for DiscordWebhookSender {
    fn default() -> Self {
        Self::new()
    }
}

enum DiscordWebhookSendError {
    Retryable(InfraError),
    Fatal(InfraError),
}

impl DiscordWebhookSendError {
    fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

impl From<DiscordWebhookSendError> for InfraError {
    fn from(error: DiscordWebhookSendError) -> Self {
        match error {
            DiscordWebhookSendError::Retryable(error) | DiscordWebhookSendError::Fatal(error) => {
                error
            }
        }
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

fn status_error(status: StatusCode) -> InfraError {
    InfraError::Outgoing {
        cause: format!("Discord webhook returned non-success status: {status}"),
    }
}

#[derive(Serialize)]
struct DiscordWebhookRequest {
    username: String,
    embeds: Vec<DiscordEmbed>,
}

#[derive(Serialize)]
struct DiscordEmbed {
    title: String,
    url: String,
    color: i32,
    fields: Vec<DiscordEmbedField>,
}

#[derive(Clone, Serialize)]
struct DiscordEmbedField {
    name: String,
    value: String,
    inline: bool,
}

impl From<DiscordWebhookMessage> for DiscordWebhookRequest {
    fn from(message: DiscordWebhookMessage) -> Self {
        let fields = message
            .fields
            .into_iter()
            .map(DiscordEmbedField::from)
            .collect::<Vec<_>>();
        let embeds = fields
            .chunks(DISCORD_MAX_EMBED_FIELDS)
            .enumerate()
            .map(|(index, fields)| DiscordEmbed {
                title: match index {
                    0 => truncate(message.title.as_str(), DISCORD_EMBED_TITLE_LIMIT),
                    _ => truncate(
                        format!("{} ({})", message.title, index + 1).as_str(),
                        DISCORD_EMBED_TITLE_LIMIT,
                    ),
                },
                url: message.link_url.clone(),
                color: DISCORD_EMBED_COLOR_LIME,
                fields: fields.to_vec(),
            })
            .collect();

        Self {
            username: "seichi-portal-backend".to_string(),
            embeds,
        }
    }
}

impl From<DiscordWebhookField> for DiscordEmbedField {
    fn from(field: DiscordWebhookField) -> Self {
        Self {
            name: truncate(field.name.as_str(), DISCORD_FIELD_NAME_LIMIT),
            value: truncate(
                non_empty_value(field.value).as_str(),
                DISCORD_FIELD_VALUE_LIMIT,
            ),
            inline: field.inline,
        }
    }
}

fn non_empty_value(value: String) -> String {
    match value.trim().is_empty() {
        true => "(空)".to_string(),
        false => value,
    }
}

fn truncate(value: &str, limit: usize) -> String {
    match value.char_indices().nth(limit) {
        Some((index, _)) => value[..index].to_string(),
        None => value.to_string(),
    }
}
