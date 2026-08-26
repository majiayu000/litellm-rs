//! Deployment core data structures for Router Phase 1
//!
//! This module defines the fundamental building blocks for the LiteLLM Router:
//! - `Deployment`: A concrete provider deployment with configuration and runtime state
//! - `DeploymentConfig`: Configuration parameters (TPM/RPM limits, timeouts, weights)
//! - `DeploymentState`: Low-contention runtime state using atomic operations
//! - `HealthStatus`: Health status enumeration for deployments
//!
//! ## Design Philosophy
//!
//! State tracking uses atomic operations for low-contention updates. Per-minute
//! counter updates share a read lock, while the rare rollover takes its write
//! lock so a reset cannot erase usage from the new window.
//!
//! ## Performance Characteristics
//!
//! - Low contention: ordinary per-minute updates proceed concurrently
//! - Probe lifecycle synchronization stays off the request path
//! - Zero-copy: Deployments are accessed by reference, never cloned
//! - Cache-friendly: Hot path fields grouped together

use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::Provider;
use crate::utils::auth::crypto::hmac::CredentialDigest;
use parking_lot::RwLock;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "deployment/probe_state.rs"]
mod probe_state;
#[path = "deployment/provider_instance.rs"]
mod provider_instance;
use probe_state::ProbeLifecycle;
pub(crate) use probe_state::publish_probe_group;
pub(crate) use provider_instance::ProviderInstanceIdentity;
use url::Url;

/// Deployment identifier (unique within router)
pub type DeploymentId = String;

/// Immutable legacy-selector metadata published beside a deployment.
///
/// It deliberately contains no raw credential and has no serialization or
/// display implementation.
#[derive(Clone)]
pub(crate) struct LegacySelectorMetadata {
    credential_digest: CredentialDigest,
}

impl LegacySelectorMetadata {
    pub(crate) fn from_stored_credential(credential: &str) -> Self {
        Self {
            credential_digest: CredentialDigest::from_credential(credential),
        }
    }

    pub(crate) fn credential_matches(&self, request_digest: &CredentialDigest) -> bool {
        self.credential_digest.constant_time_matches(request_digest)
    }
}

impl fmt::Debug for LegacySelectorMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LegacySelectorMetadata")
            .field("credential_digest", &"[REDACTED]")
            .finish()
    }
}

/// Normalized retry timing for a gateway-configured deployment.
///
/// Retry eligibility and retry-after precedence remain owned by `RetryPolicy`;
/// this value only carries the provider-specific fallback schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct RetrySchedule {
    /// Delay before the first retry, in milliseconds.
    pub base_delay_ms: u64,
    /// Hard upper bound for any configured retry delay, in milliseconds.
    pub max_delay_ms: u64,
    /// Exponential multiplier applied for each subsequent retry.
    pub backoff_multiplier: f64,
    /// Symmetric jitter ratio in the inclusive range `0.0..=1.0`.
    pub jitter_ratio: f64,
}

/// Runtime policy for an active provider health probe.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HealthCheckPolicy {
    /// Gateway provider name used to form one compatible probe group.
    pub provider_name: String,
    /// Delay between ordinary probe attempts.
    pub interval_secs: u64,
    /// Consecutive failures required before marking deployments unhealthy.
    pub failure_threshold: u32,
    /// Delay after reaching the failure threshold.
    pub recovery_timeout_secs: u64,
    /// Normalized custom unauthenticated GET endpoint, or native provider probe when absent.
    pub endpoint: Option<Url>,
    /// Runtime network policy applied to a configured custom endpoint.
    pub endpoint_access: ProviderEndpointAccess,
    /// HTTP statuses accepted by a custom endpoint probe.
    pub expected_codes: Vec<u16>,
}

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

    /// Provider-specific retry schedule, or `None` to use router defaults.
    pub retry_schedule: Option<RetrySchedule>,

    /// Active health probe policy for gateway-created deployments.
    pub health_check_policy: Option<HealthCheckPolicy>,
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
            retry_schedule: None,
            health_check_policy: None,
        }
    }
}

