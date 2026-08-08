use std::time::Duration;

use common::rate_limit::{RateLimitQuota, RateLimitStore};
use resource::rate_limit::ValkeyRateLimitStore;
use uuid::Uuid;

fn test_key(name: &str) -> String {
    format!("rl:integration:{}:{name}", Uuid::new_v4())
}

/// Run this test with a real Valkey instance:
///
/// `RATE_LIMIT_VALKEY_URL=redis://127.0.0.1:6379/ cargo test -p resource --test rate_limit_valkey -- --ignored`
#[tokio::test]
#[ignore = "requires a Valkey instance; executed by the CI Valkey job"]
async fn valkey_rate_limit_is_atomic_shared_and_fixed_window() {
    let url = std::env::var("RATE_LIMIT_VALKEY_URL")
        .expect("RATE_LIMIT_VALKEY_URL must be set for the Valkey integration test");
    let first_store = ValkeyRateLimitStore::from_url(url.clone()).unwrap();
    let second_store = ValkeyRateLimitStore::from_url(url).unwrap();

    // Separate store instances observe the same shared counter.
    let shared_key = test_key("shared");
    let quota = RateLimitQuota::new(shared_key, 2, 60);
    assert!(
        first_store
            .check(std::slice::from_ref(&quota))
            .await
            .unwrap()
            .allowed
    );
    let second = second_store
        .check(std::slice::from_ref(&quota))
        .await
        .unwrap();
    assert!(second.allowed);
    assert_eq!(second.remaining, 0);
    assert!(
        !first_store
            .check(std::slice::from_ref(&quota))
            .await
            .unwrap()
            .allowed
    );

    // If one temporary quota is exhausted, the other quota is not consumed.
    let exhausted_key = test_key("exhausted");
    let untouched_key = test_key("untouched");
    let exhausted = RateLimitQuota::new(exhausted_key, 1, 60);
    let untouched = RateLimitQuota::new(untouched_key, 1, 60);
    assert!(
        first_store
            .check(std::slice::from_ref(&exhausted))
            .await
            .unwrap()
            .allowed
    );
    let denied = first_store
        .check(&[exhausted.clone(), untouched.clone()])
        .await
        .unwrap();
    assert!(!denied.allowed);
    assert!(
        first_store
            .check(std::slice::from_ref(&untouched))
            .await
            .unwrap()
            .allowed
    );

    // The server-side fixed-window decision includes a positive retry TTL and
    // permits a new request after the window has elapsed.
    let window_quota = RateLimitQuota::new(test_key("window"), 1, 2);
    assert!(
        first_store
            .check(std::slice::from_ref(&window_quota))
            .await
            .unwrap()
            .allowed
    );
    let denied = first_store
        .check(std::slice::from_ref(&window_quota))
        .await
        .unwrap();
    assert!(!denied.allowed);
    assert!(denied.retry_after_seconds > 0);
    tokio::time::sleep(Duration::from_secs(denied.retry_after_seconds + 1)).await;
    assert!(
        first_store
            .check(std::slice::from_ref(&window_quota))
            .await
            .unwrap()
            .allowed
    );
}
