use std::time::Duration;

use common::retry::{RetryPolicy, retry_async_if};
use errors::infra::InfraError;
use reqwest::StatusCode;
use serde::Serialize;

const DISCORD_EMBED_COLOR_LIME: i32 = 65_280;
const DISCORD_MAX_EMBED_FIELDS: usize = 25;
const DISCORD_MAX_EMBEDS: usize = 10;
const DISCORD_MAX_TOTAL_EMBED_CHARACTERS: usize = 6000;
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

#[derive(Clone, PartialEq, Eq)]
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
            .map_err(|_| DiscordWebhookSendError::Retryable(request_transport_error()))?;
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

fn request_transport_error() -> InfraError {
    InfraError::Outgoing {
        cause: "failed to send Discord webhook request".to_string(),
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
            .take(DISCORD_MAX_EMBED_FIELDS * DISCORD_MAX_EMBEDS)
            .collect::<Vec<_>>();
        let embeds = embeds_within_discord_limits(&message.title, &message.link_url, fields);

        Self {
            username: "seichi-portal-backend".to_string(),
            embeds,
        }
    }
}

fn embeds_within_discord_limits(
    title: &str,
    link_url: &str,
    fields: Vec<DiscordEmbedField>,
) -> Vec<DiscordEmbed> {
    let chunks = if fields.is_empty() {
        vec![Vec::new()]
    } else {
        fields
            .chunks(DISCORD_MAX_EMBED_FIELDS)
            .map(<[DiscordEmbedField]>::to_vec)
            .take(DISCORD_MAX_EMBEDS)
            .collect()
    };
    let mut remaining = DISCORD_MAX_TOTAL_EMBED_CHARACTERS;

    chunks
        .into_iter()
        .enumerate()
        .filter_map(|(index, fields)| {
            if remaining == 0 {
                return None;
            }

            let raw_title = match index {
                0 => title.to_owned(),
                _ => format!("{title} ({})", index + 1),
            };
            let title_limit = DISCORD_EMBED_TITLE_LIMIT.min(remaining);
            let embed_title = truncate(&raw_title, title_limit);
            remaining -= embed_title.chars().count();

            let fields = fields
                .into_iter()
                .map_while(|field| {
                    if remaining < 2 {
                        return None;
                    }

                    let name_limit = DISCORD_FIELD_NAME_LIMIT.min(remaining - 1);
                    let name = truncate(&field.name, name_limit);
                    remaining -= name.chars().count();

                    let value_limit = DISCORD_FIELD_VALUE_LIMIT.min(remaining);
                    let value = truncate(&field.value, value_limit);
                    remaining -= value.chars().count();

                    Some(DiscordEmbedField {
                        name,
                        value,
                        inline: field.inline,
                    })
                })
                .collect();

            Some(DiscordEmbed {
                title: embed_title,
                url: link_url.to_owned(),
                color: DISCORD_EMBED_COLOR_LIME,
                fields,
            })
        })
        .collect()
}

impl From<DiscordWebhookField> for DiscordEmbedField {
    fn from(field: DiscordWebhookField) -> Self {
        Self {
            name: truncate(
                non_empty_value(field.name).as_str(),
                DISCORD_FIELD_NAME_LIMIT,
            ),
            value: truncate(
                non_empty_value(field.value).as_str(),
                DISCORD_FIELD_VALUE_LIMIT,
            ),
            inline: field.inline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_conversion_obeys_all_embed_count_and_character_limits() {
        let message = DiscordWebhookMessage {
            discord_webhook_url: "https://discord.com/api/webhooks/id/token".to_string(),
            title: "題".repeat(500),
            link_url: "https://example.com".to_string(),
            fields: (0..400)
                .map(|_| DiscordWebhookField::new("名".repeat(500), "値".repeat(2000), false))
                .collect(),
        };

        let request = DiscordWebhookRequest::from(message);
        let total_characters = request
            .embeds
            .iter()
            .map(|embed| {
                embed.title.chars().count()
                    + embed
                        .fields
                        .iter()
                        .map(|field| field.name.chars().count() + field.value.chars().count())
                        .sum::<usize>()
            })
            .sum::<usize>();

        assert!(request.embeds.len() <= DISCORD_MAX_EMBEDS);
        assert!(total_characters <= DISCORD_MAX_TOTAL_EMBED_CHARACTERS);
        assert!(request.embeds.iter().all(|embed| {
            embed.title.chars().count() <= DISCORD_EMBED_TITLE_LIMIT
                && embed.fields.len() <= DISCORD_MAX_EMBED_FIELDS
                && embed.fields.iter().all(|field| {
                    !field.name.is_empty()
                        && !field.value.is_empty()
                        && field.name.chars().count() <= DISCORD_FIELD_NAME_LIMIT
                        && field.value.chars().count() <= DISCORD_FIELD_VALUE_LIMIT
                })
        }));
    }

    #[test]
    fn transport_error_never_contains_the_webhook_url_or_token() {
        let secret = "https://discord.com/api/webhooks/123/super-secret-token";
        let error = request_transport_error();
        let rendered = format!("{error:?}");

        assert!(!rendered.contains(secret));
        assert!(!rendered.contains("super-secret-token"));
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