/// Deployment runtime state
///
/// Counters use relaxed atomics. Per-minute snapshots and updates additionally
/// coordinate with rollover through a shared read/write gate.
///
/// ## State Reset
///
/// Per-minute counters roll lazily when readers or writers observe an elapsed
/// window. No background task is required for correct per-minute semantics.
#[derive(Debug, Clone)]
pub struct DeploymentState {
    inner: Arc<DeploymentStateInner>,
    minute_window_lock: Arc<RwLock<()>>,
    provider_instance_identity: ProviderInstanceIdentity,
    probe_health: Arc<AtomicU8>,
    probe_last_checked_at_millis: Arc<AtomicU64>,
    probe_lifecycle: Arc<ProbeLifecycle>,
    probe_generation: u64,
}

impl Deref for DeploymentState {
    type Target = DeploymentStateInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Shared deployment runtime counters.
///
/// `DeploymentState` is a cheap cloneable handle around this inner state so
/// cloned deployments and routing snapshots cannot fork runtime counters.
#[derive(Debug)]
pub struct DeploymentStateInner {
    /// Health status (0=unknown, 1=healthy, 2=degraded, 3=unhealthy, 4=cooldown)
    pub health: AtomicU8,

    /// Whether the active probe remains unhealthy while request cooldown owns `health`.
    pub probe_unhealthy: AtomicBool,

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

    /// Consecutive successful requests since last failure (for half-open promotion)
    pub consecutive_successes: AtomicU32,

    /// Last minute reset timestamp (unix seconds)
    pub minute_reset_at: AtomicU64,
}

impl DeploymentState {
    /// Create new deployment state with default values
    pub fn new() -> Self {
        Self::new_for_provider_instance(ProviderInstanceIdentity::new())
    }

    fn new_for_provider_instance(provider_instance_identity: ProviderInstanceIdentity) -> Self {
        let now = current_timestamp();
        Self {
            inner: Arc::new(DeploymentStateInner {
                health: AtomicU8::new(HealthStatus::Healthy as u8),
                probe_unhealthy: AtomicBool::new(false),
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
                consecutive_successes: AtomicU32::new(0),
                minute_reset_at: AtomicU64::new(now),
            }),
            minute_window_lock: Arc::new(RwLock::new(())),
            provider_instance_identity,
            probe_health: Arc::new(AtomicU8::new(HealthStatus::Unknown as u8)),
            probe_last_checked_at_millis: Arc::new(AtomicU64::new(0)),
            probe_lifecycle: Arc::new(ProbeLifecycle::new()),
            probe_generation: 0,
        }
    }

    /// Explicitly start a fresh per-minute counter window.
    ///
    /// Production correctness does not depend on calling this method because
    /// counter readers and writers also roll elapsed windows lazily.
    pub fn reset_minute(&self) {
        let _guard = self.minute_window_lock.write();
        self.finish_minute_reset(current_timestamp());
    }

    pub(crate) fn roll_minute_window(&self, now: u64) {
        self.with_current_minute(now, || ());
    }

    pub(crate) fn minute_counters(&self, now: u64) -> MinuteCounters {
        self.roll_minute_window(now);
        let _guard = self.minute_window_lock.read();
        MinuteCounters {
            tpm: self.tpm_current.load(Ordering::Relaxed),
            rpm: self.rpm_current.load(Ordering::Relaxed),
            failures: self.fails_this_minute.load(Ordering::Relaxed),
        }
    }

    fn with_current_minute<T>(&self, now: u64, operation: impl FnOnce() -> T) -> T {
        let guard = self.minute_window_lock.read();
        let last = self.minute_reset_at.load(Ordering::Acquire);
        if !minute_window_needs_roll(now, last) {
            let result = operation();
            drop(guard);
            return result;
        }
        drop(guard);

        let reset_guard = self.minute_window_lock.write();
        // `now` may have been prefetched by a selector that stalled while
        // another caller rolled the window. Re-read the wall clock on this
        // rare slow path so an old observer cannot masquerade as rollback
        // and erase fresh usage.
        let reset_now = current_timestamp();
        let last = self.minute_reset_at.load(Ordering::Acquire);
        if minute_window_needs_roll(reset_now, last) {
            self.finish_minute_reset(reset_now);
        }
        let result = operation();
        drop(reset_guard);
        result
    }

