//! Health checking methods
//!
//! This module provides health check implementations and methods for
//! updating provider health status.

use super::monitor::HealthMonitor;
use super::provider::ProviderHealth;
use super::types::HealthCheckResult;
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::info;

impl HealthMonitor {
    /// Get health status for a provider
    pub async fn get_provider_health(&self, provider_id: &str) -> Option<ProviderHealth> {
        let health = self.provider_health.read().await;
        health.get(provider_id).cloned()
    }

    /// Get health status for all providers
    pub async fn get_all_provider_health(&self) -> HashMap<String, ProviderHealth> {
        let health = self.provider_health.read().await;
        health.clone()
    }

    /// Manually update provider health
    pub async fn update_provider_health(&self, provider_id: &str, result: HealthCheckResult) {
        let mut health_map = self.provider_health.write().await;
        if let Some(provider_health) = health_map.get_mut(provider_id) {
            provider_health.update(result);
            info!(
                "Manually updated health for {}: {:?}",
                provider_id, provider_health.status
            );
        }
    }
}

/// Perform actual health check for a provider
pub(crate) async fn perform_health_check(provider_id: &str) -> Result<Duration> {
    let start_time = Instant::now();

    // In a real implementation, this would call the provider's health endpoint
    // For now, simulate a health check with variable response times
    let delay = match provider_id {
        id if id.contains("openai") => Duration::from_millis(100 + rand::random::<u64>() % 200),
        id if id.contains("anthropic") => Duration::from_millis(150 + rand::random::<u64>() % 300),
        _ => Duration::from_millis(50 + rand::random::<u64>() % 100),
    };

    tokio::time::sleep(delay).await;

    // Simulate occasional failures
    if rand::random::<f64>() < 0.05 {
        return Err(GatewayError::External(
            "Simulated health check failure".to_string(),
        ));
    }

    Ok(start_time.elapsed())
}
