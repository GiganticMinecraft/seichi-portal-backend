use std::time::Duration;

use common::retry::{RetryPolicy, retry_async};
use errors::infra::InfraError;
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
    pub webhook_url: String,
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
        retry_async(Self::retry_policy(), |_| {
            let sender = self.clone();
            let message = message.clone();
            async move { sender.send(message).await }
        })
        .await
    }

    async fn send(&self, message: DiscordWebhookMessage) -> Result<(), InfraError> {
        let webhook_url = message.webhook_url.clone();
        let request = DiscordWebhookRequest::from(message);
        let response = self.client.post(webhook_url).json(&request).send().await?;
        let status = response.status();

        status
            .is_success()
            .then_some(())
            .ok_or_else(|| InfraError::Outgoing {
                cause: format!("Discord webhook returned non-success status: {status}"),
            })
    }
}

impl Default for DiscordWebhookSender {
    fn default() -> Self {
        Self::new()
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
