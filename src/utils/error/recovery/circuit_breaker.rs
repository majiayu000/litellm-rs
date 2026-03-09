//! Circuit breaker implementation for fault tolerance

use super::types::{CircuitBreakerConfig, CircuitBreakerMetrics, CircuitState};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
#[allow(unused_imports)]
use tracing::{debug, warn};

const STATE_CLOSED: u8 = 0;
const STATE_OPEN: u8 = 1;
const STATE_HALF_OPEN: u8 = 2;
const NO_TIMESTAMP: u64 = u64::MAX;

fn decode_state(state: u8) -> CircuitState {
    match state {
        STATE_CLOSED => CircuitState::Closed,
        STATE_OPEN => CircuitState::Open,
        STATE_HALF_OPEN => CircuitState::HalfOpen,
        _ => CircuitState::Closed,
    }
}

fn duration_to_nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: AtomicU8,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure_time: AtomicU64,
    request_count: AtomicU32,
    window_start: AtomicU64,
    time_origin: Instant,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: AtomicU8::new(STATE_CLOSED),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            last_failure_time: AtomicU64::new(NO_TIMESTAMP),
            request_count: AtomicU32::new(0),
            window_start: AtomicU64::new(0),
            time_origin: Instant::now(),
        }
    }

    fn now_nanos(&self) -> u64 {
        self.time_origin
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, R, E>(&self, f: F) -> Result<R>
    where
        F: std::future::Future<Output = std::result::Result<R, E>>,
        E: std::fmt::Display + std::fmt::Debug,
    {
        // Check if circuit should allow the request
        if !self.can_execute().await {
            return Err(GatewayError::ProviderUnavailable(
                "Circuit breaker is open".to_string(),
            ));
        }

        self.request_count.fetch_add(1, Ordering::Relaxed);

        match f.await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(error) => {
                self.on_failure().await;
                Err(GatewayError::External(format!(
                    "Circuit breaker protected call failed: {}",
                    error
                )))
            }
        }
    }

    /// Check if the circuit breaker allows execution
    async fn can_execute(&self) -> bool {
        match self.state.load(Ordering::Acquire) {
            STATE_CLOSED => true,
            STATE_OPEN => {
                let last_failure_time = self.last_failure_time.load(Ordering::Acquire);
                if last_failure_time == NO_TIMESTAMP {
                    return false;
                }

                let elapsed = self.now_nanos().saturating_sub(last_failure_time);
                if elapsed < duration_to_nanos(self.config.timeout) {
                    return false;
                }

                match self.state.compare_exchange(
                    STATE_OPEN,
                    STATE_HALF_OPEN,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        debug!("Circuit breaker transitioning from Open to HalfOpen");
                        self.success_count.store(0, Ordering::Relaxed);
                        true
                    }
                    Err(current_state) => current_state != STATE_OPEN,
                }
            }
            STATE_HALF_OPEN => true,
            _ => false,
        }
    }

    /// Handle successful request
    async fn on_success(&self) {
        let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;

        if success_count >= self.config.success_threshold
            && self
                .state
                .compare_exchange(
                    STATE_HALF_OPEN,
                    STATE_CLOSED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
        {
            debug!("Circuit breaker transitioning from HalfOpen to Closed");
            self.failure_count.store(0, Ordering::Relaxed);
            self.success_count.store(0, Ordering::Relaxed);
        }
    }

    /// Handle failed request
    async fn on_failure(&self) {
        let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        let request_count = self.request_count.load(Ordering::Relaxed);
        let now = self.now_nanos();

        self.last_failure_time.store(now, Ordering::Release);

        let window_start = self.window_start.load(Ordering::Acquire);
        if now.saturating_sub(window_start) >= duration_to_nanos(self.config.window_size)
            && self
                .window_start
                .compare_exchange(window_start, now, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            self.failure_count.store(1, Ordering::Relaxed);
            self.request_count.store(1, Ordering::Relaxed);
            return;
        }

        // Check if we should open the circuit
        if request_count >= self.config.min_requests
            && failure_count >= self.config.failure_threshold
            && self.state.load(Ordering::Acquire) != STATE_OPEN
        {
            warn!(
                "Circuit breaker opening due to {} failures out of {} requests",
                failure_count, request_count
            );
            self.state.store(STATE_OPEN, Ordering::Release);
        }

        // Always open from half-open on failure
        if self.state.load(Ordering::Acquire) == STATE_HALF_OPEN {
            debug!("Circuit breaker transitioning from HalfOpen to Open due to failure");
            self.state.store(STATE_OPEN, Ordering::Release);
        }
    }

    /// Get current circuit breaker state
    pub fn state(&self) -> CircuitState {
        decode_state(self.state.load(Ordering::Acquire))
    }

    /// Get current metrics
    pub fn metrics(&self) -> CircuitBreakerMetrics {
        CircuitBreakerMetrics {
            state: self.state(),
            failure_count: self.failure_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
            request_count: self.request_count.load(Ordering::Relaxed),
        }
    }

    /// Reset the circuit breaker
    pub fn reset(&self) {
        self.state.store(STATE_CLOSED, Ordering::Release);
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.request_count.store(0, Ordering::Relaxed);
        self.last_failure_time
            .store(NO_TIMESTAMP, Ordering::Release);
        self.window_start.store(self.now_nanos(), Ordering::Release);
        debug!("Circuit breaker reset");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    fn default_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            min_requests: 5,
            timeout: Duration::from_millis(100),
            window_size: Duration::from_secs(60),
        }
    }

    // ==================== Creation Tests ====================

    #[test]
    fn test_circuit_breaker_new() {
        let config = default_config();
        let cb = CircuitBreaker::new(config);

        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_initial_state_is_closed() {
        let cb = CircuitBreaker::new(default_config());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_initial_metrics() {
        let cb = CircuitBreaker::new(default_config());
        let metrics = cb.metrics();

        assert_eq!(metrics.state, CircuitState::Closed);
        assert_eq!(metrics.failure_count, 0);
        assert_eq!(metrics.success_count, 0);
        assert_eq!(metrics.request_count, 0);
    }

    // ==================== State Tests ====================

    #[test]
    fn test_circuit_breaker_state_returns_current_state() {
        let cb = CircuitBreaker::new(default_config());
        let state = cb.state();
        assert_eq!(state, CircuitState::Closed);
    }

    // ==================== Metrics Tests ====================

    #[test]
    fn test_circuit_breaker_metrics_structure() {
        let cb = CircuitBreaker::new(default_config());
        let metrics = cb.metrics();

        // Verify metrics struct fields
        assert_eq!(metrics.state, CircuitState::Closed);
        assert_eq!(metrics.failure_count, 0);
        assert_eq!(metrics.success_count, 0);
        assert_eq!(metrics.request_count, 0);
    }

    // ==================== Reset Tests ====================

    #[test]
    fn test_circuit_breaker_reset() {
        let cb = CircuitBreaker::new(default_config());

        // Manually increment counters (accessing internal state)
        cb.failure_count.store(5, Ordering::Relaxed);
        cb.success_count.store(3, Ordering::Relaxed);
        cb.request_count.store(10, Ordering::Relaxed);

        // Reset
        cb.reset();

        let metrics = cb.metrics();
        assert_eq!(metrics.state, CircuitState::Closed);
        assert_eq!(metrics.failure_count, 0);
        assert_eq!(metrics.success_count, 0);
        assert_eq!(metrics.request_count, 0);
    }

    #[test]
    fn test_circuit_breaker_reset_clears_last_failure_time() {
        let cb = CircuitBreaker::new(default_config());

        // Set last failure time
        cb.last_failure_time
            .store(cb.now_nanos(), Ordering::Relaxed);

        // Reset
        cb.reset();

        assert_eq!(cb.last_failure_time.load(Ordering::Relaxed), NO_TIMESTAMP);
    }

    #[test]
    fn test_circuit_breaker_reset_resets_window_start() {
        let cb = CircuitBreaker::new(default_config());

        let before = cb.window_start.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(10));

        cb.reset();

        let after = cb.window_start.load(Ordering::Relaxed);
        assert!(after > before);
    }

    // ==================== Config Tests ====================

    #[test]
    fn test_circuit_breaker_with_custom_config() {
        let config = CircuitBreakerConfig {
            failure_threshold: 10,
            success_threshold: 5,
            min_requests: 20,
            timeout: Duration::from_secs(120),
            window_size: Duration::from_secs(300),
        };

        let cb = CircuitBreaker::new(config);
        assert_eq!(cb.config.failure_threshold, 10);
        assert_eq!(cb.config.success_threshold, 5);
    }

    #[test]
    fn test_circuit_breaker_with_zero_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 0,
            success_threshold: 0,
            min_requests: 0,
            timeout: Duration::from_millis(1),
            window_size: Duration::from_millis(1),
        };

        let cb = CircuitBreaker::new(config);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    // ==================== Async Call Tests ====================

    #[tokio::test]
    async fn test_circuit_breaker_call_success() {
        let cb = CircuitBreaker::new(default_config());

        let result = cb.call(async { Ok::<_, String>("success") }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_circuit_breaker_call_increments_request_count() {
        let cb = CircuitBreaker::new(default_config());

        let _ = cb.call(async { Ok::<_, String>("test") }).await;

        let metrics = cb.metrics();
        assert_eq!(metrics.request_count, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_call_increments_success_count() {
        let cb = CircuitBreaker::new(default_config());

        let _ = cb.call(async { Ok::<_, String>("test") }).await;

        let metrics = cb.metrics();
        assert_eq!(metrics.success_count, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_call_failure() {
        let cb = CircuitBreaker::new(default_config());

        let result: Result<()> = cb.call(async { Err::<(), _>("failure") }).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_circuit_breaker_call_failure_increments_failure_count() {
        let cb = CircuitBreaker::new(default_config());

        let _: Result<()> = cb.call(async { Err::<(), _>("failure") }).await;

        let metrics = cb.metrics();
        assert_eq!(metrics.failure_count, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_multiple_successes() {
        let cb = CircuitBreaker::new(default_config());

        for _ in 0..5 {
            let _ = cb.call(async { Ok::<_, String>("success") }).await;
        }

        let metrics = cb.metrics();
        assert_eq!(metrics.request_count, 5);
        assert_eq!(metrics.success_count, 5);
        assert_eq!(metrics.failure_count, 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_mixed_results() {
        let cb = CircuitBreaker::new(default_config());

        // Two successes
        let _ = cb.call(async { Ok::<_, String>("success") }).await;
        let _ = cb.call(async { Ok::<_, String>("success") }).await;

        // One failure
        let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;

        let metrics = cb.metrics();
        assert_eq!(metrics.request_count, 3);
        assert!(metrics.failure_count >= 1);
    }

    // ==================== State Transition Tests ====================

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            min_requests: 3, // Lower min_requests so circuit opens sooner
            timeout: Duration::from_millis(100),
            window_size: Duration::from_secs(60),
        };
        let cb = CircuitBreaker::new(config);

        // Cause enough failures to open circuit
        for _ in 0..5 {
            let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;
        }

        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_open_rejects_requests() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            min_requests: 2,
            timeout: Duration::from_secs(10), // Long timeout so it stays open
            window_size: Duration::from_secs(60),
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        for _ in 0..5 {
            let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;
        }

        assert_eq!(cb.state(), CircuitState::Open);

        // Next request should be rejected
        let result: Result<()> = cb.call(async { Ok::<(), String>(()) }).await;
        assert!(result.is_err());

        if let Err(GatewayError::ProviderUnavailable(msg)) = result {
            assert!(msg.contains("Circuit breaker is open"));
        } else {
            panic!("Expected ProviderUnavailable error");
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker_transitions_to_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            min_requests: 2,
            timeout: Duration::from_millis(50), // Short timeout
            window_size: Duration::from_secs(60),
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        for _ in 0..3 {
            let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;
        }

        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Next call should transition to half-open
        let _ = cb.call(async { Ok::<_, String>("success") }).await;

        // State should be HalfOpen or Closed depending on success
        let state = cb.state();
        assert!(state == CircuitState::HalfOpen || state == CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_closes_after_success_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            min_requests: 2,
            timeout: Duration::from_millis(10),
            window_size: Duration::from_secs(60),
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        for _ in 0..3 {
            let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;
        }

        // Wait for timeout to transition to half-open
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Successful calls in half-open state
        for _ in 0..3 {
            let _ = cb.call(async { Ok::<_, String>("success") }).await;
        }

        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_failure_reopens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            min_requests: 2,
            timeout: Duration::from_millis(10),
            window_size: Duration::from_secs(60),
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        for _ in 0..3 {
            let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;
        }
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(50)).await;

        // This will transition to half-open and then fail
        let _: Result<()> = cb.call(async { Err::<(), _>("fail again") }).await;

        assert_eq!(cb.state(), CircuitState::Open);
    }

    // ==================== Concurrent Access Tests ====================

    #[tokio::test]
    async fn test_circuit_breaker_concurrent_calls() {
        let cb = Arc::new(CircuitBreaker::new(default_config()));

        let mut handles = vec![];

        for _ in 0..10 {
            let cb_clone = cb.clone();
            let handle = tokio::spawn(async move {
                let _ = cb_clone.call(async { Ok::<_, String>("success") }).await;
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let metrics = cb.metrics();
        assert_eq!(metrics.request_count, 10);
    }

    #[tokio::test]
    async fn test_circuit_breaker_concurrent_state_access() {
        let cb = Arc::new(CircuitBreaker::new(default_config()));

        let mut handles = vec![];

        for _ in 0..5 {
            let cb_clone = cb.clone();
            let handle = tokio::spawn(async move { cb_clone.state() });
            handles.push(handle);
        }

        for handle in handles {
            let state = handle.await.unwrap();
            assert_eq!(state, CircuitState::Closed);
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker_concurrent_reset() {
        let cb = Arc::new(CircuitBreaker::new(default_config()));

        // Add some state
        cb.failure_count.store(5, Ordering::Relaxed);

        let mut handles = vec![];
        for _ in 0..3 {
            let cb_clone = cb.clone();
            let handle = tokio::spawn(async move {
                cb_clone.reset();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // State should be reset
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.metrics().failure_count, 0);
    }

    // ==================== Error Type Tests ====================

    #[tokio::test]
    async fn test_circuit_breaker_preserves_error_message() {
        let cb = CircuitBreaker::new(default_config());

        let result: Result<()> = cb
            .call(async { Err::<(), _>("specific error message") })
            .await;

        if let Err(GatewayError::External(msg)) = result {
            assert!(msg.contains("specific error message"));
        } else {
            panic!("Expected External error");
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker_open_error_type() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            min_requests: 1,
            timeout: Duration::from_secs(60),
            window_size: Duration::from_secs(60),
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;
        let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;

        // Next request should fail with ProviderUnavailable
        let result: Result<()> = cb.call(async { Ok::<(), String>(()) }).await;

        assert!(matches!(result, Err(GatewayError::ProviderUnavailable(_))));
    }

    // ==================== Window Size Tests ====================

    #[tokio::test]
    async fn test_circuit_breaker_window_resets_after_expiry() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            min_requests: 10,
            timeout: Duration::from_secs(60),
            window_size: Duration::from_millis(50), // Very short window
        };
        let cb = CircuitBreaker::new(config);

        // Add some failures
        for _ in 0..3 {
            let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;
        }

        // Wait for window to expire
        tokio::time::sleep(Duration::from_millis(100)).await;

        // This failure should reset window
        let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;

        // Failure count should be reset to 1
        let metrics = cb.metrics();
        assert_eq!(metrics.failure_count, 1);
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_circuit_breaker_debug() {
        let cb = CircuitBreaker::new(default_config());
        // Just verify it doesn't panic
        let _ = cb.state();
        let _ = cb.metrics();
    }

    #[tokio::test]
    async fn test_circuit_breaker_many_failures_then_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            min_requests: 2,
            timeout: Duration::from_millis(10),
            window_size: Duration::from_secs(60),
        };
        let cb = CircuitBreaker::new(config);

        // Many failures
        for _ in 0..10 {
            let _: Result<()> = cb.call(async { Err::<(), _>("fail") }).await;
        }

        assert_eq!(cb.state(), CircuitState::Open);

        // Wait and recover
        tokio::time::sleep(Duration::from_millis(50)).await;

        for _ in 0..5 {
            let _ = cb.call(async { Ok::<_, String>("ok") }).await;
        }

        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
