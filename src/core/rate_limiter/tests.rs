//! Tests for rate limiter

#[cfg(test)]
use super::limiter::RateLimiter;
#[cfg(feature = "gateway")]
use super::limiter::{
    RedisRateLimitBackend, degraded_metric_count_for_tests, render_degraded_metrics,
    reset_degraded_metrics_for_tests,
};
use super::types::RateLimitEntry;
use super::{RateLimitRecordSource, RateLimitReservation};
use crate::config::models::rate_limit::{RateLimitConfig, RateLimitStrategy, RedisFailureMode};
#[cfg(feature = "gateway")]
use crate::utils::error::gateway_error::{GatewayError, Result};
#[cfg(feature = "gateway")]
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "gateway")]
static REDIS_METRICS_TEST_LOCK: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

#[cfg(feature = "gateway")]
struct FailingRedisBackend;

#[cfg(feature = "gateway")]
#[async_trait::async_trait]
impl RedisRateLimitBackend for FailingRedisBackend {
    async fn rate_limit_status(
        &self,
        _key: &str,
        _limit: u32,
        _window_secs: u64,
    ) -> Result<super::RateLimitResult> {
        Err(GatewayError::Storage("redis unavailable".to_string()))
    }

    async fn rate_limit_check_and_record(
        &self,
        _key: &str,
        _limit: u32,
        _window_secs: u64,
    ) -> Result<super::RateLimitResult> {
        Err(GatewayError::Storage("redis unavailable".to_string()))
    }

    async fn rate_limit_release(&self, _key: &str, _reservation_ttl_secs: u64) -> Result<()> {
        Err(GatewayError::Storage("redis unavailable".to_string()))
    }

    fn is_noop(&self) -> bool {
        false
    }
}

fn test_config(enabled: bool, rpm: u32) -> RateLimitConfig {
    test_config_with_strategy(enabled, rpm, RateLimitStrategy::SlidingWindow)
}

fn test_config_with_strategy(
    enabled: bool,
    rpm: u32,
    strategy: RateLimitStrategy,
) -> RateLimitConfig {
    RateLimitConfig {
        enabled,
        default_rpm: rpm,
        default_tpm: 100000,
        strategy,
        ..Default::default()
    }
}

fn test_config_with_requests_per_minute_alias(enabled: bool, rpm: u32) -> RateLimitConfig {
    RateLimitConfig {
        enabled,
        default_rpm: 1000,
        requests_per_minute: Some(rpm),
        default_tpm: 100000,
        strategy: RateLimitStrategy::SlidingWindow,
        ..Default::default()
    }
}

#[cfg(feature = "gateway")]
fn limiter_with_failing_redis(config: RateLimitConfig) -> RateLimiter {
    RateLimiter::with_redis_backend(config, Arc::new(FailingRedisBackend))
}

#[tokio::test]
async fn test_rate_limiter_disabled() {
    let limiter = RateLimiter::new(test_config(false, 10));

    for _ in 0..100 {
        let result = limiter.check_and_record("test-key").await;
        assert!(result.allowed);
    }
}

#[tokio::test]
async fn test_sliding_window_allows_within_limit() {
    let limiter = RateLimiter::new(test_config(true, 10));

    for i in 0..10 {
        let result = limiter.check_and_record("test-key").await;
        assert!(result.allowed, "Request {} should be allowed", i);
    }
}

#[tokio::test]
async fn test_sliding_window_blocks_over_limit() {
    let limiter = RateLimiter::new(test_config(true, 5));

    // Fill up the limit using atomic check_and_record
    for _ in 0..5 {
        let result = limiter.check_and_record("test-key").await;
        assert!(result.allowed);
    }

    // This should be blocked
    let result = limiter.check_and_record("test-key").await;
    assert!(!result.allowed);
    assert!(result.retry_after_secs.is_some());
}

#[tokio::test]
async fn test_requests_per_minute_alias_is_enforced() {
    let limiter = RateLimiter::new(test_config_with_requests_per_minute_alias(true, 2));

    let first = limiter.check_and_record("alias-key").await;
    let second = limiter.check_and_record("alias-key").await;
    let third = limiter.check_and_record("alias-key").await;

    assert!(first.allowed);
    assert_eq!(first.limit, 2);
    assert!(second.allowed);
    assert!(!third.allowed);
    assert_eq!(third.limit, 2);
    assert_eq!(limiter.limit(), 2);
}

#[tokio::test]
async fn test_different_keys_independent() {
    let limiter = RateLimiter::new(test_config(true, 2));

    // Fill up limit for key1 using atomic method
    limiter.check_and_record("key1").await;
    limiter.check_and_record("key1").await;

    // key1 should be blocked
    let result = limiter.check_and_record("key1").await;
    assert!(!result.allowed);

    // key2 should still work
    let result = limiter.check_and_record("key2").await;
    assert!(result.allowed);
}

