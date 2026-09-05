use std::env;

use anyhow::Result;

const DEFAULT_MAXIMUM_LIFETIME_SECONDS: u32 = 7 * 24 * 60 * 60;

pub struct SessionConfig {
    maximum_lifetime_seconds: u32,
}

impl SessionConfig {
    pub fn from_environment() -> Result<Self> {
        Self::from_value(env::var("SESSION_MAX_LIFETIME_SECONDS").ok().as_deref())
    }

    fn from_value(value: Option<&str>) -> Result<Self> {
        let maximum_lifetime_seconds = value
            .map(str::parse::<u32>)
            .transpose()
            .map_err(|_| {
                anyhow::anyhow!("SESSION_MAX_LIFETIME_SECONDS must be a positive integer")
            })?
            .unwrap_or(DEFAULT_MAXIMUM_LIFETIME_SECONDS);

        if maximum_lifetime_seconds == 0 {
            anyhow::bail!("SESSION_MAX_LIFETIME_SECONDS must be greater than zero");
        }

        Ok(Self {
            maximum_lifetime_seconds,
        })
    }

    pub fn maximum_lifetime_seconds(&self) -> u32 {
        self.maximum_lifetime_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_maximum_lifetime_is_seven_days() {
        let config = SessionConfig::from_value(None).unwrap();

        assert_eq!(
            config.maximum_lifetime_seconds(),
            DEFAULT_MAXIMUM_LIFETIME_SECONDS
        );
    }

    #[test]
    fn maximum_lifetime_must_be_positive() {
        assert!(SessionConfig::from_value(Some("0")).is_err());
        assert!(SessionConfig::from_value(Some("invalid")).is_err());
    }
}
