//! Deployment core data structures for Router Phase 1
//!
//! This module defines the fundamental building blocks for the LiteLLM Router:
//! - `Deployment`: A concrete provider deployment with configuration and runtime state
//! - `DeploymentConfig`: Configuration parameters (TPM/RPM limits, timeouts, weights)
//! - `DeploymentState`: Lock-free runtime state using atomic operations
//! - `HealthStatus`: Health status enumeration for deployments
//!
//! ## Design Philosophy
//!
//! All state tracking uses atomic operations with `Relaxed` ordering for maximum performance.
//! This is safe because:
//! - State values are eventually consistent (exact precision not required for routing decisions)
//! - No cross-field invariants need to be maintained atomically
//! - Routing can tolerate slightly stale state for massive performance gains
//!
//! ## Performance Characteristics
//!
//! - Lock-free: All state updates use atomics, zero contention
//! - Zero-copy: Deployments are accessed by reference, never cloned
//! - Cache-friendly: Hot path fields grouped together

use crate::core::providers::Provider;
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Deployment identifier (unique within router)
pub type DeploymentId = String;

/// Health status enumeration for deployments
///
/// Maps to AtomicU8 values for lock-free updates:
/// - 0 = Unknown (newly created, not yet health checked)
/// - 1 = Healthy (passing health checks, ready to serve)
/// - 2 = Degraded (experiencing issues but still functional)
/// - 3 = Unhealthy (failing health checks, should not serve)
/// - 4 = Cooldown (temporarily disabled after failures)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HealthStatus {
    Unknown = 0,
    Healthy = 1,
    Degraded = 2,
    Unhealthy = 3,
    Cooldown = 4,
}

impl From<u8> for HealthStatus {
    fn from(value: u8) -> Self {
        match value {
            1 => HealthStatus::Healthy,
            2 => HealthStatus::Degraded,
            3 => HealthStatus::Unhealthy,
            4 => HealthStatus::Cooldown,
            _ => HealthStatus::Unknown,
        }
    }
}

impl From<HealthStatus> for u8 {
    fn from(status: HealthStatus) -> Self {
        status as u8
    }
}

/// Deployment configuration
///
/// These are static parameters that don't change during runtime.
/// All are stored as simple values (no atomics needed).
#[derive(Debug, Clone)]
pub struct DeploymentConfig {
    /// Tokens per minute limit (None = unlimited)
    pub tpm_limit: Option<u64>,

    /// Requests per minute limit (None = unlimited)
    pub rpm_limit: Option<u64>,

    /// Maximum parallel requests (None = unlimited)
    pub max_parallel_requests: Option<u32>,

    /// Weight for weighted random selection (higher = more likely to be selected)
    pub weight: u32,

    /// Timeout in seconds
    pub timeout_secs: u64,

    /// Priority (lower value = higher priority)
    pub priority: u32,
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            tpm_limit: None,
            rpm_limit: None,
            max_parallel_requests: None,
            weight: 1,
            timeout_secs: 60,
            priority: 0,
        }
    }
}

/// Deployment runtime state
///
/// All fields use atomics for lock-free updates with `Relaxed` ordering.
/// This is safe because routing decisions can tolerate eventual consistency.
///
/// ## State Reset
///
/// TPM/RPM counters are reset every minute by a background task.
/// The `minute_reset_at` timestamp tracks when the last reset occurred.
#[derive(Debug)]
pub struct DeploymentState {
    /// Health status (0=unknown, 1=healthy, 2=degraded, 3=unhealthy, 4=cooldown)
    pub health: AtomicU8,

    /// Current minute TPM usage
    pub tpm_current: AtomicU64,

    /// Current minute RPM usage
    pub rpm_current: AtomicU64,

    /// Current active requests
    pub active_requests: AtomicU32,

    /// Total requests (lifetime)
    pub total_requests: AtomicU64,

    /// Successful requests (lifetime)
    pub success_requests: AtomicU64,

    /// Failed requests (lifetime)
    pub fail_requests: AtomicU64,

    /// Failures this minute (for cooldown detection)
    pub fails_this_minute: AtomicU32,

    /// Cooldown end timestamp (unix seconds)
    pub cooldown_until: AtomicU64,

    /// Last request timestamp (unix seconds)
    pub last_request_at: AtomicU64,

    /// Average latency in microseconds (sliding window)
    pub avg_latency_us: AtomicU64,

