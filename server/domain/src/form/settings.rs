use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use regex::Regex;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use types::non_empty_string::NonEmptyString;

use crate::{account::models::UserGroupId, auth::Actor, form::is_administrator};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
pub struct AllowedUserGroups(Vec<UserGroupId>);

impl AllowedUserGroups {
    pub fn new(group_ids: Vec<UserGroupId>) -> Self {
        let mut group_ids = group_ids;
        group_ids.sort_by_key(|id| id.into_inner());
        group_ids.dedup();
        Self(group_ids)
    }

    pub fn unrestricted() -> Self {
        Self(Vec::new())
    }

    pub fn as_slice(&self) -> &[UserGroupId] {
        &self.0
    }

    pub fn allows(&self, actor: &Actor) -> bool {
        if self.0.is_empty() || matches!(actor, Actor::System) || is_administrator(actor) {
            return true;
        }

        matches!(actor, Actor::AccountUser(user) if user
            .groups()
            .iter()
            .any(|group| self.0.contains(group.id())))
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct FormSettings {
    #[serde(default)]
    discord_webhook_url: DiscordWebhookUrl,
    #[serde(default)]
    visibility: Visibility,
    #[serde(default)]
    allowed_user_groups: AllowedUserGroups,
}

impl FormSettings {
    pub fn new() -> Self {
        Self {
            discord_webhook_url: DiscordWebhookUrl::try_new(None).unwrap(),
            visibility: Visibility::PUBLIC,
            allowed_user_groups: AllowedUserGroups::unrestricted(),
        }
    }

    pub fn discord_webhook_url(&self, actor: &Actor) -> Result<&DiscordWebhookUrl, DomainError> {
        if matches!(actor, Actor::System) {
            Ok(&self.discord_webhook_url)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    pub fn discord_webhook_enabled(&self) -> bool {
        self.discord_webhook_url.0.is_some()
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn allowed_user_groups(&self) -> &AllowedUserGroups {
        &self.allowed_user_groups
    }

    pub fn change_discord_webhook_url(self, discord_webhook_url: DiscordWebhookUrl) -> Self {
        Self {
            discord_webhook_url,
            ..self
        }
    }

    pub fn change_visibility(self, visibility: Visibility) -> Self {
        Self { visibility, ..self }
    }

    pub fn change_allowed_user_groups(self, allowed_user_groups: AllowedUserGroups) -> Self {
        Self {
            allowed_user_groups,
            ..self
        }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Default, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<NonEmptyString>), Deserialize(via: Option::<NonEmptyString>
))]
pub struct DiscordWebhookUrl(Option<NonEmptyString>);

impl std::fmt::Debug for DiscordWebhookUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DiscordWebhookUrl([REDACTED])")
    }
}

impl DiscordWebhookUrl {
    pub fn try_new(url: Option<NonEmptyString>) -> Result<Self, DomainError> {
        if let Some(url) = &url {
            let regex = Regex::new("https://discord.com/api/webhooks/.*").unwrap();
            if !regex.is_match(url) {
                return Err(DomainError::InvalidDiscordWebhookUrl);
            }
        }

        Ok(Self(url))
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(
    Serialize, Deserialize, Debug, EnumString, Display, Copy, Clone, Default, PartialOrd, PartialEq,
)]
pub enum Visibility {
    PUBLIC,
    #[default]
    PRIVATE,
}

impl TryFrom<String> for Visibility {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::models::{AccountUser, Role};
    use uuid::Uuid;

    #[test]
    fn webhook_url_is_available_only_to_system() {
        let secret = "super-secret-token";
        let webhook = DiscordWebhookUrl::try_new(Some(
            NonEmptyString::try_new(format!("https://discord.com/api/webhooks/123/{secret}"))
                .unwrap(),
        ))
        .unwrap();
        let settings = FormSettings::new().change_discord_webhook_url(webhook.clone());
        let administrator = Actor::from(AccountUser::new(
            "administrator".to_string(),
            Uuid::new_v4().into(),
            Role::Administrator,
        ));

        assert_eq!(settings.discord_webhook_url(&Actor::System), Ok(&webhook));
        assert!(matches!(
            settings.discord_webhook_url(&administrator),
            Err(DomainError::Forbidden)
        ));
    }

    #[test]
    fn webhook_enabled_reports_whether_a_url_is_configured() {
        let settings_without_webhook = FormSettings::new();
        let settings_with_webhook = settings_without_webhook.clone().change_discord_webhook_url(
            DiscordWebhookUrl::try_new(Some(
                NonEmptyString::try_new("https://discord.com/api/webhooks/123/token".to_string())
                    .unwrap(),
            ))
            .unwrap(),
        );

        assert!(!settings_without_webhook.discord_webhook_enabled());
        assert!(settings_with_webhook.discord_webhook_enabled());
    }

    #[test]
    fn webhook_url_and_form_settings_debug_redact_the_token() {
        let secret = "super-secret-token";
        let webhook = DiscordWebhookUrl::try_new(Some(
            NonEmptyString::try_new(format!("https://discord.com/api/webhooks/123/{secret}"))
                .unwrap(),
        ))
        .unwrap();
        let settings = FormSettings::new().change_discord_webhook_url(webhook.clone());

        assert!(!format!("{webhook:?}").contains(secret));
        assert!(!format!("{settings:?}").contains(secret));
    }
}
