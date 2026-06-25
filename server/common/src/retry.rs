use std::{future::Future, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_retries: usize,
    initial_delay: Duration,
    backoff_multiplier: u32,
}

impl RetryPolicy {
    pub fn new(max_retries: usize, initial_delay: Duration, backoff_multiplier: u32) -> Self {
        Self {
            max_retries,
            initial_delay,
            backoff_multiplier,
        }
    }

    pub fn max_attempts(&self) -> usize {
        self.max_retries + 1
    }

    pub fn delay_for_retry(&self, retry_index: usize) -> Duration {
        (0..retry_index).fold(self.initial_delay, |delay, _| {
            delay.saturating_mul(self.backoff_multiplier)
        })
    }

    fn should_retry_after_attempt(&self, attempt_index: usize) -> bool {
        attempt_index < self.max_retries
    }
}

pub async fn retry_async<T, E, Operation, OperationFuture>(
    policy: RetryPolicy,
    operation: Operation,
) -> Result<T, E>
where
    Operation: FnMut(usize) -> OperationFuture,
    OperationFuture: Future<Output = Result<T, E>>,
{
    retry_async_with_sleeper(policy, operation, tokio::time::sleep).await
}

pub async fn retry_async_with_sleeper<T, E, Operation, OperationFuture, Sleeper, SleepFuture>(
    policy: RetryPolicy,
    mut operation: Operation,
    mut sleeper: Sleeper,
) -> Result<T, E>
where
    Operation: FnMut(usize) -> OperationFuture,
    OperationFuture: Future<Output = Result<T, E>>,
    Sleeper: FnMut(Duration) -> SleepFuture,
    SleepFuture: Future<Output = ()>,
{
    let mut attempt_index = 0;

    loop {
        match operation(attempt_index).await {
            Ok(value) => return Ok(value),
            Err(_) if policy.should_retry_after_attempt(attempt_index) => {
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
    use std::{
        cell::RefCell,
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    #[tokio::test]
    async fn retry_async_does_not_retry_when_first_attempt_succeeds() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let delays = Rc::new(RefCell::new(vec![]));
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
                let delays = Rc::clone(&delays);
                move |delay| {
                    let delays = Rc::clone(&delays);
                    async move {
                        delays.borrow_mut().push(delay);
                    }
                }
            },
        )
        .await;

        assert_eq!(result, Ok("ok"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert!(delays.borrow().is_empty());
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
