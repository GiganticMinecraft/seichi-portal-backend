use std::{future::Future, time::Duration};

/// 非同期処理のリトライ回数と待機時間を表すポリシー。
///
/// 初回実行はリトライ回数に含めず、`max_retries` 回まで再実行します。
/// 待機時間は `initial_delay` から始まり、リトライごとに
/// `backoff_multiplier` 倍されます。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_retries: usize,
    initial_delay: Duration,
    backoff_multiplier: u32,
}

impl RetryPolicy {
    /// リトライポリシーを作成します。
    pub fn new(max_retries: usize, initial_delay: Duration, backoff_multiplier: u32) -> Self {
        Self {
            max_retries,
            initial_delay,
            backoff_multiplier,
        }
    }

    /// 初回実行を含めた最大試行回数を返します。
    pub fn max_attempts(&self) -> usize {
        self.max_retries + 1
    }

    /// 指定したリトライの前に待機する時間を返します。
    ///
    /// `retry_index` は 0 始まりで、最初のリトライでは `initial_delay` を返します。
    pub fn delay_for_retry(&self, retry_index: usize) -> Duration {
        (0..retry_index).fold(self.initial_delay, |delay, _| {
            delay.saturating_mul(self.backoff_multiplier)
        })
    }

    fn should_retry_after_attempt(&self, attempt_index: usize) -> bool {
        attempt_index < self.max_retries
    }
}

/// 失敗した非同期処理をポリシーに従ってリトライします。
///
/// すべてのエラーをリトライ対象として扱います。`operation` には 0 始まりの
/// 試行回数が渡されます。
pub async fn retry_async<T, E, Operation, OperationFuture>(
    policy: RetryPolicy,
    operation: Operation,
) -> Result<T, E>
where
    T: Send,
    E: Send,
    Operation: Fn(usize) -> OperationFuture + Send + Sync,
    OperationFuture: Future<Output = Result<T, E>> + Send,
{
    retry_async_if(policy, operation, |_| true).await
}

/// リトライ対象のエラーを判定しながら、失敗した非同期処理をリトライします。
///
/// `should_retry` が `false` を返したエラーは即座に返します。`operation` には
/// 0 始まりの試行回数が渡されます。
pub async fn retry_async_if<T, E, Operation, OperationFuture, ShouldRetry>(
    policy: RetryPolicy,
    operation: Operation,
    should_retry: ShouldRetry,
) -> Result<T, E>
where
    T: Send,
    E: Send,
    Operation: Fn(usize) -> OperationFuture + Send + Sync,
    OperationFuture: Future<Output = Result<T, E>> + Send,
    ShouldRetry: Fn(&E) -> bool + Send + Sync,
{
    retry_async_with_sleeper_if(policy, operation, tokio::time::sleep, should_retry).await
}

/// 待機処理を差し替えて、失敗した非同期処理をリトライします。
///
/// 主にテストで実時間の sleep を避けたい場合に使用します。すべてのエラーを
/// リトライ対象として扱います。
pub async fn retry_async_with_sleeper<T, E, Operation, OperationFuture, Sleeper, SleepFuture>(
    policy: RetryPolicy,
    operation: Operation,
    sleeper: Sleeper,
) -> Result<T, E>
where
    T: Send,
    E: Send,
    Operation: Fn(usize) -> OperationFuture + Send + Sync,
    OperationFuture: Future<Output = Result<T, E>> + Send,
    Sleeper: Fn(Duration) -> SleepFuture + Send + Sync,
    SleepFuture: Future<Output = ()> + Send,
{
    retry_async_with_sleeper_if(policy, operation, sleeper, |_| true).await
}

/// 待機処理とリトライ可否判定を差し替えて、失敗した非同期処理をリトライします。
///
/// `operation` には 0 始まりの試行回数が渡されます。リトライ上限に達した場合、
/// または `should_retry` が `false` を返した場合は、その時点のエラーを返します。
pub async fn retry_async_with_sleeper_if<
    T,
    E,
    Operation,
    OperationFuture,
    Sleeper,
    SleepFuture,
    ShouldRetry,
