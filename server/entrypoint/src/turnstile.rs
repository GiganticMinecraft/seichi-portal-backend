use std::{collections::HashSet, env};

use anyhow::Result;

pub enum TurnstileConfig {
    Disabled,
    Enabled {
        secret_key: String,
        allowed_hostnames: HashSet<String>,
    },
}

impl TurnstileConfig {
    pub fn from_environment() -> Result<Self> {
        let enabled = env::var("TURNSTILE_ENABLED").ok();
        let secret_key = env::var("TURNSTILE_SECRET_KEY").ok();
        let allowed_hostnames = env::var("TURNSTILE_ALLOWED_HOSTNAMES").ok();

        Self::from_values(
            enabled.as_deref(),
            secret_key.as_deref(),
            allowed_hostnames.as_deref(),
        )
    }

    fn from_values(
        enabled: Option<&str>,
        secret_key: Option<&str>,
        allowed_hostnames: Option<&str>,
    ) -> Result<Self> {
        let enabled = enabled.ok_or_else(|| anyhow::anyhow!("TURNSTILE_ENABLED is not set"))?;
        let enabled = enabled
            .parse::<bool>()
            .map_err(|_| anyhow::anyhow!("TURNSTILE_ENABLED must be true or false"))?;

        if !enabled {
            return Ok(Self::Disabled);
        }

        let secret_key = secret_key
            .filter(|secret_key| !secret_key.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("TURNSTILE_SECRET_KEY is required when Turnstile is enabled")
            })?;
        let allowed_hostnames = allowed_hostnames
            .map(parse_allowed_hostnames)
            .transpose()?
            .filter(|allowed_hostnames| !allowed_hostnames.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("TURNSTILE_ALLOWED_HOSTNAMES is required when Turnstile is enabled")
            })?;

        Ok(Self::Enabled {
            secret_key: secret_key.to_owned(),
            allowed_hostnames,
        })
    }
}

fn parse_allowed_hostnames(value: &str) -> Result<HashSet<String>> {
    let hostnames: HashSet<String> = value
        .split(',')
        .map(str::trim)
        .filter(|hostname| !hostname.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();

    if hostnames.iter().any(|hostname| hostname.contains('*')) {
        anyhow::bail!(
            "TURNSTILE_ALLOWED_HOSTNAMES must contain exact hostnames; wildcard is not supported"
        );
    }

    Ok(hostnames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_must_be_present_and_boolean() {
        assert!(TurnstileConfig::from_values(None, None, None).is_err());
        assert!(TurnstileConfig::from_values(Some("maybe"), None, None).is_err());
    }

    #[test]
    fn disabled_does_not_require_secret_or_hostnames() {
        assert!(matches!(
            TurnstileConfig::from_values(Some("false"), None, None),
            Ok(TurnstileConfig::Disabled)
        ));
    }

    #[test]
    fn enabled_requires_secret_and_hostnames() {
        assert!(TurnstileConfig::from_values(Some("true"), None, Some("example.com")).is_err());
        assert!(TurnstileConfig::from_values(Some("true"), Some("secret"), None).is_err());
        assert!(TurnstileConfig::from_values(Some("true"), Some("secret"), Some(", ")).is_err());
        assert!(
            TurnstileConfig::from_values(Some("true"), Some("secret"), Some("*.example.com"))
                .is_err()
        );
    }

    #[test]
    fn enabled_normalizes_a_comma_separated_hostname_list() {
        let TurnstileConfig::Enabled {
            secret_key,
            allowed_hostnames,
        } = TurnstileConfig::from_values(
            Some("true"),
            Some("secret"),
            Some(" Example.COM,localhost,example.com "),
        )
        .unwrap()
        else {
            panic!("expected enabled Turnstile config");
        };

        assert_eq!(secret_key, "secret");
        assert_eq!(
            allowed_hostnames,
            HashSet::from(["example.com".to_owned(), "localhost".to_owned()])
        );
    }
}
