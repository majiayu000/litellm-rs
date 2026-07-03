//! Core rate limiter implementation

use super::types::{RateLimitEntry, RateLimitResult};
use crate::config::models::rate_limit::{RateLimitConfig, RateLimitStrategy, RedisFailureMode};
#[cfg(feature = "gateway")]
use crate::utils::error::gateway_error::Result;
#[cfg(feature = "gateway")]
use async_trait::async_trait;
use dashmap::DashMap;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tracing::error;

static REDIS_DEGRADED_METRICS: LazyLock<
    parking_lot::Mutex<BTreeMap<(&'static str, &'static str), u64>>,
> = LazyLock::new(|| parking_lot::Mutex::new(BTreeMap::new()));

#[derive(Debug, Clone, Copy)]
enum RedisRateLimitOperation {
    Check,
    CheckAndRecord,
    Release,
}

impl RedisRateLimitOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::CheckAndRecord => "check_and_record",
            Self::Release => "release",
        }
    }
}

#[cfg(feature = "gateway")]
#[async_trait]
pub(crate) trait RedisRateLimitBackend: Send + Sync {
    async fn rate_limit_status(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> Result<RateLimitResult>;

    async fn rate_limit_check_and_record(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> Result<RateLimitResult>;

    async fn rate_limit_release(&self, key: &str, reservation_ttl_secs: u64) -> Result<()>;

    fn is_noop(&self) -> bool;
}

#[cfg(feature = "gateway")]
#[async_trait]
impl RedisRateLimitBackend for crate::storage::redis::RedisPool {
    async fn rate_limit_status(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> Result<RateLimitResult> {
        crate::storage::redis::RedisPool::rate_limit_status(self, key, limit, window_secs).await
    }

    async fn rate_limit_check_and_record(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> Result<RateLimitResult> {
        crate::storage::redis::RedisPool::rate_limit_check_and_record(self, key, limit, window_secs)
            .await
    }

    async fn rate_limit_release(&self, key: &str, reservation_ttl_secs: u64) -> Result<()> {
        crate::storage::redis::RedisPool::rate_limit_release(self, key, reservation_ttl_secs).await
    }

    fn is_noop(&self) -> bool {
        self.is_noop()
    }
}

fn record_redis_degraded(
    operation: RedisRateLimitOperation,
    mode: RedisFailureMode,
    key: &str,
    err: &impl fmt::Display,
) {
    let operation = operation.as_str();
    let mode = mode.as_str();

    error!(
        operation,
        mode,
        key,
        error = %err,
        "Redis distributed rate limiter degraded"
    );

    let mut metrics = REDIS_DEGRADED_METRICS.lock();
    *metrics.entry((operation, mode)).or_insert(0) += 1;
}

pub fn render_degraded_metrics() -> String {
    let metrics = REDIS_DEGRADED_METRICS.lock();
    let mut rendered = String::from(
        "# HELP rate_limiter_degraded_total Redis distributed rate limiter degraded operations\n\
         # TYPE rate_limiter_degraded_total counter\n",
    );

    for ((operation, mode), value) in metrics.iter() {
        rendered.push_str(&format!(
            "rate_limiter_degraded_total{{operation=\"{operation}\",mode=\"{mode}\"}} {value}\n"
        ));
    }

    rendered
}

#[cfg(test)]
pub(crate) fn reset_degraded_metrics_for_tests() {
    REDIS_DEGRADED_METRICS.lock().clear();
}

#[cfg(test)]
pub(crate) fn degraded_metric_count_for_tests(operation: &str, mode: &str) -> u64 {
    REDIS_DEGRADED_METRICS
        .lock()
        .get(&(operation, mode))
        .copied()
        .unwrap_or(0)
}

/// Backend that recorded a rate-limit reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RateLimitRecordSource {
    Disabled,
    Local,
    Distributed,
}

/// A concrete rate-limit record that may later be released.
///
/// The auth middleware consumes this in the follow-up split PR; keep the
/// primitive here so the core limiter behavior can land separately.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct RateLimitReservation {
    source: RateLimitRecordSource,
    recorded_at: Instant,
    expires_at: Option<Instant>,
}

#[allow(dead_code)]
impl RateLimitReservation {
    fn new(source: RateLimitRecordSource, recorded_at: Instant, reset_after_secs: u64) -> Self {
        let expires_at = match source {
            RateLimitRecordSource::Disabled => None,
            RateLimitRecordSource::Local | RateLimitRecordSource::Distributed => {
                Some(recorded_at + Duration::from_secs(reset_after_secs.max(1)))
            }
        };

        Self {
            source,
            recorded_at,
            expires_at,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        source: RateLimitRecordSource,
        recorded_at: Instant,
        reset_after_secs: u64,
    ) -> Self {
        Self::new(source, recorded_at, reset_after_secs)
    }

    #[cfg(test)]
    pub(crate) fn source(&self) -> RateLimitRecordSource {
        self.source
    }

    fn remaining_window_secs(&self) -> u64 {
        let Some(expires_at) = self.expires_at else {
            return 0;
        };

        let remaining = expires_at.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            0
        } else {
            remaining.as_secs().max(1)
        }
    }
}

/// Rate limiter implementation
pub struct RateLimiter {
    /// Rate limit configuration
    pub(super) config: RateLimitConfig,
    /// Rate limit entries by key (IP or API key) — per-key lock granularity
    pub(super) entries: Arc<DashMap<String, RateLimitEntry>>,
    /// Window duration
    pub(super) window: Duration,
    /// Optional distributed Redis backend
    #[cfg(feature = "gateway")]
    pub(super) redis: Option<Arc<dyn RedisRateLimitBackend>>,
}

impl RateLimiter {
    pub(super) fn token_bucket_reservation_window(&self) -> Duration {
        self.window
    }

    fn local_reservation_reset_after_secs(&self, result: &RateLimitResult) -> u64 {
        match self.config.strategy {
            RateLimitStrategy::TokenBucket => self.token_bucket_reservation_window().as_secs(),
            RateLimitStrategy::SlidingWindow => self.window.as_secs(),
            RateLimitStrategy::FixedWindow => result.reset_after_secs,
        }
    }

    fn disabled_result(&self) -> RateLimitResult {
        let limit = self.config.effective_rpm();
        RateLimitResult {
            allowed: true,
            current_count: 0,
            limit,
            remaining: limit,
            reset_after_secs: 0,
            retry_after_secs: None,
        }
    }

    /// Names of config fields that are parsed but not enforced by any
    /// strategy yet; only enabled/strategy/default_rpm/requests_per_minute take effect.
    fn unimplemented_field_names(config: &RateLimitConfig) -> Vec<&'static str> {
        config.unimplemented_runtime_field_names()
    }

    /// Surface dead configuration at error level without blocking startup
    /// so silently ignored limits are visible (no silent degradation).
    fn warn_unimplemented_fields(config: &RateLimitConfig) {
        let unimplemented = Self::unimplemented_field_names(config);
        if !unimplemented.is_empty() {
            error!(
                "rate_limit fields [{}] are set but not enforced yet; only enabled/strategy/default_rpm/requests_per_minute take effect",
                unimplemented.join(", ")
            );
        }
    }

    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self::with_window(config, Duration::from_secs(60)) // 1 minute window
    }

