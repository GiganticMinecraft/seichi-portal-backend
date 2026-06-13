use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use regex::Regex;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use types::non_empty_string::NonEmptyString;

use crate::{form::is_administrator, user::models::Actor};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct FormSettings {
    #[serde(default)]
    webhook_url: WebhookUrl,
    #[serde(default)]
    visibility: Visibility,
}

impl FormSettings {
    pub fn new() -> Self {
        Self {
            webhook_url: WebhookUrl::try_new(None).unwrap(),
            visibility: Visibility::PUBLIC,
        }
    }

    pub fn webhook_url(&self, actor: &Actor) -> Result<&WebhookUrl, DomainError> {
        if is_administrator(actor) {
            Ok(&self.webhook_url)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn change_webhook_url(self, webhook_url: WebhookUrl) -> Self {
        Self {
            webhook_url,
            ..self
        }
    }

    pub fn change_visibility(self, visibility: Visibility) -> Self {
        Self { visibility, ..self }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<NonEmptyString>), Deserialize(via: Option::<NonEmptyString>
))]
pub struct WebhookUrl(Option<NonEmptyString>);

impl WebhookUrl {
    pub fn try_new(url: Option<NonEmptyString>) -> Result<Self, DomainError> {
        if let Some(url) = &url {
            let regex = Regex::new("https://discord.com/api/webhooks/.*").unwrap();
            if !regex.is_match(url) {
                return Err(DomainError::InvalidWebhookUrl);
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
