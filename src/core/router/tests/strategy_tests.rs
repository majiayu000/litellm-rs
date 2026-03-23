//! Strategy selection tests

use super::router_tests::create_test_deployment;
use crate::core::router::config::{RouterConfig, RoutingStrategy};
use crate::core::router::deployment::HealthStatus;
use crate::core::router::error::RouterError;
use crate::core::router::unified::Router;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn test_select_deployment_simple_shuffle() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("test-1", "gpt-4").await;
    let d2 = create_test_deployment("test-2", "gpt-4").await;
    let d3 = create_test_deployment("test-3", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d3.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);
    router.add_deployment(d3);

    let mut selections = std::collections::HashMap::new();
    for _ in 0..100 {
        let result = router.select_deployment("gpt-4");
        assert!(result.is_ok());
        let id = result.unwrap();
        *selections.entry(id.clone()).or_insert(0) += 1;
        router.release_deployment(&id);
    }

    assert!(selections.contains_key("test-1"));
    assert!(selections.contains_key("test-2"));
    assert!(selections.contains_key("test-3"));
}

#[tokio::test]
async fn test_select_deployment_least_busy() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::LeastBusy,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("test-1", "gpt-4").await;
    let d2 = create_test_deployment("test-2", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

    d1.state.active_requests.store(5, Ordering::Relaxed);
    d2.state.active_requests.store(2, Ordering::Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);

    let result = router.select_deployment("gpt-4").unwrap();
    assert_eq!(result, "test-2");
    router.release_deployment(&result);
}

#[tokio::test]
async fn test_select_deployment_usage_based() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::UsageBased,
        ..Default::default()
    };
    let router = Router::new(config);

    let mut d1 = create_test_deployment("test-1", "gpt-4").await;
    let mut d2 = create_test_deployment("test-2", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

    d1.config.tpm_limit = Some(10000);
    d1.state.tpm_current.store(8000, Ordering::Relaxed);

    d2.config.tpm_limit = Some(10000);
    d2.state.tpm_current.store(3000, Ordering::Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);

    let result = router.select_deployment("gpt-4").unwrap();
    assert_eq!(result, "test-2");
    router.release_deployment(&result);
}

#[tokio::test]
async fn test_select_deployment_latency_based() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::LatencyBased,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("test-1", "gpt-4").await;
    let d2 = create_test_deployment("test-2", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

    d1.state.avg_latency_us.store(100_000, Ordering::Relaxed);
    d2.state.avg_latency_us.store(50_000, Ordering::Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);

    let result = router.select_deployment("gpt-4").unwrap();
    assert_eq!(result, "test-2");
    router.release_deployment(&result);
}

#[tokio::test]
async fn test_select_deployment_round_robin() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::RoundRobin,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("test-1", "gpt-4").await;
    let d2 = create_test_deployment("test-2", "gpt-4").await;
    let d3 = create_test_deployment("test-3", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d3.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);
    router.add_deployment(d3);

    let r1 = router.select_deployment("gpt-4").unwrap();
    let r2 = router.select_deployment("gpt-4").unwrap();
    let r3 = router.select_deployment("gpt-4").unwrap();
    let r4 = router.select_deployment("gpt-4").unwrap();

    assert_ne!(r1, r2);
    assert_ne!(r2, r3);
    assert_ne!(r1, r3);
    assert_eq!(r1, r4);

    router.release_deployment(&r1);
    router.release_deployment(&r2);
    router.release_deployment(&r3);
    router.release_deployment(&r4);
}

#[tokio::test]
async fn test_select_deployment_priority_based() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::PriorityBased,
        ..Default::default()
    };
    let router = Router::new(config);

    let mut d1 = create_test_deployment("test-1", "gpt-4").await;
    let mut d2 = create_test_deployment("test-2", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

    d1.config.priority = 2;
    d2.config.priority = 1;

    router.add_deployment(d1);
    router.add_deployment(d2);

    let result = router.select_deployment("gpt-4").unwrap();
    assert_eq!(result, "test-2");
    router.release_deployment(&result);
}