    /// Last minute reset timestamp (unix seconds)
    pub minute_reset_at: AtomicU64,
}

impl DeploymentState {
    /// Create new deployment state with default values
    pub fn new() -> Self {
        let now = current_timestamp();
        Self {
            health: AtomicU8::new(HealthStatus::Healthy as u8),
            tpm_current: AtomicU64::new(0),
            rpm_current: AtomicU64::new(0),
            active_requests: AtomicU32::new(0),
            total_requests: AtomicU64::new(0),
            success_requests: AtomicU64::new(0),
            fail_requests: AtomicU64::new(0),
            fails_this_minute: AtomicU32::new(0),
            cooldown_until: AtomicU64::new(0),
            last_request_at: AtomicU64::new(0),
            avg_latency_us: AtomicU64::new(0),
            minute_reset_at: AtomicU64::new(now),
        }
    }

    /// Reset per-minute counters
    ///
    /// Should be called by a background task every minute.
    pub fn reset_minute(&self) {
        self.tpm_current.store(0, Ordering::Relaxed);
        self.rpm_current.store(0, Ordering::Relaxed);
        self.fails_this_minute.store(0, Ordering::Relaxed);
        self.minute_reset_at
            .store(current_timestamp(), Ordering::Relaxed);
    }

    /// Get current health status
    pub fn health_status(&self) -> HealthStatus {
        self.health.load(Ordering::Relaxed).into()
    }
}

impl Default for DeploymentState {
    fn default() -> Self {
        Self::new()
    }
}

// Manual Clone implementation because AtomicU64 doesn't implement Clone
impl Clone for DeploymentState {
    fn clone(&self) -> Self {
        Self {
            health: AtomicU8::new(self.health.load(Ordering::Relaxed)),
            tpm_current: AtomicU64::new(self.tpm_current.load(Ordering::Relaxed)),
            rpm_current: AtomicU64::new(self.rpm_current.load(Ordering::Relaxed)),
            active_requests: AtomicU32::new(self.active_requests.load(Ordering::Relaxed)),
            total_requests: AtomicU64::new(self.total_requests.load(Ordering::Relaxed)),
            success_requests: AtomicU64::new(self.success_requests.load(Ordering::Relaxed)),
            fail_requests: AtomicU64::new(self.fail_requests.load(Ordering::Relaxed)),
            fails_this_minute: AtomicU32::new(self.fails_this_minute.load(Ordering::Relaxed)),
            cooldown_until: AtomicU64::new(self.cooldown_until.load(Ordering::Relaxed)),
            last_request_at: AtomicU64::new(self.last_request_at.load(Ordering::Relaxed)),
            avg_latency_us: AtomicU64::new(self.avg_latency_us.load(Ordering::Relaxed)),
            minute_reset_at: AtomicU64::new(self.minute_reset_at.load(Ordering::Relaxed)),
        }
    }
}

/// Deployment - a concrete provider deployment
///
/// Represents a single deployment of a provider (e.g., "openai-gpt4-primary").
/// Multiple deployments can serve the same model_name (e.g., "gpt-4").
///
/// ## Example
///
/// ```rust,no_run
/// # use litellm_rs::core::router::deployment::{Deployment, DeploymentConfig};
/// # use litellm_rs::Provider;
/// # fn example(provider: Provider) {
/// let deployment = Deployment::new(
///     "openai-gpt4-primary".to_string(),
///     provider,
///     "gpt-4-turbo".to_string(),
///     "gpt-4".to_string(),
/// )
/// .with_config(DeploymentConfig {
///     tpm_limit: Some(100_000),
///     rpm_limit: Some(500),
///     weight: 2,
///     ..Default::default()
/// })
/// .with_tags(vec!["production".to_string(), "fast".to_string()]);
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Deployment {
    /// Unique deployment ID
    pub id: DeploymentId,

    /// Provider instance
    pub provider: Provider,

    /// Actual model name (e.g., "azure/gpt-4-turbo")
    pub model: String,

    /// User-facing model name / model group (e.g., "gpt-4")
    pub model_name: String,

    /// Configuration
    pub config: DeploymentConfig,

    /// Runtime state (lock-free)
    pub state: DeploymentState,

    /// Tags for filtering (e.g., ["production", "fast"])
    pub tags: Vec<String>,
}