    fn finish_minute_reset(&self, now: u64) {
        self.tpm_current.store(0, Ordering::Relaxed);
        self.rpm_current.store(0, Ordering::Relaxed);
        self.fails_this_minute.store(0, Ordering::Relaxed);
        self.minute_reset_at.store(now, Ordering::Release);
    }

    /// Get current health status
    pub fn health_status(&self) -> HealthStatus {
        self.health.load(Ordering::Relaxed).into()
    }

    /// Get the last result published by the active readiness probe.
    pub fn probe_health_status(&self) -> HealthStatus {
        self.probe_health.load(Ordering::Acquire).into()
    }

    pub(crate) fn set_probe_health_status(&self, status: HealthStatus) {
        self.probe_last_checked_at_millis
            .store(current_timestamp_millis(), Ordering::Relaxed);
        self.probe_health.store(status as u8, Ordering::Release);
    }

    /// Get the completion time of the last active probe, in Unix milliseconds.
    pub fn probe_last_checked_at_millis(&self) -> Option<u64> {
        match self.probe_last_checked_at_millis.load(Ordering::Relaxed) {
            0 => None,
            timestamp => Some(timestamp),
        }
    }

    /// Share request-routing state while requiring fresh probe evidence.
    pub(crate) fn for_snapshot_insertion(&self) -> Self {
        self.for_snapshot_insertion_with_provider(self.provider_instance_identity.clone())
    }

    pub(crate) fn for_snapshot_insertion_with_provider(
        &self,
        provider_instance_identity: ProviderInstanceIdentity,
    ) -> Self {
        let probe_generation = self.probe_lifecycle.next_generation();
        Self {
            inner: Arc::clone(&self.inner),
            minute_window_lock: Arc::clone(&self.minute_window_lock),
            provider_instance_identity,
            probe_health: Arc::new(AtomicU8::new(HealthStatus::Unknown as u8)),
            probe_last_checked_at_millis: Arc::new(AtomicU64::new(0)),
            probe_lifecycle: Arc::clone(&self.probe_lifecycle),
            probe_generation,
        }
    }