    /// Create a rate limiter with custom window
    pub fn with_window(config: RateLimitConfig, window: Duration) -> Self {
        Self::warn_unimplemented_fields(&config);

        Self {
            config,
            entries: Arc::new(DashMap::new()),
            window,
            #[cfg(feature = "gateway")]
            redis: None,
        }
    }

    /// Create a rate limiter backed by Redis for distributed enforcement.
    #[cfg(feature = "gateway")]
    pub fn with_redis(
        config: RateLimitConfig,
        redis: Arc<crate::storage::redis::RedisPool>,
    ) -> Self {
        Self::with_redis_backend(config, redis)
    }

    #[cfg(feature = "gateway")]
    pub(crate) fn with_redis_backend(
        config: RateLimitConfig,
        redis: Arc<dyn RedisRateLimitBackend>,
    ) -> Self {
        Self::warn_unimplemented_fields(&config);
        let redis = if redis.is_noop() { None } else { Some(redis) };

        Self {
            config,
            entries: Arc::new(DashMap::new()),
            window: Duration::from_secs(60),
            redis,
        }
    }

    fn redis_fail_closed_result(&self, limit: u32) -> RateLimitResult {
        let reset_after_secs = self.window.as_secs().max(1);
        RateLimitResult {
            allowed: false,
            current_count: limit,
            limit,
            remaining: 0,
            reset_after_secs,
            retry_after_secs: Some(reset_after_secs),
        }
    }