impl Deployment {
    /// Create a new deployment
    ///
    /// # Arguments
    ///
    /// * `id` - Unique deployment identifier
    /// * `provider` - Provider instance
    /// * `model` - Actual model name (provider-specific)
    /// * `model_name` - User-facing model name (model group)
    pub fn new(id: DeploymentId, provider: Provider, model: String, model_name: String) -> Self {
        Self {
            id,
            provider,
            model,
            model_name,
            config: DeploymentConfig::default(),
            state: DeploymentState::new(),
            tags: Vec::new(),
        }
    }

    /// Set deployment configuration (builder pattern)
    pub fn with_config(mut self, config: DeploymentConfig) -> Self {
        self.config = config;
        self
    }

    /// Set deployment tags (builder pattern)
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Check if deployment is healthy
    ///
    /// Returns true if health status is Healthy or Degraded (but not Unknown, Unhealthy, or Cooldown).
    pub fn is_healthy(&self) -> bool {
        let status = self.state.health_status();
        matches!(status, HealthStatus::Healthy | HealthStatus::Degraded)
    }

    /// Check if deployment is in cooldown
    ///
    /// Returns true if current time is before cooldown_until timestamp.
    /// When cooldown expires, automatically resets health to Degraded so the
    /// deployment becomes eligible for selection again.
    pub fn is_in_cooldown(&self) -> bool {
        let cooldown_until = self.state.cooldown_until.load(Ordering::Relaxed);
        if cooldown_until == 0 {
            return false;
        }
        let now = current_timestamp();
        if cooldown_until > now {
            return true;
        }
        // Cooldown expired: reset health from Cooldown to Degraded so
        // `is_healthy()` returns true and the deployment is selectable.
        // CAS failure means another thread already transitioned the state -- safe to ignore.
        self.state
            .health
            .compare_exchange(
                HealthStatus::Cooldown as u8,
                HealthStatus::Degraded as u8,
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .ok();
        false
    }

    /// Record a successful request
    ///
    /// Updates counters and calculates exponential moving average for latency.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens consumed
    /// * `latency_us` - Request latency in microseconds
    pub fn record_success(&self, tokens: u64, latency_us: u64) {
        // Update counters
        self.state.total_requests.fetch_add(1, Ordering::Relaxed);
        self.state.success_requests.fetch_add(1, Ordering::Relaxed);
        self.state.tpm_current.fetch_add(tokens, Ordering::Relaxed);
        self.state.rpm_current.fetch_add(1, Ordering::Relaxed);
        self.state
            .last_request_at
            .store(current_timestamp(), Ordering::Relaxed);

        // Update average latency using exponential moving average (alpha = 0.2)
        let current_avg = self.state.avg_latency_us.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            latency_us
        } else {
            // EMA: new_avg = alpha * new_value + (1 - alpha) * old_avg
            // Using alpha = 0.2 = 1/5
            (latency_us + 4 * current_avg) / 5
        };
        self.state.avg_latency_us.store(new_avg, Ordering::Relaxed);

        // If health was Degraded, consider promoting to Healthy
        let current_health = self.state.health.load(Ordering::Relaxed);
        if current_health == HealthStatus::Degraded as u8 {
            // Simple heuristic: promote after successful request
            self.state
                .health
                .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
        }
    }

    /// Record a failed request
    ///
    /// Increments failure counters. The caller is responsible for deciding
    /// whether to enter cooldown based on failure rate.
    pub fn record_failure(&self) {
        self.state.total_requests.fetch_add(1, Ordering::Relaxed);
        self.state.fail_requests.fetch_add(1, Ordering::Relaxed);
        self.state.fails_this_minute.fetch_add(1, Ordering::Relaxed);
        self.state
            .last_request_at
            .store(current_timestamp(), Ordering::Relaxed);

        // Mark as degraded (caller can escalate to Unhealthy/Cooldown if needed)
        self.state
            .health
            .store(HealthStatus::Degraded as u8, Ordering::Relaxed);
    }

    /// Enter cooldown state
    ///
    /// Sets health to Cooldown and configures cooldown end time.
    ///
    /// # Arguments
    ///
    /// * `duration_secs` - Cooldown duration in seconds
    pub fn enter_cooldown(&self, duration_secs: u64) {
        let cooldown_until = current_timestamp() + duration_secs;
        self.state
            .cooldown_until
            .store(cooldown_until, Ordering::Relaxed);
        self.state
            .health
            .store(HealthStatus::Cooldown as u8, Ordering::Relaxed);
    }
}

