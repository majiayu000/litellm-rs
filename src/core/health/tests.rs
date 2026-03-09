//! Tests for health monitoring system

#[cfg(test)]
use crate::core::health::{
    monitor::{HealthMonitor, HealthMonitorConfig},
    provider::{ProviderHealth, SystemHealth},
    types::{HealthCheckResult, HealthStatus},
};
use std::collections::HashMap;

#[test]
fn test_health_status_scoring() {
    assert_eq!(HealthStatus::Healthy.score(), 100);
    assert_eq!(HealthStatus::Degraded.score(), 70);
    assert_eq!(HealthStatus::Unhealthy.score(), 30);
    assert_eq!(HealthStatus::Down.score(), 0);

    assert!(HealthStatus::Healthy.allows_requests());
    assert!(HealthStatus::Degraded.allows_requests());
    assert!(!HealthStatus::Unhealthy.allows_requests());
    assert!(!HealthStatus::Down.allows_requests());
}

#[test]
fn test_provider_health_update() {
    let mut provider = ProviderHealth::new("test-provider".to_string());

    // Start healthy
    assert_eq!(provider.status, HealthStatus::Healthy);
    assert_eq!(provider.consecutive_failures, 0);

    // Add unhealthy result
    let unhealthy_result = HealthCheckResult::unhealthy("test error".to_string(), 1000);
    provider.update(unhealthy_result);

    assert_eq!(provider.status, HealthStatus::Unhealthy);
    assert_eq!(provider.consecutive_failures, 1);

    // Add healthy result
    let healthy_result = HealthCheckResult::healthy(500);
    provider.update(healthy_result);

    assert_eq!(provider.status, HealthStatus::Healthy);
    assert_eq!(provider.consecutive_failures, 0);
}

#[test]
fn test_provider_routing_weight() {
    let mut provider = ProviderHealth::new("test-provider".to_string());

    // Healthy provider should have high weight
    let healthy_result = HealthCheckResult::healthy(100);
    provider.update(healthy_result);
    let weight = provider.routing_weight();
    assert!(weight > 0.8);

    // Unhealthy provider should have zero weight
    provider.status = HealthStatus::Down;
    let weight = provider.routing_weight();
    assert_eq!(weight, 0.0);
}

#[tokio::test]
async fn test_health_monitor_registration() {
    let config = HealthMonitorConfig {
        auto_check_enabled: false,
        ..Default::default()
    };
    let monitor = HealthMonitor::new(config);

    monitor.register_provider("test-provider".to_string()).await;

    let health = monitor.get_provider_health("test-provider").await;
    assert!(health.is_some());
    assert_eq!(health.unwrap().provider_id, "test-provider");
}

#[test]
fn test_system_health() {
    let mut providers = HashMap::new();
    providers.insert(
        "provider1".to_string(),
        ProviderHealth::new("provider1".to_string()),
    );

    let mut provider2 = ProviderHealth::new("provider2".to_string());
    provider2.status = HealthStatus::Unhealthy;
    providers.insert("provider2".to_string(), provider2);

    let system_health = SystemHealth::new(providers);
    assert_eq!(system_health.overall_status(), HealthStatus::Degraded);

    let metrics = system_health.metrics();
    assert_eq!(metrics.get("total_providers"), Some(&2.0));
    assert_eq!(metrics.get("healthy_providers"), Some(&1.0));
}
