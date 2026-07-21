use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use reqwest::Url;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Discord {
    pub bot_token: String,
}

pub static DISCORD_BOT: LazyLock<Discord> =
    LazyLock::new(|| envy::prefixed("DISCORD_").from_env::<Discord>().unwrap());

#[derive(Clone)]
pub struct DiscordGlobalWebhookUrl(String);

impl DiscordGlobalWebhookUrl {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Debug for DiscordGlobalWebhookUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DiscordGlobalWebhookUrl([REDACTED])")
    }
}

pub fn load_discord_global_webhook_url() -> Result<Option<DiscordGlobalWebhookUrl>> {
    let value = match std::env::var("DISCORD_GLOBAL_WEBHOOK_URL") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(error).context("DISCORD_GLOBAL_WEBHOOK_URL could not be read"),
    };

    parse_discord_global_webhook_url(value).map(Some)
}

fn parse_discord_global_webhook_url(value: String) -> Result<DiscordGlobalWebhookUrl> {
    if value.trim().is_empty() {
        bail!("DISCORD_GLOBAL_WEBHOOK_URL must not be empty when set");
    }

    let url = Url::parse(&value).context("DISCORD_GLOBAL_WEBHOOK_URL must be a valid URL")?;
    let path_segments = url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();
    let is_discord_webhook = url.scheme() == "https"
        && matches!(url.host_str(), Some("discord.com") | Some("discordapp.com"))
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && matches!(path_segments.as_slice(), ["api", "webhooks", id, token] if !id.is_empty() && !token.is_empty());

    if !is_discord_webhook {
        bail!("DISCORD_GLOBAL_WEBHOOK_URL must be a Discord webhook URL");
    }

    Ok(DiscordGlobalWebhookUrl(value))
}

#[cfg(test)]
mod tests {
    use super::{DiscordGlobalWebhookUrl, parse_discord_global_webhook_url};

    #[test]
    fn global_webhook_url_debug_output_is_redacted() {
        let url = DiscordGlobalWebhookUrl(
            "https://discord.com/api/webhooks/123/sensitive-token".to_string(),
        );

        let output = format!("{url:?}");

        assert!(!output.contains("sensitive-token"));
        assert_eq!(output, "DiscordGlobalWebhookUrl([REDACTED])");
    }

    #[test]
    fn accepts_discord_webhook_url() {
        assert!(
            parse_discord_global_webhook_url(
                "https://discord.com/api/webhooks/123/token".to_string()
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_empty_or_non_discord_url() {
        for value in [
            "",
            "   ",
            "http://discord.com/api/webhooks/123/token",
            "https://example.com/api/webhooks/123/token",
            "https://discord.com/api/webhooks/123/token/extra",
            "https://discord.com/api/webhooks/123/token?wait=true",
        ] {
            assert!(parse_discord_global_webhook_url(value.to_string()).is_err());
        }
    }
}