    pub(crate) fn provider_instance_identity(&self) -> ProviderInstanceIdentity {
        self.provider_instance_identity.clone()
    }
}

impl Default for DeploymentState {
    fn default() -> Self {
        Self::new()
    }
}

const MINUTE_WINDOW_SECS: u64 = 60;

fn minute_window_needs_roll(now: u64, last: u64) -> bool {
    now < last || now - last >= MINUTE_WINDOW_SECS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MinuteCounters {
    pub(crate) tpm: u64,
    pub(crate) rpm: u64,
    pub(crate) failures: u32,
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

    /// Built-in provider enum instance.
    ///
    /// Router deployments dispatch through the closed `Provider` enum. A
    /// third-party `LLMProvider` implementation is not routeable here unless it
    /// is wired into that enum and its dispatch/factory paths.
    pub provider: Provider,

    /// Actual model name (e.g., "azure/gpt-4-turbo")
    pub model: String,

    /// User-facing model name / model group (e.g., "gpt-4")
    pub model_name: String,

    /// Configuration
    pub config: DeploymentConfig,

    /// Shared low-contention runtime state with synchronized probe lifecycle
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
    /// * `provider` - Built-in provider enum instance
    /// * `model` - Actual model name (provider-specific)
    /// * `model_name` - User-facing model name (model group)
    pub fn new(id: DeploymentId, provider: Provider, model: String, model_name: String) -> Self {
        Self::new_with_provider_instance(
            id,
            provider,
            model,
            model_name,
            ProviderInstanceIdentity::new(),
        )
    }

    pub(crate) fn new_with_provider_instance(
        id: DeploymentId,
        provider: Provider,
        model: String,
        model_name: String,
        provider_instance_identity: ProviderInstanceIdentity,
    ) -> Self {
        Self {
            id,
            provider,
            model,
            model_name,
            config: DeploymentConfig::default(),
            state: DeploymentState::new_for_provider_instance(provider_instance_identity),
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
        // Cooldown expiry restores probe-owned Unhealthy when the provider did
        // not recover during cooldown; otherwise it enters request half-open.
        let next = if self.state.probe_unhealthy.load(Ordering::Relaxed) {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        };
        // CAS failure means another thread already transitioned the state -- safe to ignore.
        self.state
            .health
            .compare_exchange(
                HealthStatus::Cooldown as u8,
                next as u8,
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
        let now = current_timestamp();
        self.state.total_requests.fetch_add(1, Ordering::Relaxed);
        self.state.success_requests.fetch_add(1, Ordering::Relaxed);
        self.state.with_current_minute(now, || {
            self.state.tpm_current.fetch_add(tokens, Ordering::Relaxed);
            self.state.rpm_current.fetch_add(1, Ordering::Relaxed);
        });
        self.state.last_request_at.store(now, Ordering::Relaxed);

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

        // Track consecutive successes for half-open promotion.
        // The caller (Router) checks the counter against success_threshold
        // to decide when to promote from Degraded to Healthy.
        self.state
            .consecutive_successes
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record a failed request
    ///
    /// Increments failure counters. The caller is responsible for deciding
    /// whether to enter cooldown based on failure rate.
    pub fn record_failure(&self) {
        self.record_failure_with_minute_counters();
    }

    pub(crate) fn record_failure_with_minute_counters(&self) -> MinuteCounters {
        let now = current_timestamp();
        self.state.total_requests.fetch_add(1, Ordering::Relaxed);
        self.state.fail_requests.fetch_add(1, Ordering::Relaxed);
        let counters = self.state.with_current_minute(now, || {
            self.state.fails_this_minute.fetch_add(1, Ordering::Relaxed);
            MinuteCounters {
                tpm: self.state.tpm_current.load(Ordering::Relaxed),
                rpm: self.state.rpm_current.load(Ordering::Relaxed),
                failures: self.state.fails_this_minute.load(Ordering::Relaxed),
            }
        });
        self.state.last_request_at.store(now, Ordering::Relaxed);

        // Reset consecutive success counter on failure
        self.state.consecutive_successes.store(0, Ordering::Relaxed);

        // Request failures may degrade an available deployment, but must not
        // overwrite a stronger state owned by health probes or cooldown logic.
        let mut current = self.state.health.load(Ordering::Relaxed);
        while matches!(
            HealthStatus::from(current),
            HealthStatus::Healthy | HealthStatus::Unknown
        ) {
            match self.state.health.compare_exchange_weak(
                current,
                HealthStatus::Degraded as u8,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
        counters
    }

    pub(crate) fn promote_to_healthy_if_degraded(&self) {
        let mut current = self.state.health.load(Ordering::Relaxed);
        while current == HealthStatus::Degraded as u8 {
            match self.state.health.compare_exchange_weak(
                current,
                HealthStatus::Healthy as u8,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
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

        let mut current = self.state.health.load(Ordering::Relaxed);
        while HealthStatus::from(current) != HealthStatus::Cooldown {
            match self.state.health.compare_exchange_weak(
                current,
                HealthStatus::Cooldown as u8,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }
}

/// Get current Unix timestamp in seconds
///
/// Returns the number of seconds since UNIX_EPOCH.
pub(crate) fn current_timestamp() -> u64 {
    current_timestamp_duration().as_secs()
}

fn current_timestamp_millis() -> u64 {
    current_timestamp_duration()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn current_timestamp_duration() -> std::time::Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
}

#[cfg(test)]
#[path = "deployment/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "deployment_window_tests.rs"]
mod deployment_window_tests;
