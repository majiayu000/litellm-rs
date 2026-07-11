//! Cooldown and failure handling tests

use super::router_tests::create_test_deployment;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::router::config::RouterConfig;
use crate::core::router::deployment::HealthStatus;
use crate::core::router::error::CooldownReason;
use crate::core::router::unified::Router;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[tokio::test]
async fn test_record_failure_triggers_cooldown() {
    let config = RouterConfig {
        allowed_fails: 3,
        cooldown_time_secs: 10,
        min_requests: 1,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_failure("test-1");
    router.record_failure("test-1");

    if let Some(d) = router.get_deployment("test-1") {
        assert!(!d.is_in_cooldown());
    }

    router.record_failure("test-1");

    if let Some(d) = router.get_deployment("test-1") {
        assert!(d.is_in_cooldown());
        assert_eq!(
            d.state.health.load(Ordering::Relaxed),
            HealthStatus::Cooldown as u8
        );
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_request_cooldown_does_not_override_probe_unhealthy() {
    let config = RouterConfig {
        cooldown_time_secs: 60,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    deployment
        .state
        .probe_unhealthy
        .store(true, Ordering::Relaxed);
    deployment
        .state
        .health
        .store(HealthStatus::Unhealthy as u8, Ordering::Relaxed);
    router.add_deployment(deployment);

    router.record_failure_with_reason("test-1", CooldownReason::RateLimit);

    let deployment = router
        .get_deployment("test-1")
        .expect("deployment should remain registered");
    assert!(deployment.is_in_cooldown());
    assert_eq!(deployment.state.health_status(), HealthStatus::Cooldown);
    deployment.state.cooldown_until.store(1, Ordering::Relaxed);
    assert!(!deployment.is_in_cooldown());
    assert_eq!(deployment.state.health_status(), HealthStatus::Unhealthy);
}

#[tokio::test]
async fn test_cooldown_on_rate_limit() {
    let config = RouterConfig {
        allowed_fails: 3,
        cooldown_time_secs: 10,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_failure_with_reason("test-1", CooldownReason::RateLimit);

    if let Some(d) = router.get_deployment("test-1") {
        assert!(d.is_in_cooldown());
        assert_eq!(
            d.state.health.load(Ordering::Relaxed),
            HealthStatus::Cooldown as u8
        );
        assert_eq!(d.state.fail_requests.load(Ordering::Relaxed), 1);
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_cooldown_on_auth_error() {
    let config = RouterConfig {
        allowed_fails: 3,
        cooldown_time_secs: 10,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_failure_with_reason("test-1", CooldownReason::AuthError);

    if let Some(d) = router.get_deployment("test-1") {
        assert!(d.is_in_cooldown());
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_cooldown_on_consecutive_failures() {
    let config = RouterConfig {
        allowed_fails: 3,
        cooldown_time_secs: 10,
        min_requests: 1,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_failure_with_reason("test-1", CooldownReason::ConsecutiveFailures);
    router.record_failure_with_reason("test-1", CooldownReason::ConsecutiveFailures);

    if let Some(d) = router.get_deployment("test-1") {
        assert!(!d.is_in_cooldown());
        assert_eq!(d.state.fails_this_minute.load(Ordering::Relaxed), 2);
    }

    router.record_failure_with_reason("test-1", CooldownReason::ConsecutiveFailures);

    if let Some(d) = router.get_deployment("test-1") {
        assert!(d.is_in_cooldown());
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_cooldown_on_high_failure_rate() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    for _ in 0..6 {
        router.record_failure("test-1");
    }
    for _ in 0..4 {
        router.record_success("test-1", 100, 50_000);
    }

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(d.state.total_requests.load(Ordering::Relaxed), 10);
        assert_eq!(d.state.fail_requests.load(Ordering::Relaxed), 6);
    }

    router.record_failure_with_reason("test-1", CooldownReason::HighFailureRate);

    if let Some(d) = router.get_deployment("test-1") {
        assert!(d.is_in_cooldown());
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_high_failure_rate_no_cooldown_insufficient_requests() {
    let config = RouterConfig {
        allowed_fails: 10,
        cooldown_time_secs: 10,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    if let Some(d) = router.get_deployment("test-1") {
        d.state.total_requests.store(5, Ordering::Relaxed);
        d.state.fail_requests.store(4, Ordering::Relaxed);
        d.state.success_requests.store(1, Ordering::Relaxed);
        d.state.fails_this_minute.store(4, Ordering::Relaxed);
    }

    router.record_failure_with_reason("test-1", CooldownReason::HighFailureRate);

    if let Some(d) = router.get_deployment("test-1") {
        assert!(!d.is_in_cooldown());
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_timeout_cooldown() {
    let config = RouterConfig {
        allowed_fails: 3,
        cooldown_time_secs: 10,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_failure_with_reason("test-1", CooldownReason::Timeout);

    if let Some(d) = router.get_deployment("test-1") {
        assert!(d.is_in_cooldown());
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_not_found_cooldown() {
    let config = RouterConfig {
        allowed_fails: 3,
        cooldown_time_secs: 10,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_failure_with_reason("test-1", CooldownReason::NotFound);

    if let Some(d) = router.get_deployment("test-1") {
        assert!(d.is_in_cooldown());
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_manual_cooldown() {
    let config = RouterConfig {
        allowed_fails: 3,
        cooldown_time_secs: 10,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_failure_with_reason("test-1", CooldownReason::Manual);

    if let Some(d) = router.get_deployment("test-1") {
        assert!(d.is_in_cooldown());
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_infer_cooldown_reason_rate_limit() {
    let error = ProviderError::rate_limit("test_provider", Some(60));
    let reason = Router::infer_cooldown_reason(&error);
    assert_eq!(reason, CooldownReason::RateLimit);
}

#[tokio::test]
async fn test_infer_cooldown_reason_auth_error() {
    let error = ProviderError::authentication("test_provider", "Invalid API key");
    let reason = Router::infer_cooldown_reason(&error);
    assert_eq!(reason, CooldownReason::AuthError);
}

#[tokio::test]
async fn test_infer_cooldown_reason_not_found() {
    let error = ProviderError::model_not_found("test_provider", "gpt-5");
    let reason = Router::infer_cooldown_reason(&error);
    assert_eq!(reason, CooldownReason::NotFound);
}

#[tokio::test]
async fn test_infer_cooldown_reason_timeout() {
    let error = ProviderError::timeout("test_provider", "Request timed out");
    let reason = Router::infer_cooldown_reason(&error);
    assert_eq!(reason, CooldownReason::Timeout);
}

#[tokio::test]
async fn test_infer_cooldown_reason_api_error_429() {
    let error = ProviderError::api_error("test_provider", 429, "Too many requests");
    let reason = Router::infer_cooldown_reason(&error);
    assert_eq!(reason, CooldownReason::RateLimit);
}

#[tokio::test]
async fn test_infer_cooldown_reason_api_error_401() {
    let error = ProviderError::api_error("test_provider", 401, "Unauthorized");
    let reason = Router::infer_cooldown_reason(&error);
    assert_eq!(reason, CooldownReason::AuthError);
}

#[tokio::test]
async fn test_infer_cooldown_reason_generic_error() {
    let error = ProviderError::network("test_provider", "Connection failed");
    let reason = Router::infer_cooldown_reason(&error);
    assert_eq!(reason, CooldownReason::ConsecutiveFailures);
}

#[test]
fn test_cooldown_reason_equality() {
    assert_eq!(CooldownReason::RateLimit, CooldownReason::RateLimit);
    assert_ne!(CooldownReason::RateLimit, CooldownReason::AuthError);
    assert_eq!(CooldownReason::Manual, CooldownReason::Manual);
}

#[tokio::test]
async fn test_minute_reset_task_integration() {
    let router = Arc::new(Router::default());
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let task = router.clone().start_minute_reset_task();

    tokio::time::sleep(Duration::from_millis(100)).await;

    router.record_success("test-1", 1000, 50_000);
    router.record_failure("test-1");

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(d.state.tpm_current.load(Ordering::Relaxed), 1000);
        assert_eq!(d.state.rpm_current.load(Ordering::Relaxed), 1);
    }

    task.abort();
}

#[tokio::test]
async fn test_min_requests_prevents_premature_cooldown() {
    // With min_requests=10, 3 failures out of only 3 total should NOT trip
    let config = RouterConfig {
        allowed_fails: 3,
        cooldown_time_secs: 10,
        min_requests: 10,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_failure("test-1");
    router.record_failure("test-1");
    router.record_failure("test-1");

    if let Some(d) = router.get_deployment("test-1") {
        // 3 fails >= allowed_fails(3), but total_this_minute(3) < min_requests(10)
        assert!(!d.is_in_cooldown());
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_min_requests_met_triggers_cooldown() {
    // With min_requests=5, enough total traffic allows the breaker to trip
    let config = RouterConfig {
        allowed_fails: 3,
        cooldown_time_secs: 10,
        min_requests: 5,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    // Record 3 successes first to build up total traffic
    for _ in 0..3 {
        router.record_success("test-1", 100, 50_000);
    }

    // Now record 3 failures (total_this_minute = 3 success + 3 fail = 6 >= 5)
    router.record_failure("test-1");
    router.record_failure("test-1");
    router.record_failure("test-1");

    if let Some(d) = router.get_deployment("test-1") {
        assert!(d.is_in_cooldown());
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_success_threshold_delays_promotion() {
    // success_threshold=3 means Degraded -> Healthy requires 3 consecutive successes
    let config = RouterConfig {
        allowed_fails: 3,
        cooldown_time_secs: 10,
        success_threshold: 3,
        min_requests: 1,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    // Force into Degraded state
    if let Some(d) = router.get_deployment("test-1") {
        d.state
            .health
            .store(HealthStatus::Degraded as u8, Ordering::Relaxed);
    }

    // First two successes should NOT promote to Healthy
    router.record_success("test-1", 100, 50_000);
    router.record_success("test-1", 100, 50_000);

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(
            d.state.health.load(Ordering::Relaxed),
            HealthStatus::Degraded as u8
        );
    }

    // Third consecutive success promotes to Healthy
    router.record_success("test-1", 100, 50_000);

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(
            d.state.health.load(Ordering::Relaxed),
            HealthStatus::Healthy as u8
        );
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_failure_resets_consecutive_successes() {
    let config = RouterConfig {
        allowed_fails: 100, // high so we don't trip cooldown
        cooldown_time_secs: 10,
        success_threshold: 3,
        min_requests: 1,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    // Force into Degraded state
    if let Some(d) = router.get_deployment("test-1") {
        d.state
            .health
            .store(HealthStatus::Degraded as u8, Ordering::Relaxed);
    }

    // Two successes, then a failure resets the counter
    router.record_success("test-1", 100, 50_000);
    router.record_success("test-1", 100, 50_000);
    router.record_failure("test-1");

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(d.state.consecutive_successes.load(Ordering::Relaxed), 0);
        assert_eq!(
            d.state.health.load(Ordering::Relaxed),
            HealthStatus::Degraded as u8
        );
    }

    // Need 3 fresh consecutive successes
    router.record_success("test-1", 100, 50_000);
    router.record_success("test-1", 100, 50_000);

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(
            d.state.health.load(Ordering::Relaxed),
            HealthStatus::Degraded as u8
        );
    }

    router.record_success("test-1", 100, 50_000);

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(
            d.state.health.load(Ordering::Relaxed),
            HealthStatus::Healthy as u8
        );
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_multiple_deployments_minute_reset() {
    let router = Router::default();
    let deployment1 = create_test_deployment("test-1", "gpt-4").await;
    let deployment2 = create_test_deployment("test-2", "gpt-4").await;
    let deployment3 = create_test_deployment("test-3", "gpt-3.5-turbo").await;

    router.add_deployment(deployment1);
    router.add_deployment(deployment2);
    router.add_deployment(deployment3);

    router.record_success("test-1", 1000, 50_000);
    router.record_success("test-2", 2000, 60_000);
    router.record_failure("test-3");

    router.reset_minute_counters();

    for id in &["test-1", "test-2", "test-3"] {
        if let Some(d) = router.get_deployment(id) {
            assert_eq!(d.state.tpm_current.load(Ordering::Relaxed), 0);
            assert_eq!(d.state.rpm_current.load(Ordering::Relaxed), 0);
            assert_eq!(d.state.fails_this_minute.load(Ordering::Relaxed), 0);
        }
    }
}