#[tokio::test]
async fn test_token_bucket() {
    let config = RateLimitConfig {
        enabled: true,
        default_rpm: 60, // 1 per second
        default_tpm: 100000,
        strategy: RateLimitStrategy::TokenBucket,
        ..Default::default()
    };
    let limiter = RateLimiter::new(config);

    // Should allow initial requests (bucket starts full)
    let result = limiter.check_and_record("test-key").await;
    assert!(result.allowed);
}

#[tokio::test]
async fn test_fixed_window() {
    let config = RateLimitConfig {
        enabled: true,
        default_rpm: 5,
        default_tpm: 100000,
        strategy: RateLimitStrategy::FixedWindow,
        ..Default::default()
    };
    let limiter = RateLimiter::new(config);

    for _ in 0..5 {
        let result = limiter.check_and_record("test-key").await;
        assert!(result.allowed);
    }

    // Should be blocked
    let result = limiter.check_and_record("test-key").await;
    assert!(!result.allowed);
}

#[tokio::test]
async fn test_remaining_count() {
    let limiter = RateLimiter::new(test_config(true, 5));

    // First check (no record) should show 5 remaining
    let result = limiter.check("test-key").await;
    assert_eq!(result.remaining, 5);

    // After check_and_record, remaining should be 4
    let result = limiter.check_and_record("test-key").await;
    assert_eq!(result.remaining, 4);

    // Do two more
    limiter.check_and_record("test-key").await;
    limiter.check_and_record("test-key").await;

    // Should have 2 remaining
    let result = limiter.check("test-key").await;
    assert_eq!(result.remaining, 2);
}

#[tokio::test]
async fn test_atomic_check_and_record() {
    let limiter = RateLimiter::new(test_config(true, 3));

    // Use atomic method - should record and decrement in one operation
    let r1 = limiter.check_and_record("atomic-key").await;
    assert!(r1.allowed);
    assert_eq!(r1.remaining, 2); // 3-1=2 after recording

    let r2 = limiter.check_and_record("atomic-key").await;
    assert!(r2.allowed);
    assert_eq!(r2.remaining, 1);

    let r3 = limiter.check_and_record("atomic-key").await;
    assert!(r3.allowed);
    assert_eq!(r3.remaining, 0);

    // 4th request should be blocked
    let r4 = limiter.check_and_record("atomic-key").await;
    assert!(!r4.allowed);
}

#[tokio::test]
async fn test_check_and_record_with_source_and_limit_uses_override_rpm() {
    let limiter = RateLimiter::new(test_config(true, 10));

    let (first, _) = limiter
        .check_and_record_with_source_and_limit("api-key", 1)
        .await;
    let (second, _) = limiter
        .check_and_record_with_source_and_limit("api-key", 1)
        .await;

    assert!(first.allowed);
    assert!(!second.allowed);
    assert_eq!(second.limit, 1);
}

#[cfg(feature = "gateway")]
#[tokio::test]
async fn redis_check_failure_defaults_to_fail_closed() {
    let _guard = REDIS_METRICS_TEST_LOCK.lock().await;
    reset_degraded_metrics_for_tests();
    let limiter = limiter_with_failing_redis(test_config(true, 3));

    let result = limiter.check("redis-key").await;

    assert!(!result.allowed);
    assert_eq!(result.limit, 3);
    assert_eq!(result.remaining, 0);
    assert_eq!(degraded_metric_count_for_tests("check", "fail_closed"), 1);
    assert!(!limiter.entries.contains_key("redis-key"));
}

#[cfg(feature = "gateway")]
#[tokio::test]
async fn redis_check_and_record_failure_defaults_to_fail_closed_without_local_reservation() {
    let _guard = REDIS_METRICS_TEST_LOCK.lock().await;
    reset_degraded_metrics_for_tests();
    let limiter = limiter_with_failing_redis(test_config(true, 3));

    let (result, reservation) = limiter.check_and_record_with_source("redis-key").await;

    assert!(!result.allowed);
    assert_eq!(reservation.source(), RateLimitRecordSource::Disabled);
    assert_eq!(
        degraded_metric_count_for_tests("check_and_record", "fail_closed"),
        1
    );
    assert!(!limiter.entries.contains_key("redis-key"));
}