#[tokio::test]
async fn test_select_deployment_rate_limit_aware() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::RateLimitAware,
        ..Default::default()
    };
    let router = Router::new(config);

    let mut d1 = create_test_deployment("test-1", "gpt-4").await;
    let mut d2 = create_test_deployment("test-2", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

    d1.config.tpm_limit = Some(10000);
    d1.state.tpm_current.store(9000, Ordering::Relaxed);

    d2.config.tpm_limit = Some(10000);
    d2.state.tpm_current.store(3000, Ordering::Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);

    let result = router.select_deployment("gpt-4").unwrap();
    assert_eq!(result, "test-2");
    router.release_deployment(&result);
}

#[tokio::test]
async fn test_select_deployment_model_not_found() {
    let router = Router::default();

    let result = router.select_deployment("nonexistent-model");
    assert!(result.is_err());

    match result {
        Err(RouterError::ModelNotFound(model)) => {
            assert_eq!(model, "nonexistent-model");
        }
        _ => panic!("Expected ModelNotFound error"),
    }
}

#[tokio::test]
async fn test_select_deployment_no_available() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;

    deployment
        .state
        .health
        .store(HealthStatus::Unhealthy as u8, Ordering::Relaxed);
    router.add_deployment(deployment);

    let result = router.select_deployment("gpt-4");
    assert!(result.is_err());

    match result {
        Err(RouterError::NoAvailableDeployment(model)) => {
            assert_eq!(model, "gpt-4");
        }
        _ => panic!("Expected NoAvailableDeployment error"),
    }
}

#[tokio::test]
async fn test_select_deployment_respects_parallel_limit() {
    let router = Router::default();
    let mut deployment = create_test_deployment("test-1", "gpt-4").await;

    deployment
        .state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    deployment.config.max_parallel_requests = Some(2);
    deployment.state.active_requests.store(2, Ordering::Relaxed);

    router.add_deployment(deployment);

    let result = router.select_deployment("gpt-4");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_select_deployment_respects_rate_limit() {
    let router = Router::default();
    let mut deployment = create_test_deployment("test-1", "gpt-4").await;

    deployment
        .state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    deployment.config.tpm_limit = Some(1000);
    deployment.state.tpm_current.store(1000, Ordering::Relaxed);

    router.add_deployment(deployment);

    let result = router.select_deployment("gpt-4");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_release_deployment() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;

    deployment
        .state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    deployment.state.active_requests.store(5, Ordering::Relaxed);

    router.add_deployment(deployment);

    router.release_deployment("test-1");

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(d.state.active_requests.load(Ordering::Relaxed), 4);
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_select_deployment_with_alias() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;

    deployment
        .state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

    router.add_deployment(deployment);
    router.add_model_alias("gpt4", "gpt-4").unwrap();

    let result = router.select_deployment("gpt4");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test-1");
}

#[tokio::test]
async fn test_routing_metrics_provider_selected_and_strategy_used() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::RoundRobin,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("test-1", "gpt-4").await;
    let d2 = create_test_deployment("test-2", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);

    // Make 10 selections
    for _ in 0..10 {
        let id = router.select_deployment("gpt-4").unwrap();
        router.release_deployment(&id);
    }

    let metrics = router.routing_metrics();

    // Both deployments should have been selected
    let total_selected: u64 = metrics.provider_selected.values().sum();
    assert_eq!(total_selected, 10, "Expected 10 total selections");

    // Strategy should be recorded as RoundRobin
    let rr_count = metrics.strategy_used.get("RoundRobin").copied().unwrap_or(0);
    assert_eq!(rr_count, 10, "Expected 10 RoundRobin strategy uses");

    // No fallbacks triggered in simple selection
    assert_eq!(metrics.fallback_triggered, 0);
}
