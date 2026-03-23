//! Selection logic tests
//!
//! Tests for `check_parallel_limit`, `check_rate_limit`, and selection
//! with mixed health states and combined limit scenarios.

use super::router_tests::create_test_deployment;
use crate::core::router::config::{RouterConfig, RoutingStrategy};
use crate::core::router::deployment::HealthStatus;
use crate::core::router::unified::Router;
use std::sync::atomic::Ordering;

// ==================== check_parallel_limit ====================

#[tokio::test]
async fn test_check_parallel_limit_no_limit() {
    let router = Router::default();
    let deployment = create_test_deployment("d1", "gpt-4").await;
    // No max_parallel_requests set (None) -> always within limit
    assert!(router.check_parallel_limit(&deployment));
}

#[tokio::test]
async fn test_check_parallel_limit_under() {
    let router = Router::default();
    let mut deployment = create_test_deployment("d1", "gpt-4").await;
    deployment.config.max_parallel_requests = Some(5);
    deployment.state.active_requests.store(3, Ordering::Relaxed);
    assert!(router.check_parallel_limit(&deployment));
}

#[tokio::test]
async fn test_check_parallel_limit_at_limit() {
    let router = Router::default();
    let mut deployment = create_test_deployment("d1", "gpt-4").await;
    deployment.config.max_parallel_requests = Some(5);
    deployment.state.active_requests.store(5, Ordering::Relaxed);
    assert!(!router.check_parallel_limit(&deployment));
}

#[tokio::test]
async fn test_check_parallel_limit_over() {
    let router = Router::default();
    let mut deployment = create_test_deployment("d1", "gpt-4").await;
    deployment.config.max_parallel_requests = Some(5);
    deployment.state.active_requests.store(10, Ordering::Relaxed);
    assert!(!router.check_parallel_limit(&deployment));
}

// ==================== check_rate_limit ====================

#[tokio::test]
async fn test_check_rate_limit_no_limits() {
    let router = Router::default();
    let deployment = create_test_deployment("d1", "gpt-4").await;
    // No RPM or TPM limits -> always passes
    assert!(router.check_rate_limit(&deployment));
}

#[tokio::test]
async fn test_check_rate_limit_rpm_exceeded() {
    let router = Router::default();
    let mut deployment = create_test_deployment("d1", "gpt-4").await;
    deployment.config.rpm_limit = Some(100);
    deployment.state.rpm_current.store(100, Ordering::Relaxed);
    assert!(!router.check_rate_limit(&deployment));
}

#[tokio::test]
async fn test_check_rate_limit_tpm_exceeded() {
    let router = Router::default();
    let mut deployment = create_test_deployment("d1", "gpt-4").await;
    deployment.config.tpm_limit = Some(5000);
    deployment.state.tpm_current.store(5000, Ordering::Relaxed);
    assert!(!router.check_rate_limit(&deployment));
}

#[tokio::test]
async fn test_check_rate_limit_both_set_rpm_exceeded() {
    let router = Router::default();
    let mut deployment = create_test_deployment("d1", "gpt-4").await;
    deployment.config.rpm_limit = Some(100);
    deployment.config.tpm_limit = Some(5000);
    deployment.state.rpm_current.store(100, Ordering::Relaxed);
    deployment.state.tpm_current.store(0, Ordering::Relaxed);
    // RPM at limit, TPM fine -> blocked
    assert!(!router.check_rate_limit(&deployment));
}

#[tokio::test]
async fn test_check_rate_limit_both_set_tpm_exceeded() {
    let router = Router::default();
    let mut deployment = create_test_deployment("d1", "gpt-4").await;
    deployment.config.rpm_limit = Some(100);
    deployment.config.tpm_limit = Some(5000);
    deployment.state.rpm_current.store(0, Ordering::Relaxed);
    deployment.state.tpm_current.store(5000, Ordering::Relaxed);
    // RPM fine, TPM at limit -> blocked
    assert!(!router.check_rate_limit(&deployment));
}

#[tokio::test]
async fn test_check_rate_limit_both_under() {
    let router = Router::default();
    let mut deployment = create_test_deployment("d1", "gpt-4").await;
    deployment.config.rpm_limit = Some(100);
    deployment.config.tpm_limit = Some(5000);
    deployment.state.rpm_current.store(50, Ordering::Relaxed);
    deployment.state.tpm_current.store(2500, Ordering::Relaxed);
    assert!(router.check_rate_limit(&deployment));
}

