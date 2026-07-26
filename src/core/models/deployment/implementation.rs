//! Deployment implementation
//!
//! This module contains the main Deployment struct implementations.

use crate::config::models::provider::ProviderConfig;
use crate::core::models::deployment::health::{CircuitBreakerState, DeploymentHealth};
use crate::core::models::deployment::metrics::DeploymentMetrics;
use crate::core::models::deployment::types::{
    Deployment, DeploymentMetricsSnapshot, DeploymentSnapshot, DeploymentState,
};
use crate::core::models::{HealthStatus, Metadata};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use uuid::Uuid;

impl Deployment {
    /// Create a new deployment
    pub fn new(config: ProviderConfig) -> Self {
        let weight = config.weight;
        let tags = config.tags.clone();

        Self {
            metadata: Metadata::new(),
            config,
            health: Arc::new(DeploymentHealth::new()),
            metrics: Arc::new(DeploymentMetrics::new()),
            state: DeploymentState::Active,
            tags,
            weight,
            rate_limits: None,
            cost_config: None,
        }
    }

    /// Get deployment ID
    pub fn id(&self) -> Uuid {
        self.metadata.id
    }

    /// Get deployment name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get provider type
    pub fn provider_type(&self) -> &str {
        &self.config.provider_type
    }

    /// Check if deployment is available for requests
    pub fn is_available(&self) -> bool {
        matches!(
            self.state,
            DeploymentState::Active | DeploymentState::Degraded
        ) && !matches!(
            *self.health.circuit_breaker.read(),
            CircuitBreakerState::Open
        )
    }

    /// Get current health status
    pub fn health_status(&self) -> HealthStatus {
        *self.health.status.read()
    }

    /// Update health status
    pub fn update_health(&self, status: HealthStatus, response_time_ms: Option<u64>) {
        *self.health.status.write() = status;
        self.health
            .last_check
            .store(chrono::Utc::now().timestamp() as u64, Ordering::Relaxed);

        if let Some(response_time) = response_time_ms {
            self.health
                .avg_response_time
                .store(response_time, Ordering::Relaxed);
        }

        match status {
            HealthStatus::Healthy => {
                self.health.failure_count.store(0, Ordering::Relaxed);
            }
            HealthStatus::Unhealthy => {
                self.health.failure_count.fetch_add(1, Ordering::Relaxed);
                self.health
                    .last_failure
                    .store(chrono::Utc::now().timestamp() as u64, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// Record request metrics
    pub fn record_request(&self, success: bool, tokens: u32, cost: f64, response_time_ms: u64) {
        self.metrics.total_requests.fetch_add(1, Ordering::Relaxed);

        if success {
            self.metrics
                .successful_requests
                .fetch_add(1, Ordering::Relaxed);
        } else {
            self.metrics.failed_requests.fetch_add(1, Ordering::Relaxed);
        }

        self.metrics
            .total_tokens
            .fetch_add(tokens as u64, Ordering::Relaxed);

        {
            let mut total_cost = self.metrics.total_cost.write();
            *total_cost += cost;
        }

        self.metrics
            .last_request
            .store(chrono::Utc::now().timestamp() as u64, Ordering::Relaxed);

        // Update response time metrics (simplified)
        self.metrics
            .avg_response_time
            .store(response_time_ms, Ordering::Relaxed);
    }

    /// Get current metrics snapshot
    pub fn metrics_snapshot(&self) -> DeploymentMetricsSnapshot {
        let total_requests = self.metrics.total_requests.load(Ordering::Relaxed);
        let successful_requests = self.metrics.successful_requests.load(Ordering::Relaxed);
        let failed_requests = self.metrics.failed_requests.load(Ordering::Relaxed);

        let success_rate = if total_requests > 0 {
            (successful_requests as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        DeploymentMetricsSnapshot {
            total_requests,
            successful_requests,
            failed_requests,
            success_rate,
            total_tokens: self.metrics.total_tokens.load(Ordering::Relaxed),
            total_cost: *self.metrics.total_cost.read(),
            active_connections: self.metrics.active_connections.load(Ordering::Relaxed),
            avg_response_time: self.metrics.avg_response_time.load(Ordering::Relaxed),
            p95_response_time: self.metrics.p95_response_time.load(Ordering::Relaxed),
            p99_response_time: self.metrics.p99_response_time.load(Ordering::Relaxed),
            request_rate: self.metrics.request_rate.load(Ordering::Relaxed),
            token_rate: self.metrics.token_rate.load(Ordering::Relaxed),
        }
    }

    /// Create deployment snapshot
    pub fn snapshot(&self) -> DeploymentSnapshot {
        DeploymentSnapshot {
            id: self.id(),
            name: self.name().to_string(),
            provider_type: self.provider_type().to_string(),
            model: self.config.api_key.clone(), // This should be model name
            state: self.state.clone(),
            health_status: self.health_status(),
            weight: self.weight,
            tags: self.tags.clone(),
            metrics: self.metrics_snapshot(),
            updated_at: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::provider::ProviderConfig;
    use std::collections::HashMap;

    #[test]
    fn test_deployment_creation() {
        let config = ProviderConfig {
            name: "test-provider".to_string(),
            provider_type: "openai".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            endpoint_access: Default::default(),
            models: vec!["gpt-4".to_string()],
            timeout: 30,
            max_retries: 3,
            organization: None,
            api_version: None,
            project: None,
            weight: 1.0,
            priority: 0,
            rpm: 1000,
            tpm: 10000,
            enabled: true,
            max_concurrent_requests: 10,
            retry: crate::config::models::provider::RetryConfig::default(),
            health_check: crate::config::models::provider::ProviderHealthCheckConfig::default(),
            settings: HashMap::new(),
            tags: vec!["test".to_string()],
        };

        let deployment = Deployment::new(config);
        assert_eq!(deployment.name(), "test-provider");
        assert_eq!(deployment.provider_type(), "openai");
        assert!(deployment.is_available());
    }

    #[test]
    fn test_metrics_recording() {
        let config = ProviderConfig {
            name: "test-provider".to_string(),
            provider_type: "openai".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            endpoint_access: Default::default(),
            api_version: None,
            organization: None,
            project: None,
            weight: 1.0,
            priority: 0,
            rpm: 1000,
            tpm: 10000,
            max_concurrent_requests: 10,
            timeout: 30,
            max_retries: 3,
            retry: crate::config::models::provider::RetryConfig::default(),
            health_check: crate::config::models::provider::ProviderHealthCheckConfig::default(),
            settings: HashMap::new(),
            models: vec![],
            tags: vec![],
            enabled: true,
        };

        let deployment = Deployment::new(config);
        deployment.record_request(true, 100, 0.01, 250);

        let snapshot = deployment.metrics_snapshot();
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.successful_requests, 1);
        assert_eq!(snapshot.total_tokens, 100);
        assert_eq!(snapshot.total_cost, 0.01);
        assert_eq!(snapshot.success_rate, 100.0);
    }
}
