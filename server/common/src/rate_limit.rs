use async_trait::async_trait;

/// A single fixed-window quota evaluated by the shared rate-limit store.
///
/// The key is already namespaced by the caller.  The store appends the
/// Valkey-derived window id before reading or updating the counter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitQuota {
    pub key: String,
    pub limit: u64,
    pub window_seconds: u64,
}

impl RateLimitQuota {
    pub fn new(key: impl Into<String>, limit: u64, window_seconds: u64) -> Self {
        Self {
            key: key.into(),
            limit,
            window_seconds,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub retry_after_seconds: u64,
    pub remaining: u64,
    pub limit: u64,
    pub reset_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitStoreError {
    /// The store could not be reached or timed out.  Callers may fail open.
    Unavailable(String),
    /// The store returned an invalid result or rejected the request shape.
    Invalid(String),
}

#[async_trait]
pub trait RateLimitStore: Send + Sync {
    async fn check(
        &self,
        quotas: &[RateLimitQuota],
    ) -> Result<RateLimitDecision, RateLimitStoreError>;
}
