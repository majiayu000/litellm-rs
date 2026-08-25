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
    assert!(config.health_check_policy.is_none());
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
        retry_schedule: None,
        health_check_policy: None,
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

    cloned.total_requests.store(101, Ordering::Relaxed);
    state.success_requests.store(96, Ordering::Relaxed);

    assert_eq!(state.total_requests.load(Ordering::Relaxed), 101);
    assert_eq!(cloned.success_requests.load(Ordering::Relaxed), 96);
}

#[test]
fn provider_identity_survives_clone_and_tracks_replacement_input() {
    let original = DeploymentState::new();
    let cloned = original.clone();
    let snapshot_replacement = original.for_snapshot_insertion();
    assert_eq!(
        original.provider_instance_identity(),
        cloned.provider_instance_identity()
    );
    assert_eq!(
        original.provider_instance_identity(),
        snapshot_replacement.provider_instance_identity()
    );

    let distinct = DeploymentState::new();
    assert_ne!(
        original.provider_instance_identity(),
        distinct.provider_instance_identity()
    );
    let provider_replacement =
        original.for_snapshot_insertion_with_provider(distinct.provider_instance_identity());
    assert_eq!(
        provider_replacement.provider_instance_identity(),
        distinct.provider_instance_identity()
    );
}

#[tokio::test]
async fn test_deployment_clone_shares_runtime_state()
-> Result<(), crate::core::providers::unified_provider::ProviderError> {
    let deployment = Deployment::new(
        "test-1".to_string(),
        Provider::OpenAI(
            crate::core::providers::openai::OpenAIProvider::with_api_key(
                "sk-test-key-for-unit-testing-only",
            )
            .await?,
        ),
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    );

    let cloned = deployment.clone();
    cloned.state.active_requests.store(7, Ordering::Relaxed);

    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 7);
    Ok(())
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