#[cfg(feature = "gateway")]
#[tokio::test]
async fn redis_check_and_record_failure_can_explicitly_fail_open_local() {
    let _guard = REDIS_METRICS_TEST_LOCK.lock().await;
    reset_degraded_metrics_for_tests();
    let config = RateLimitConfig {
        redis_failure_mode: RedisFailureMode::FailOpenLocal,
        ..test_config(true, 1)
    };
    let limiter = limiter_with_failing_redis(config);

    let (allowed, reservation) = limiter.check_and_record_with_source("redis-key").await;
    let blocked_by_local = limiter.check_and_record("redis-key").await;

    assert!(allowed.allowed);
    assert_eq!(reservation.source(), RateLimitRecordSource::Local);
    assert!(!blocked_by_local.allowed);
    assert_eq!(
        degraded_metric_count_for_tests("check_and_record", "fail_open_local"),
        2
    );
    assert!(limiter.entries.contains_key("redis-key"));
}

#[cfg(feature = "gateway")]
#[tokio::test]
async fn redis_release_failure_is_observable_and_nonfatal() {
    let _guard = REDIS_METRICS_TEST_LOCK.lock().await;
    reset_degraded_metrics_for_tests();
    let limiter = limiter_with_failing_redis(test_config(true, 1));

    limiter
        .release_recorded(
            "redis-key",
            RateLimitReservation::for_test(RateLimitRecordSource::Distributed, Instant::now(), 60),
        )
        .await;

    assert_eq!(degraded_metric_count_for_tests("release", "fail_closed"), 1);
}

#[cfg(feature = "gateway")]
#[tokio::test]
async fn redis_degraded_metric_renders_operation_and_mode_labels() {
    let _guard = REDIS_METRICS_TEST_LOCK.lock().await;
    reset_degraded_metrics_for_tests();
    let limiter = limiter_with_failing_redis(test_config(true, 1));

    let _ = limiter.check("redis-key").await;

    let rendered = render_degraded_metrics();
    assert!(rendered.contains("# TYPE rate_limiter_degraded_total counter"));
    assert!(
        rendered
            .contains("rate_limiter_degraded_total{operation=\"check\",mode=\"fail_closed\"} 1")
    );
}

#[tokio::test]
async fn test_release_recorded_local_source_restores_capacity() {
    let limiter = RateLimiter::new(test_config(true, 1));

    let (result, reservation) = limiter.check_and_record_with_source("auth-ip").await;
    assert!(result.allowed);
    assert_eq!(reservation.source(), RateLimitRecordSource::Local);

    let blocked = limiter.check_and_record("auth-ip").await;
    assert!(!blocked.allowed);

    limiter.release_recorded("auth-ip", reservation).await;

    let allowed_again = limiter.check_and_record("auth-ip").await;
    assert!(allowed_again.allowed);
}

#[tokio::test]
async fn test_denied_check_returns_disabled_reservation() {
    let limiter = RateLimiter::new(test_config(true, 1));

    let (first, first_reservation) = limiter.check_and_record_with_source("auth-ip").await;
    assert!(first.allowed);
    assert_eq!(first_reservation.source(), RateLimitRecordSource::Local);

    let (blocked, blocked_reservation) = limiter.check_and_record_with_source("auth-ip").await;
    assert!(!blocked.allowed);
    assert_eq!(
        blocked_reservation.source(),
        RateLimitRecordSource::Disabled
    );

    limiter
        .release_recorded("auth-ip", blocked_reservation)
        .await;

    let still_blocked = limiter.check_and_record("auth-ip").await;
    assert!(!still_blocked.allowed);
}

#[tokio::test]
async fn test_release_recorded_sliding_and_fixed_windows_remove_empty_entry() {
    for strategy in [
        RateLimitStrategy::SlidingWindow,
        RateLimitStrategy::FixedWindow,
    ] {
        let limiter = RateLimiter::new(test_config_with_strategy(true, 1, strategy));

        let (result, reservation) = limiter.check_and_record_with_source("auth-ip").await;
        assert!(result.allowed);
        assert!(limiter.entries.contains_key("auth-ip"));

        limiter.release_recorded("auth-ip", reservation).await;

        assert!(!limiter.entries.contains_key("auth-ip"));
    }
}

#[tokio::test]
async fn test_release_recorded_sliding_window_uses_recorded_timestamp_lifetime() {
    let limiter = RateLimiter::with_window(
        test_config_with_strategy(true, 2, RateLimitStrategy::SlidingWindow),
        Duration::from_secs(2),
    );
    let key = "auth-ip";
    let now = Instant::now();
    limiter.entries.insert(
        key.to_string(),
        RateLimitEntry {
            timestamps: vec![now - Duration::from_millis(1500)],
            tokens: 0.0,
            last_refill: now,
        },
    );

    let (result, reservation) = limiter.check_and_record_with_source(key).await;
    assert!(result.allowed);

    tokio::time::sleep(Duration::from_millis(1100)).await;
    limiter.release_recorded(key, reservation).await;

    let first_after_release = limiter.check_and_record(key).await;
    let second_after_release = limiter.check_and_record(key).await;

    assert!(first_after_release.allowed);
    assert!(
        second_after_release.allowed,
        "release must remove the successful auth reservation for its full sliding window lifetime"
    );
}

