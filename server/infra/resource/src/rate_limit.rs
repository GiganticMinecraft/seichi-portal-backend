use std::time::Duration;

use async_trait::async_trait;
use common::rate_limit::{RateLimitDecision, RateLimitQuota, RateLimitStore, RateLimitStoreError};
use redis::{Client, ErrorKind, Script, ServerErrorKind, aio::MultiplexedConnection};

const CHECK_SCRIPT: &str = r#"
local now = tonumber(redis.call('TIME')[1])
local quota_count = tonumber(ARGV[1])
local max_retry = 0
local max_reset = 0
local min_remaining = nil
local min_limit = nil

-- Read every counter before changing any of them.  This keeps temporary
-- answer quotas all-or-nothing when one of the limits has been exhausted.
for index = 1, quota_count do
    local arg_index = 2 + ((index - 1) * 2)
    local limit = tonumber(ARGV[arg_index])
    local window = tonumber(ARGV[arg_index + 1])
    local reset = window - (now % window)
    local window_key = KEYS[index] .. ':' .. tostring(math.floor(now / window))
    local raw_count = redis.call('GET', window_key)
    local current = 0
    if raw_count then
        current = tonumber(raw_count)
        if current == nil then
            return { -1 }
        end
    end

    if current >= limit and reset > max_retry then
        max_retry = reset
    end
    if reset > max_reset then
        max_reset = reset
    end
    local remaining = limit - current
    if min_remaining == nil or remaining < min_remaining then
        min_remaining = remaining
        min_limit = limit
    end
end

if max_retry > 0 then
    return { 0, max_retry, 0, min_limit or 0, max_retry }
end

for index = 1, quota_count do
    local arg_index = 2 + ((index - 1) * 2)
    local limit = tonumber(ARGV[arg_index])
    local window = tonumber(ARGV[arg_index + 1])
    local reset = window - (now % window)
    local window_key = KEYS[index] .. ':' .. tostring(math.floor(now / window))
    local count = redis.call('INCRBY', window_key, 1)
    redis.call('EXPIRE', window_key, reset)
    local remaining = limit - count
    if min_remaining == nil or remaining < min_remaining then
        min_remaining = remaining
        min_limit = limit
    end
end

return { 1, 0, min_remaining or 0, min_limit or 0, max_reset }
"#;

const OPERATION_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Clone, Debug)]
pub struct ValkeyRateLimitStore {
    client: Client,
}

impl ValkeyRateLimitStore {
    pub fn from_environment() -> Result<Self, RateLimitStoreError> {
        let host = std::env::var("REDIS_HOST")
            .map_err(|_| RateLimitStoreError::Invalid("REDIS_HOST is not set".into()))?;
        let port = std::env::var("REDIS_PORT")
            .map_err(|_| RateLimitStoreError::Invalid("REDIS_PORT is not set".into()))?;
        Self::from_url(format!("redis://{host}:{port}/"))
    }

    pub fn from_url(url: impl Into<String>) -> Result<Self, RateLimitStoreError> {
        let client = Client::open(url.into())
            .map_err(|error| RateLimitStoreError::Invalid(error.to_string()))?;
        Ok(Self { client })
    }

    async fn connection(&self) -> Result<MultiplexedConnection, RateLimitStoreError> {
        tokio::time::timeout(
            OPERATION_TIMEOUT,
            self.client.get_multiplexed_async_connection(),
        )
        .await
        .map_err(|_| RateLimitStoreError::Unavailable("Valkey connection timed out".into()))?
        .map_err(|error| RateLimitStoreError::Unavailable(error.to_string()))
    }
}

fn classify_redis_error(error: redis::RedisError) -> RateLimitStoreError {
    let unavailable = matches!(
        error.kind(),
        ErrorKind::Io
            | ErrorKind::ClusterConnectionNotFound
            | ErrorKind::MasterNameNotFoundBySentinel
            | ErrorKind::NoValidReplicasFoundBySentinel
            | ErrorKind::EmptySentinelList
            | ErrorKind::Server(
                ServerErrorKind::BusyLoading
                    | ServerErrorKind::TryAgain
                    | ServerErrorKind::ClusterDown
                    | ServerErrorKind::MasterDown
                    | ServerErrorKind::ReadOnly,
            )
    );

    if unavailable {
        RateLimitStoreError::Unavailable(error.to_string())
    } else {
        RateLimitStoreError::Invalid(error.to_string())
    }
}