>(
    policy: RetryPolicy,
    operation: Operation,
    sleeper: Sleeper,
    should_retry: ShouldRetry,
) -> Result<T, E>
where
    T: Send,
    E: Send,
    Operation: Fn(usize) -> OperationFuture + Send + Sync,
    OperationFuture: Future<Output = Result<T, E>> + Send,
    Sleeper: Fn(Duration) -> SleepFuture + Send + Sync,
    SleepFuture: Future<Output = ()> + Send,
    ShouldRetry: Fn(&E) -> bool + Send + Sync,
{
    let mut attempt_index = 0;

    loop {
        match operation(attempt_index).await {
            Ok(value) => return Ok(value),
            Err(error)
                if policy.should_retry_after_attempt(attempt_index) && should_retry(&error) =>
            {
                sleeper(policy.delay_for_retry(attempt_index)).await;
                attempt_index += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn retry_async_does_not_retry_when_first_attempt_succeeds() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let sleeps = Arc::new(AtomicUsize::new(0));
        let result = retry_async_with_sleeper(
            RetryPolicy::new(5, Duration::from_secs(1), 2),
            {
                let attempts = Arc::clone(&attempts);
                move |_| {
                    let attempts = Arc::clone(&attempts);
                    async move {
                        attempts.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, &'static str>("ok")
                    }
                }
            },
            {
                let sleeps = Arc::clone(&sleeps);
                move |_| {
                    let sleeps = Arc::clone(&sleeps);
                    async move {
                        sleeps.fetch_add(1, Ordering::SeqCst);
                    }
                }
            },
        )
        .await;

        assert_eq!(result, Ok("ok"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(sleeps.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn retry_async_stops_when_retry_succeeds() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let result = retry_async_with_sleeper(
            RetryPolicy::new(5, Duration::from_secs(1), 2),
            {
                let attempts = Arc::clone(&attempts);
                move |_| {
                    let attempts = Arc::clone(&attempts);
                    async move {
                        let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                        match attempt {
                            0 | 1 => Err("failed"),
                            _ => Ok("ok"),
                        }
                    }
                }
            },
            |_| async {},
        )
        .await;

        assert_eq!(result, Ok("ok"));
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_async_returns_last_error_after_all_retries_fail() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let result = retry_async_with_sleeper(
            RetryPolicy::new(5, Duration::from_secs(1), 2),
            {
                let attempts = Arc::clone(&attempts);
                move |_| {
                    let attempts = Arc::clone(&attempts);
                    async move {
                        let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                        Err::<(), _>(attempt)
                    }
                }
            },
            |_| async {},
        )
        .await;

        assert_eq!(result, Err(5));
        assert_eq!(attempts.load(Ordering::SeqCst), 6);
    }

    #[tokio::test]
    async fn retry_async_does_not_retry_when_error_is_not_retryable() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let result = retry_async_with_sleeper_if(
            RetryPolicy::new(5, Duration::from_secs(1), 2),
            {
                let attempts = Arc::clone(&attempts);
                move |_| {
                    let attempts = Arc::clone(&attempts);
                    async move {
                        attempts.fetch_add(1, Ordering::SeqCst);
                        Err::<(), _>("fatal")
                    }
                }
            },
            |_| async {},
            |error| error != &"fatal",
        )
        .await;

        assert_eq!(result, Err("fatal"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn retry_policy_calculates_exponential_backoff_delays() {
        let policy = RetryPolicy::new(5, Duration::from_secs(1), 2);
        let delays = (0..5)
            .map(|retry_index| policy.delay_for_retry(retry_index))
            .collect::<Vec<_>>();

        assert_eq!(
            delays,
            vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(4),
                Duration::from_secs(8),
                Duration::from_secs(16),
            ]
        );
    }
}
