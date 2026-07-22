use deriving_via::DerivingVia;
use errors::domain::DomainError;
use regex::Regex;
use types::non_empty_string::NonEmptyString;

use crate::{
    auth::Actor,
    form::is_administrator,
    types::authorization_guard::{AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded},
};

/// Discord が発行した Webhook URL であることを検証済みの値。
#[derive(Clone, DerivingVia, PartialEq)]
#[deriving(IntoInner(via: NonEmptyString))]
pub struct ValidatedDiscordWebhookUrl(NonEmptyString);

impl std::fmt::Debug for ValidatedDiscordWebhookUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ValidatedDiscordWebhookUrl([REDACTED])")
    }
}

impl ValidatedDiscordWebhookUrl {
    pub fn try_new(url: NonEmptyString) -> Result<Self, DomainError> {
        let regex = Regex::new(r"^https://discord\.com/api/webhooks/[^/?#]+/[^/?#]+$").unwrap();
        if !regex.is_match(url.as_str()) {
            return Err(DomainError::InvalidDiscordWebhookUrl);
        }

        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// システム全体の Discord Webhook 通知設定。
///
/// URL の未検証状態や「有効だが URL がない」状態を型として作れない。
#[derive(Clone, Debug, Default, PartialEq)]
pub enum GlobalDiscordWebhookSetting {
    #[default]
    Disabled,
    Enabled(ValidatedDiscordWebhookUrl),
}

impl GlobalDiscordWebhookSetting {
    pub fn from_optional_url(url: Option<NonEmptyString>) -> Result<Self, DomainError> {
        url.map(ValidatedDiscordWebhookUrl::try_new)
            .transpose()
            .map(|url| url.map_or(Self::Disabled, Self::Enabled))
    }

    pub fn enabled(&self) -> bool {
        matches!(self, Self::Enabled(_))
    }

    pub fn url(&self) -> Option<&ValidatedDiscordWebhookUrl> {
        match self {
            Self::Disabled => None,
            Self::Enabled(url) => Some(url),
        }
    }
}

impl AuthorizationRole for GlobalDiscordWebhookSetting {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for GlobalDiscordWebhookSetting {
    fn can_create(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System) || is_administrator(actor)
    }

    fn can_update(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::models::{AccountUser, Role, UserId};
    use uuid::Uuid;

    #[test]
    fn accepts_only_discord_webhook_urls_without_query_or_fragment() {
        let valid =
            NonEmptyString::try_new("https://discord.com/api/webhooks/123/token".to_string())
                .unwrap();
        assert!(ValidatedDiscordWebhookUrl::try_new(valid).is_ok());

        for invalid in [
            "https://example.com/api/webhooks/123/token",
            "https://discord.com/api/webhooks/123/token?wait=true",
            "https://discord.com/api/webhooks/123",
        ] {
            let value = NonEmptyString::try_new(invalid.to_string()).unwrap();
            assert_eq!(
                ValidatedDiscordWebhookUrl::try_new(value),
                Err(DomainError::InvalidDiscordWebhookUrl)
            );
        }
    }

    #[test]
    fn singleton_setting_cannot_be_created_and_only_administrator_can_update() {
        let setting = GlobalDiscordWebhookSetting::Disabled;
        let administrator = Actor::from(AccountUser::new(
            "administrator".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::Administrator,
        ));
        let standard_user = Actor::from(AccountUser::new(
            "standard".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::StandardUser,
        ));

        assert!(setting.can_read(&administrator));
        assert!(setting.can_read(&Actor::System));
        assert!(!setting.can_read(&standard_user));
        assert!(!setting.can_create(&administrator));
        assert!(!setting.can_create(&Actor::System));
        assert!(!setting.can_create(&standard_user));
        assert!(setting.can_update(&administrator));
        assert!(!setting.can_update(&Actor::System));
        assert!(!setting.can_update(&standard_user));
    }
}