fn validate_quotas(quotas: &[RateLimitQuota]) -> Result<(), RateLimitStoreError> {
    if quotas.is_empty() {
        return Err(RateLimitStoreError::Invalid(
            "at least one quota is required".into(),
        ));
    }
    if quotas
        .iter()
        .any(|quota| quota.key.is_empty() || quota.limit == 0 || quota.window_seconds == 0)
    {
        return Err(RateLimitStoreError::Invalid(
            "quota key, limit, and window must be non-zero".into(),
        ));
    }
    if quotas
        .iter()
        .enumerate()
        .any(|(index, quota)| quotas[..index].iter().any(|prior| prior.key == quota.key))
    {
        return Err(RateLimitStoreError::Invalid(
            "quota keys must be unique".into(),
        ));
    }
    Ok(())
}

fn decision_from_result(result: Vec<i64>) -> Result<RateLimitDecision, RateLimitStoreError> {
    if result.len() != 5 || !matches!(result[0], 0 | 1) {
        return Err(RateLimitStoreError::Invalid(
            "Valkey rate-limit script returned an invalid result".into(),
        ));
    }

    let decision = RateLimitDecision {
        allowed: result[0] == 1,
        retry_after_seconds: u64::try_from(result[1])
            .map_err(|_| RateLimitStoreError::Invalid("negative retry-after from Valkey".into()))?,
        remaining: u64::try_from(result[2]).map_err(|_| {
            RateLimitStoreError::Invalid("negative remaining count from Valkey".into())
        })?,
        limit: u64::try_from(result[3])
            .map_err(|_| RateLimitStoreError::Invalid("negative limit from Valkey".into()))?,
        reset_seconds: u64::try_from(result[4])
            .map_err(|_| RateLimitStoreError::Invalid("negative reset from Valkey".into()))?,
    };

    let result_is_consistent = decision.limit > 0
        && decision.reset_seconds > 0
        && decision.remaining <= decision.limit
        && if decision.allowed {
            decision.retry_after_seconds == 0
        } else {
            decision.retry_after_seconds > 0 && decision.remaining == 0
        };
    if !result_is_consistent {
        return Err(RateLimitStoreError::Invalid(
            "Valkey rate-limit script returned inconsistent fields".into(),
        ));
    }
    Ok(decision)
}

#[async_trait]
impl RateLimitStore for ValkeyRateLimitStore {
    async fn check(
        &self,
        quotas: &[RateLimitQuota],
    ) -> Result<RateLimitDecision, RateLimitStoreError> {
        validate_quotas(quotas)?;

        let mut connection = self.connection().await?;
        let script = Script::new(CHECK_SCRIPT);
        let mut invocation = script.prepare_invoke();
        for quota in quotas {
            invocation.key(&quota.key);
        }
        invocation.arg(quotas.len() as u64);
        for quota in quotas {
            invocation.arg(quota.limit).arg(quota.window_seconds);
        }

        let result: Vec<i64> =
            tokio::time::timeout(OPERATION_TIMEOUT, invocation.invoke_async(&mut connection))
                .await
                .map_err(|_| RateLimitStoreError::Unavailable("Valkey operation timed out".into()))?
                .map_err(classify_redis_error)?;

        decision_from_result(result)
    }
}

#[cfg(test)]
mod tests {
    use super::{decision_from_result, validate_quotas};
    use common::rate_limit::RateLimitQuota;

    #[test]
    fn empty_quota_list_is_rejected_before_valkey() {
        assert!(validate_quotas(&[]).is_err());
    }

    #[test]
    fn duplicate_keys_are_rejected_before_valkey() {
        let quotas = [
            RateLimitQuota::new("same", 1, 60),
            RateLimitQuota::new("same", 2, 60),
        ];
        assert!(validate_quotas(&quotas).is_err());
    }

    #[test]
    fn unknown_script_decision_flag_is_rejected() {
        let error = decision_from_result(vec![2, 0, 0, 1, 1]).unwrap_err();
        assert!(matches!(
            error,
            common::rate_limit::RateLimitStoreError::Invalid(_)
        ));
    }

    #[test]
    fn inconsistent_script_decision_fields_are_rejected() {
        let error = decision_from_result(vec![1, 4, 0, 1, 1]).unwrap_err();
        assert!(matches!(
            error,
            common::rate_limit::RateLimitStoreError::Invalid(_)
        ));
    }
}