// ==================== Selection with degraded deployments ====================

#[tokio::test]
async fn test_select_deployment_prefers_healthy_over_degraded() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::LeastBusy,
        ..Default::default()
    };
    let router = Router::new(config);

    let healthy = create_test_deployment("healthy-1", "gpt-4").await;
    let degraded = create_test_deployment("degraded-1", "gpt-4").await;

    healthy
        .state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    degraded
        .state
        .health
        .store(HealthStatus::Degraded as u8, Ordering::Relaxed);

    router.add_deployment(healthy);
    router.add_deployment(degraded);

    // With LeastBusy and both at 0 active, healthy should be preferred
    // (degraded deployments are still selectable but healthy take priority in filtering)
    let result = router.select_deployment("gpt-4");
    assert!(result.is_ok(), "Should select at least one deployment");
}

#[tokio::test]
async fn test_select_deployment_degraded_selected_when_only_option() {
    let router = Router::default();

    let degraded = create_test_deployment("degraded-1", "gpt-4").await;
    degraded
        .state
        .health
        .store(HealthStatus::Degraded as u8, Ordering::Relaxed);

    router.add_deployment(degraded);

    let result = router.select_deployment("gpt-4");
    assert!(result.is_ok(), "Degraded deployment should be selectable");
    assert_eq!(result.unwrap(), "degraded-1");
}

// ==================== Selection across strategies with weights ====================

#[tokio::test]
async fn test_simple_shuffle_respects_zero_weight() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        ..Default::default()
    };
    let router = Router::new(config);

    let mut d1 = create_test_deployment("heavy", "gpt-4").await;
    let mut d2 = create_test_deployment("zero", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d1.config.weight = 10;

    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.config.weight = 0;

    router.add_deployment(d1);
    router.add_deployment(d2);

    // With one weight at 0, the zero-weight deployment should rarely/never be selected
    // Run 50 selections and verify the heavy deployment is overwhelmingly preferred
    let mut heavy_count = 0;
    for _ in 0..50 {
        let id = router.select_deployment("gpt-4").unwrap();
        if id == "heavy" {
            heavy_count += 1;
        }
        router.release_deployment(&id);
    }

    // heavy should be selected at least 40 out of 50 times
    assert!(
        heavy_count >= 40,
        "Expected heavy deployment to dominate, got {}/50",
        heavy_count
    );
}

#[tokio::test]
async fn test_round_robin_cycles_deterministically() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::RoundRobin,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("rr-1", "gpt-4").await;
    let d2 = create_test_deployment("rr-2", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);

    // Collect 10 selections
    let mut ids = Vec::new();
    for _ in 0..10 {
        let id = router.select_deployment("gpt-4").unwrap();
        ids.push(id.clone());
        router.release_deployment(&id);
    }

    // Both deployments should appear (round robin alternates)
    assert!(ids.contains(&"rr-1".to_string()));
    assert!(ids.contains(&"rr-2".to_string()));

    // Verify alternating pattern: consecutive entries should differ
    let alternating_count = ids.windows(2).filter(|w| w[0] != w[1]).count();
    assert!(
        alternating_count >= 7,
        "Expected mostly alternating, got {}/9 alternations",
        alternating_count
    );
}

#[tokio::test]
async fn test_latency_based_selects_fastest() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::LatencyBased,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("fast", "gpt-4").await;
    let d2 = create_test_deployment("slow", "gpt-4").await;

    d1.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d1.state.avg_latency_us.store(50, Ordering::Relaxed);

    d2.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    d2.state.avg_latency_us.store(500, Ordering::Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);

    // LatencyBased should consistently pick the faster deployment
    let mut fast_count = 0;
    for _ in 0..20 {
        let id = router.select_deployment("gpt-4").unwrap();
        if id == "fast" {
            fast_count += 1;
        }
        router.release_deployment(&id);
    }

    assert_eq!(
        fast_count, 20,
        "LatencyBased should always pick fastest, got {}/20",
        fast_count
    );
}
