//! Health monitor implementation
//!
//! This module provides the main HealthMonitor struct and its core methods
//! for managing provider health monitoring.

use super::provider::ProviderHealth;
use crate::utils::error::recovery::circuit_breaker::CircuitBreaker;
use crate::utils::error::recovery::types::CircuitBreakerConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

/// Health monitor configuration
#[derive(Debug, Clone)]
pub struct HealthMonitorConfig {
    /// Interval between health checks
    pub check_interval: Duration,
    /// Timeout for individual health checks
    pub check_timeout: Duration,
    /// Number of failures before marking as unhealthy
    pub failure_threshold: u32,
    /// Number of successes needed to recover from unhealthy
    pub recovery_threshold: u32,
    /// Whether to enable automatic health checks
    pub auto_check_enabled: bool,
    /// Maximum response time before considering degraded
    pub degraded_threshold_ms: u64,
    /// Minimum requests in the window before the circuit breaker can trip.
    ///
    /// Mirrors `RouterConfig::min_requests`: the circuit breaker will not open
    /// until the provider has handled at least this many requests, preventing
    /// premature tripping on small sample sizes.
    pub min_requests: u32,
    /// Consecutive successes required to close the circuit from half-open.
    ///
    /// Mirrors `RouterConfig::success_threshold`: after the open timeout
    /// expires the circuit enters half-open; this many consecutive successful
    /// health-checks must occur before it transitions back to closed.
    pub success_threshold: u32,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            check_timeout: Duration::from_secs(10),
            failure_threshold: 3,
            recovery_threshold: 2,
            auto_check_enabled: true,
            degraded_threshold_ms: 2000,
            min_requests: 10,
            success_threshold: 3,
        }
    }
}

/// Health monitor for tracking provider and service health
pub struct HealthMonitor {
    pub(crate) config: HealthMonitorConfig,
    pub(crate) provider_health: Arc<RwLock<HashMap<String, ProviderHealth>>>,
    /// Circuit breakers stored as Arc for shared access without Clone
    pub(crate) circuit_breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    pub(crate) check_tasks: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(config: HealthMonitorConfig) -> Self {
        Self {
            config,
            provider_health: Arc::new(RwLock::new(HashMap::new())),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            check_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a provider for health monitoring
    pub async fn register_provider(&self, provider_id: String) {
        info!(
            "Registering provider for health monitoring: {}",
            provider_id
        );

        // Initialize provider health
        {
            let mut health = self.provider_health.write().await;
            health.insert(
                provider_id.clone(),
                ProviderHealth::new(provider_id.clone()),
            );
        }

        // Initialize circuit breaker with configured thresholds (wrapped in Arc for shared access)
        {
            let mut breakers = self.circuit_breakers.write().await;
            let breaker_config = CircuitBreakerConfig {
                failure_threshold: self.config.failure_threshold,
                success_threshold: self.config.success_threshold,
                min_requests: self.config.min_requests,
                ..CircuitBreakerConfig::default()
            };
            breakers.insert(
                provider_id.clone(),
                Arc::new(CircuitBreaker::new(breaker_config)),
            );
        }

        // Start health check task if auto-check is enabled
        if self.config.auto_check_enabled {
            self.start_health_check_task(provider_id).await;
        }
    }

    /// Get circuit breaker for a provider
    pub async fn get_circuit_breaker(&self, provider_id: &str) -> Option<Arc<CircuitBreaker>> {
        let breakers = self.circuit_breakers.read().await;
        breakers.get(provider_id).cloned()
    }

    /// Shutdown health monitoring for all providers
    pub async fn shutdown(&self) {
        info!("Shutting down health monitoring");

        // Cancel all health check tasks
        let tasks = {
            let mut task_map = self.check_tasks.write().await;
            task_map.drain().map(|(_, task)| task).collect::<Vec<_>>()
        };

        for task in tasks {
            task.abort();
        }

        info!("Health monitoring shutdown complete");
    }

    /// Start health check task for a provider
    pub(crate) async fn start_health_check_task(&self, provider_id: String) {
        use super::checker::perform_health_check;
        use super::types::HealthCheckResult;
        use std::time::Instant;
        use tokio::time::interval;
        use tracing::debug;

        let provider_health = self.provider_health.clone();
        let check_interval = self.config.check_interval;
        let check_timeout = self.config.check_timeout;
        let degraded_threshold = self.config.degraded_threshold_ms;
        let provider_id_clone = provider_id.clone();

        let task = tokio::spawn(async move {
            let provider_id = provider_id_clone;
            let mut interval = interval(check_interval);

            loop {
                interval.tick().await;

                debug!("Running health check for provider: {}", provider_id);

                let start_time = Instant::now();
                let result =
                    match tokio::time::timeout(check_timeout, perform_health_check(&provider_id))
                        .await
                    {
                        Ok(Ok(response_time)) => {
                            let response_time_ms = response_time.as_millis() as u64;
                            if response_time_ms > degraded_threshold {
                                HealthCheckResult::degraded(
                                    format!("High latency: {}ms", response_time_ms),
                                    response_time_ms,
                                )
                            } else {
                                HealthCheckResult::healthy(response_time_ms)
                            }
                        }
                        Ok(Err(error)) => {
                            let elapsed = start_time.elapsed().as_millis() as u64;
                            HealthCheckResult::unhealthy(error.to_string(), elapsed)
                        }
                        Err(_) => {
                            let elapsed = start_time.elapsed().as_millis() as u64;
                            HealthCheckResult::unhealthy(
                                "Health check timeout".to_string(),
                                elapsed,
                            )
                        }
                    };

                // Update provider health
                let mut health_map = provider_health.write().await;
                if let Some(provider_health) = health_map.get_mut(&provider_id) {
                    provider_health.update(result);
                    debug!(
                        "Updated health for {}: {:?}",
                        provider_id, provider_health.status
                    );
                }
            }
        });

        // Store task handle
        let mut tasks = self.check_tasks.write().await;
        tasks.insert(provider_id, task);
    }
}
