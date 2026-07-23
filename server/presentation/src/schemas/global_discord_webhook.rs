use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct GlobalDiscordWebhookUpdateSchema {
    /// Discord Webhook URL。`null` を指定すると通知を無効化する。
    #[serde(deserialize_with = "deserialize_required_nullable_url")]
    #[schema(required = true)]
    pub url: Option<String>,
}

fn deserialize_required_nullable_url<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}

#[derive(Serialize, ToSchema)]
pub struct GlobalDiscordWebhookStatusSchema {
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_response_never_contains_the_webhook_url() {
        let value =
            serde_json::to_value(GlobalDiscordWebhookStatusSchema { enabled: true }).unwrap();

        assert_eq!(value, serde_json::json!({ "enabled": true }));
    }

    #[test]
    fn update_request_requires_url_field_while_accepting_string_or_null() {
        assert!(serde_json::from_str::<GlobalDiscordWebhookUpdateSchema>(r#"{}"#).is_err());
        assert_eq!(
            serde_json::from_str::<GlobalDiscordWebhookUpdateSchema>(r#"{"url":null}"#)
                .unwrap()
                .url,
            None
        );
        assert_eq!(
            serde_json::from_str::<GlobalDiscordWebhookUpdateSchema>(
                r#"{"url":"https://discord.com/api/webhooks/123/token"}"#,
            )
            .unwrap()
            .url
            .as_deref(),
            Some("https://discord.com/api/webhooks/123/token")
        );
    }

    #[test]
    fn update_request_openapi_marks_url_as_required_and_nullable() {
        let schema = serde_json::to_value(
            <GlobalDiscordWebhookUpdateSchema as utoipa::PartialSchema>::schema(),
        )
        .unwrap();

        assert_eq!(schema["required"], serde_json::json!(["url"]));
        assert!(
            schema["properties"]["url"]["type"]
                .as_array()
                .is_some_and(|types| types.iter().any(|value| value == "null"))
        );
    }
}