    /// Check if a request should be allowed (read-only, does not record)
    ///
    /// WARNING: Using check() followed by record() has a race condition.
    /// Use check_and_record() for atomic check-then-record operations.
    pub async fn check(&self, key: &str) -> RateLimitResult {
        if !self.config.enabled {
            return self.disabled_result();
        }

        #[cfg(feature = "gateway")]
        if let Some(redis) = &self.redis {
            match redis
                .rate_limit_status(key, self.config.effective_rpm(), self.window.as_secs())
                .await
            {
                Ok(result) => return result,
                Err(err) => {
                    record_redis_degraded(
                        RedisRateLimitOperation::Check,
                        self.config.redis_failure_mode,
                        key,
                        &err,
                    );
                    if self.config.redis_failure_mode == RedisFailureMode::FailClosed {
                        return self.redis_fail_closed_result(self.config.effective_rpm());
                    }
                }
            }
        }

        match self.config.strategy {
            RateLimitStrategy::SlidingWindow => {
                self.check_sliding_window_impl(
                    key,
                    self.config.effective_rpm(),
                    false,
                    Instant::now(),
                )
                .await
            }
            RateLimitStrategy::TokenBucket => {
                self.check_token_bucket_impl(
                    key,
                    self.config.effective_rpm(),
                    false,
                    Instant::now(),
                )
                .await
            }
            RateLimitStrategy::FixedWindow => {
                self.check_fixed_window_impl(
                    key,
                    self.config.effective_rpm(),
                    false,
                    Instant::now(),
                )
                .await
            }
        }
    }

    /// Atomically check and record a request (prevents TOCTOU race condition)
    ///
    /// This is the preferred method for rate limiting as it performs both
    /// the check and record operations in a single lock acquisition.
    pub async fn check_and_record(&self, key: &str) -> RateLimitResult {
        self.check_and_record_with_source(key).await.0
    }

    /// Atomically check and record a request, returning the backend used.
    pub(crate) async fn check_and_record_with_source(
        &self,
        key: &str,
    ) -> (RateLimitResult, RateLimitReservation) {
        self.check_and_record_with_source_and_limit(key, self.config.effective_rpm())
            .await
    }

    /// Atomically check and record a request with a per-request RPM limit.
    pub(crate) async fn check_and_record_with_source_and_limit(
        &self,
        key: &str,
        requests_per_minute: u32,
    ) -> (RateLimitResult, RateLimitReservation) {
        if !self.config.enabled {
            let recorded_at = Instant::now();
            return (
                self.disabled_result(),
                RateLimitReservation::new(RateLimitRecordSource::Disabled, recorded_at, 0),
            );
        }

        #[cfg(feature = "gateway")]
        if let Some(redis) = &self.redis {
            match redis
                .rate_limit_check_and_record(key, requests_per_minute, self.window.as_secs())
                .await
            {
                Ok(result) => {
                    let reservation = if result.allowed {
                        RateLimitReservation::new(
                            RateLimitRecordSource::Distributed,
                            Instant::now(),
                            result.reset_after_secs,
                        )
                    } else {
                        RateLimitReservation::new(
                            RateLimitRecordSource::Disabled,
                            Instant::now(),
                            0,
                        )
                    };
                    return (result, reservation);
                }
                Err(err) => {
                    record_redis_degraded(
                        RedisRateLimitOperation::CheckAndRecord,
                        self.config.redis_failure_mode,
                        key,
                        &err,
                    );
                    if self.config.redis_failure_mode == RedisFailureMode::FailClosed {
                        let result = self.redis_fail_closed_result(requests_per_minute);
                        return (
                            result,
                            RateLimitReservation::new(
                                RateLimitRecordSource::Disabled,
                                Instant::now(),
                                0,
                            ),
                        );
                    }
                }
            }
        }

        let recorded_at = Instant::now();
        let result = self
            .check_and_record_local(key, recorded_at, requests_per_minute)
            .await;
        let reservation = if result.allowed {
            let reset_after_secs = self.local_reservation_reset_after_secs(&result);
            RateLimitReservation::new(RateLimitRecordSource::Local, recorded_at, reset_after_secs)
        } else {
            RateLimitReservation::new(RateLimitRecordSource::Disabled, recorded_at, 0)
        };
        (result, reservation)
    }

    async fn check_and_record_local(
        &self,
        key: &str,
        recorded_at: Instant,
        requests_per_minute: u32,
    ) -> RateLimitResult {
        match self.config.strategy {
            RateLimitStrategy::SlidingWindow => {
                self.check_sliding_window_impl(key, requests_per_minute, true, recorded_at)
                    .await
            }
            RateLimitStrategy::TokenBucket => {
                self.check_token_bucket_impl(key, requests_per_minute, true, recorded_at)
                    .await
            }
            RateLimitStrategy::FixedWindow => {
                self.check_fixed_window_impl(key, requests_per_minute, true, recorded_at)
                    .await
            }
        }
    }