#[tokio::test]
async fn test_release_recorded_token_bucket_restores_fresh_reservation() {
    let limiter = RateLimiter::new(test_config_with_strategy(
        true,
        1,
        RateLimitStrategy::TokenBucket,
    ));

    let (result, reservation) = limiter.check_and_record_with_source("auth-ip").await;
    assert!(result.allowed);

    let blocked = limiter.check_and_record("auth-ip").await;
    assert!(!blocked.allowed);

    limiter.release_recorded("auth-ip", reservation).await;

    let allowed_again = limiter.check_and_record("auth-ip").await;
    assert!(allowed_again.allowed);
}

#[tokio::test]
async fn test_release_recorded_token_bucket_keeps_burst_reservations_releasable() {
    let limiter = RateLimiter::new(test_config_with_strategy(
        true,
        60,
        RateLimitStrategy::TokenBucket,
    ));
    let key = "auth-ip";
    let original_success = Instant::now() - Duration::from_secs(2);

    limiter.entries.insert(
        key.to_string(),
        RateLimitEntry {
            timestamps: vec![original_success],
            tokens: 0.0,
            last_refill: Instant::now(),
        },
    );

    limiter
        .release_recorded(
            key,
            RateLimitReservation::for_test(RateLimitRecordSource::Local, original_success, 60),
        )
        .await;

    let allowed_again = limiter.check_and_record(key).await;
    assert!(
        allowed_again.allowed,
        "burst reservations must stay releasable beyond one token refill interval"
    );
}

#[tokio::test]
async fn test_release_recorded_token_bucket_skips_expired_reservation() {
    let limiter = RateLimiter::new(test_config_with_strategy(
        true,
        60,
        RateLimitStrategy::TokenBucket,
    ));
    let key = "auth-ip";
    let original_success = Instant::now() - Duration::from_secs(2);
    let later_rejected_auth = Instant::now();

    limiter.entries.insert(
        key.to_string(),
        RateLimitEntry {
            timestamps: vec![original_success, later_rejected_auth],
            tokens: 0.0,
            last_refill: later_rejected_auth,
        },
    );

    limiter
        .release_recorded(
            key,
            RateLimitReservation::for_test(RateLimitRecordSource::Local, original_success, 1),
        )
        .await;

    let blocked = limiter.check_and_record(key).await;
    assert!(
        !blocked.allowed,
        "expired success release must not refund a later rejected-auth token"
    );
}

#[tokio::test]
async fn test_release_recorded_fixed_window_preserves_window_anchor() {
    let limiter = RateLimiter::with_window(
        test_config_with_strategy(true, 1, RateLimitStrategy::FixedWindow),
        Duration::from_millis(200),
    );
    let key = "auth-ip";
    let window_start = Instant::now() - Duration::from_millis(150);
    let later_hit = window_start + Duration::from_millis(50);

    limiter.entries.insert(
        key.to_string(),
        RateLimitEntry {
            timestamps: vec![window_start, later_hit],
            tokens: 0.0,
            last_refill: window_start,
        },
    );

    limiter
        .release_recorded(
            key,
            RateLimitReservation::for_test(RateLimitRecordSource::Local, window_start, 1),
        )
        .await;

    tokio::time::sleep(Duration::from_millis(70)).await;
    let allowed_after_original_window = limiter.check_and_record(key).await;

    assert!(
        allowed_after_original_window.allowed,
        "fixed-window releases must not move the remaining request's window anchor"
    );
}

#[tokio::test]
async fn test_release_recorded_distributed_source_keeps_local_capacity() {
    let limiter = RateLimiter::new(test_config(true, 1));

    let result = limiter.check_and_record("auth-ip").await;
    assert!(result.allowed);

    limiter
        .release_recorded(
            "auth-ip",
            RateLimitReservation::for_test(RateLimitRecordSource::Distributed, Instant::now(), 60),
        )
        .await;

    let blocked = limiter.check_and_record("auth-ip").await;
    assert!(!blocked.allowed);
}

#[tokio::test]
async fn test_cleanup() {
    let limiter = RateLimiter::with_window(test_config(true, 100), Duration::from_millis(50));

    // Use atomic method
    limiter.check_and_record("key1").await;
    limiter.check_and_record("key2").await;

    // Wait for window to expire
    tokio::time::sleep(Duration::from_millis(100)).await;

    limiter.cleanup().await;

    // After cleanup, should have full limit again
    let result = limiter.check("key1").await;
    assert!(result.allowed);
    assert_eq!(result.remaining, 100);
}
