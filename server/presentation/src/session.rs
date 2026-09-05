use axum::http::HeaderMap;
use chrono::TimeDelta;

use crate::rate_limit::PROXY_SECRET_HEADER;

#[derive(Clone)]
pub struct SessionPolicy {
    maximum_lifetime: TimeDelta,
    proxy_secret: Option<String>,
}

impl SessionPolicy {
    pub fn new(maximum_lifetime_seconds: u32, proxy_secret: Option<String>) -> Self {
        Self {
            maximum_lifetime: TimeDelta::seconds(i64::from(maximum_lifetime_seconds)),
            proxy_secret: proxy_secret.filter(|secret| !secret.is_empty()),
        }
    }

    pub fn maximum_lifetime(&self) -> TimeDelta {
        self.maximum_lifetime
    }

    pub fn allows_session_creation(&self, headers: &HeaderMap) -> bool {
        let Some(expected) = self.proxy_secret.as_deref() else {
            return true;
        };

        headers
            .get(PROXY_SECRET_HEADER)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|actual| actual == expected)
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn configured_proxy_secret_is_required_for_session_creation() {
        let policy = SessionPolicy::new(60, Some("expected".to_owned()));
        let mut headers = HeaderMap::new();

        assert!(!policy.allows_session_creation(&headers));

        headers.insert(PROXY_SECRET_HEADER, HeaderValue::from_static("different"));
        assert!(!policy.allows_session_creation(&headers));

        headers.insert(PROXY_SECRET_HEADER, HeaderValue::from_static("expected"));
        assert!(policy.allows_session_creation(&headers));
    }

    #[test]
    fn proxy_secret_is_optional_for_local_development() {
        let policy = SessionPolicy::new(60, None);

        assert!(policy.allows_session_creation(&HeaderMap::new()));
    }
}