/// Get current Unix timestamp in seconds
///
/// Returns the number of seconds since UNIX_EPOCH.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    // ==================== HealthStatus Tests ====================

    #[test]
    fn test_health_status_from_u8_healthy() {
        assert_eq!(HealthStatus::from(1), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_status_from_u8_degraded() {
        assert_eq!(HealthStatus::from(2), HealthStatus::Degraded);
    }

    #[test]
    fn test_health_status_from_u8_unhealthy() {
        assert_eq!(HealthStatus::from(3), HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_status_from_u8_cooldown() {
        assert_eq!(HealthStatus::from(4), HealthStatus::Cooldown);
    }

    #[test]
    fn test_health_status_from_u8_unknown() {
        assert_eq!(HealthStatus::from(0), HealthStatus::Unknown);
        assert_eq!(HealthStatus::from(255), HealthStatus::Unknown);
    }

    #[test]
    fn test_health_status_to_u8() {
        assert_eq!(u8::from(HealthStatus::Unknown), 0);
        assert_eq!(u8::from(HealthStatus::Healthy), 1);
        assert_eq!(u8::from(HealthStatus::Degraded), 2);
        assert_eq!(u8::from(HealthStatus::Unhealthy), 3);
        assert_eq!(u8::from(HealthStatus::Cooldown), 4);
    }

    #[test]
    fn test_health_status_clone() {
        let status = HealthStatus::Healthy;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    // ==================== DeploymentConfig Tests ====================

    #[test]
    fn test_deployment_config_default() {
        let config = DeploymentConfig::default();
        assert!(config.tpm_limit.is_none());
        assert!(config.rpm_limit.is_none());
        assert!(config.max_parallel_requests.is_none());
        assert_eq!(config.weight, 1);
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.priority, 0);
    }

    #[test]
    fn test_deployment_config_custom() {
        let config = DeploymentConfig {
            tpm_limit: Some(100_000),
            rpm_limit: Some(500),
            max_parallel_requests: Some(10),
            weight: 2,
            timeout_secs: 120,
            priority: 1,
        };
        assert_eq!(config.tpm_limit, Some(100_000));
        assert_eq!(config.rpm_limit, Some(500));
        assert_eq!(config.max_parallel_requests, Some(10));
        assert_eq!(config.weight, 2);
    }

    #[test]
    fn test_deployment_config_clone() {
        let config = DeploymentConfig {
            tpm_limit: Some(50_000),
            rpm_limit: Some(100),
            ..DeploymentConfig::default()
        };
        let cloned = config.clone();
        assert_eq!(config.tpm_limit, cloned.tpm_limit);
        assert_eq!(config.rpm_limit, cloned.rpm_limit);
    }

    // ==================== DeploymentState Tests ====================

    #[test]
    fn test_deployment_state_new() {
        let state = DeploymentState::new();
        assert_eq!(state.health_status(), HealthStatus::Healthy);
        assert_eq!(state.tpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(state.rpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(state.active_requests.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_deployment_state_default() {
        let state = DeploymentState::default();
        assert_eq!(state.health_status(), HealthStatus::Healthy);
    }

    #[test]
    fn test_deployment_state_reset_minute() {
        let state = DeploymentState::new();
        state.tpm_current.store(1000, Ordering::Relaxed);
        state.rpm_current.store(50, Ordering::Relaxed);
        state.fails_this_minute.store(5, Ordering::Relaxed);

        state.reset_minute();

        assert_eq!(state.tpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(state.rpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(state.fails_this_minute.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_deployment_state_health_status() {
        let state = DeploymentState::new();
        state
            .health
            .store(HealthStatus::Degraded as u8, Ordering::Relaxed);
        assert_eq!(state.health_status(), HealthStatus::Degraded);
    }

    #[test]
    fn test_deployment_state_clone() {
        let state = DeploymentState::new();
        state.total_requests.store(100, Ordering::Relaxed);
        state.success_requests.store(95, Ordering::Relaxed);

        let cloned = state.clone();
        assert_eq!(cloned.total_requests.load(Ordering::Relaxed), 100);
        assert_eq!(cloned.success_requests.load(Ordering::Relaxed), 95);
    }

    // ==================== current_timestamp Tests ====================

    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        assert!(ts > 0);
        // Timestamp should be after year 2020
        assert!(ts > 1577836800); // 2020-01-01
    }

    #[test]
    fn test_current_timestamp_monotonic() {
        let ts1 = current_timestamp();
        let ts2 = current_timestamp();
        assert!(ts2 >= ts1);
    }
}