    /// Release one request previously reserved by the given backend.
    #[allow(dead_code)]
    pub(crate) async fn release_recorded(&self, key: &str, reservation: RateLimitReservation) {
        if !self.config.enabled {
            return;
        }

        match reservation.source {
            RateLimitRecordSource::Disabled => {}
            RateLimitRecordSource::Local => {
                if reservation.remaining_window_secs() == 0 {
                    return;
                }

                self.release_local(key, Some(reservation.recorded_at));
            }
            RateLimitRecordSource::Distributed => {
                let remaining_window_secs = reservation.remaining_window_secs();
                if remaining_window_secs == 0 {
                    return;
                }

                #[cfg(feature = "gateway")]
                if let Some(redis) = &self.redis
                    && let Err(err) = redis.rate_limit_release(key, remaining_window_secs).await
                {
                    record_redis_degraded(
                        RedisRateLimitOperation::Release,
                        self.config.redis_failure_mode,
                        key,
                        &err,
                    );
                }
            }
        }
    }

    fn release_local(&self, key: &str, recorded_at: Option<Instant>) {
        let limit = self.config.effective_rpm() as f64;
        let strategy = self.config.strategy.clone();
        let reservation_window = self.token_bucket_reservation_window();

        let _removed_entry = self.entries.remove_if_mut(key, |_, entry| {
            match &strategy {
                RateLimitStrategy::SlidingWindow | RateLimitStrategy::FixedWindow => {
                    if let Some(recorded_at) = recorded_at {
                        if let Some(position) =
                            entry.timestamps.iter().position(|&ts| ts == recorded_at)
                        {
                            entry.timestamps.remove(position);
                        }
                    } else {
                        entry.timestamps.pop();
                    }
                }
                RateLimitStrategy::TokenBucket => {
                    let should_refund = if let Some(recorded_at) = recorded_at {
                        let now = Instant::now();
                        entry
                            .timestamps
                            .retain(|&ts| now.saturating_duration_since(ts) < reservation_window);

                        if let Some(position) =
                            entry.timestamps.iter().position(|&ts| ts == recorded_at)
                        {
                            entry.timestamps.remove(position);
                            true
                        } else {
                            false
                        }
                    } else {
                        true
                    };

                    if should_refund {
                        entry.tokens = (entry.tokens + 1.0).min(limit);
                    }
                }
            }

            match &strategy {
                RateLimitStrategy::SlidingWindow | RateLimitStrategy::FixedWindow => {
                    entry.timestamps.is_empty()
                }
                RateLimitStrategy::TokenBucket => {
                    entry.timestamps.is_empty() && entry.tokens >= limit
                }
            }
        });
    }

    /// Record a request (increments counter)
    ///
    /// WARNING: This is a separate operation from check() and has a race condition.
    /// Use check_and_record() instead to avoid race conditions.
    #[deprecated(note = "Use check_and_record() instead to avoid race conditions")]
    pub async fn record(&self, key: &str) {
        if !self.config.enabled {
            return;
        }

        let mut entry = self.entries.entry(key.to_string()).or_default();

        match self.config.strategy {
            RateLimitStrategy::SlidingWindow | RateLimitStrategy::FixedWindow => {
                entry.timestamps.push(std::time::Instant::now());
            }
            RateLimitStrategy::TokenBucket => {
                // Token is consumed in check_token_bucket
            }
        }
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            entries: self.entries.clone(),
            window: self.window,
            #[cfg(feature = "gateway")]
            redis: self.redis.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unimplemented_field_names_default_empty() {
        let config = RateLimitConfig::default();
        assert!(RateLimiter::unimplemented_field_names(&config).is_empty());
    }

    #[test]
    fn test_unimplemented_field_names_lists_set_fields() {
        let config = RateLimitConfig {
            default_tpm: 50_000,
            requests_per_second: Some(10),
            tokens_per_minute: Some(60_000),
            burst_size: Some(20),
            ..RateLimitConfig::default()
        };
        assert_eq!(
            RateLimiter::unimplemented_field_names(&config),
            vec![
                "default_tpm",
                "requests_per_second",
                "tokens_per_minute",
                "burst_size"
            ]
        );
    }

    #[test]
    fn test_unimplemented_field_names_partial() {
        let config = RateLimitConfig {
            burst_size: Some(5),
            ..RateLimitConfig::default()
        };
        assert_eq!(
            RateLimiter::unimplemented_field_names(&config),
            vec!["burst_size"]
        );
    }
}
