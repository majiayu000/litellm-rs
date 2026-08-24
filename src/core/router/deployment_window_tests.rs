//! Deployment and per-minute counter-window tests.

use super::*;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Barrier};

async fn create_test_deployment()
-> Result<Deployment, crate::core::providers::unified_provider::ProviderError> {
    Ok(Deployment::new(
        "window-test".to_string(),
        Provider::OpenAI(
            crate::core::providers::openai::OpenAIProvider::with_api_key(
                "sk-test-key-for-unit-testing-only",
            )
            .await?,
        ),
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    ))
}

#[test]
fn active_window_preserves_per_minute_counters() {
    let state = DeploymentState::new();
    state.tpm_current.store(1000, Ordering::Relaxed);
    state.rpm_current.store(50, Ordering::Relaxed);
    state.fails_this_minute.store(5, Ordering::Relaxed);

    // `DeploymentState::new` starts an active window; rolling must not
    // reset anything while it has not elapsed.
    state.roll_minute_window(current_timestamp());

    assert_eq!(state.tpm_current.load(Ordering::Relaxed), 1000);
    assert_eq!(state.rpm_current.load(Ordering::Relaxed), 50);
    assert_eq!(state.fails_this_minute.load(Ordering::Relaxed), 5);
}

#[test]
fn roll_is_idempotent_within_one_window() {
    let state = DeploymentState::new();
    state.tpm_current.store(10, Ordering::Relaxed);

    let stale = current_timestamp().saturating_sub(61);
    state.minute_reset_at.store(stale, Ordering::Relaxed);

    let now = current_timestamp();
    state.roll_minute_window(now);
    state.roll_minute_window(now);

    assert_eq!(state.tpm_current.load(Ordering::Relaxed), 0);
    assert!(state.minute_reset_at.load(Ordering::Relaxed) >= stale + 60);
}

#[test]
fn elapsed_window_resets_all_counters_before_publishing_timestamp() {
    let state = DeploymentState::new();
    let now = current_timestamp();
    state.tpm_current.store(1000, Ordering::Relaxed);
    state.rpm_current.store(50, Ordering::Relaxed);
    state.fails_this_minute.store(5, Ordering::Relaxed);
    state.minute_reset_at.store(now - 61, Ordering::Relaxed);

    let counters = state.minute_counters(now);

    assert_eq!(
        counters,
        MinuteCounters {
            tpm: 0,
            rpm: 0,
            failures: 0
        }
    );
    assert!(state.minute_reset_at.load(Ordering::Acquire) >= now);
}

#[test]
fn stale_roll_observer_cannot_erase_new_window_updates() {
    let state = DeploymentState::new();
    let now = current_timestamp();
    state.minute_reset_at.store(now - 61, Ordering::Relaxed);

    state.roll_minute_window(now);
    state.rpm_current.store(1, Ordering::Relaxed);
    let published_at = state.minute_reset_at.load(Ordering::Acquire);
    state.roll_minute_window(now - 61);

    assert_eq!(state.rpm_current.load(Ordering::Relaxed), 1);
    assert_eq!(state.minute_reset_at.load(Ordering::Acquire), published_at);
}

#[test]
fn wall_clock_rollback_rebases_the_counter_window() {
    let state = DeploymentState::new();
    let now = current_timestamp();
    state.rpm_current.store(10, Ordering::Relaxed);
    state.minute_reset_at.store(now + 30, Ordering::Relaxed);

    let counters = state.minute_counters(now);

    assert_eq!(counters.rpm, 0);
    let rebased_at = state.minute_reset_at.load(Ordering::Acquire);
    assert!(rebased_at >= now);
    assert!(rebased_at < now + 30);
}

#[tokio::test]
async fn late_success_is_recorded_in_the_new_window()
-> Result<(), crate::core::providers::unified_provider::ProviderError> {
    let deployment = create_test_deployment().await?;
    deployment.state.tpm_current.store(900, Ordering::Relaxed);
    deployment.state.rpm_current.store(9, Ordering::Relaxed);
    deployment
        .state
        .fails_this_minute
        .store(3, Ordering::Relaxed);
    deployment
        .state
        .minute_reset_at
        .store(current_timestamp() - 61, Ordering::Relaxed);

    deployment.record_success(25, 100);

    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 25);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
    assert_eq!(
        deployment.state.fails_this_minute.load(Ordering::Relaxed),
        0
    );
    Ok(())
}

#[tokio::test]
async fn late_failure_is_recorded_in_the_new_window()
-> Result<(), crate::core::providers::unified_provider::ProviderError> {
    let deployment = create_test_deployment().await?;
    deployment.state.tpm_current.store(900, Ordering::Relaxed);
    deployment.state.rpm_current.store(9, Ordering::Relaxed);
    deployment
        .state
        .fails_this_minute
        .store(3, Ordering::Relaxed);
    deployment
        .state
        .minute_reset_at
        .store(current_timestamp() - 61, Ordering::Relaxed);

    deployment.record_failure();

    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
    assert_eq!(
        deployment.state.fails_this_minute.load(Ordering::Relaxed),
        1
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_writers_cannot_be_erased_by_rollover()
-> Result<(), crate::core::providers::unified_provider::ProviderError> {
    const WRITERS: usize = 32;
    let deployment = Arc::new(create_test_deployment().await?);
    deployment
        .state
        .minute_reset_at
        .store(current_timestamp() - 61, Ordering::Relaxed);
    let barrier = Arc::new(Barrier::new(WRITERS));
    let mut handles = Vec::with_capacity(WRITERS);

    for index in 0..WRITERS {
        let deployment = deployment.clone();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            if index % 2 == 0 {
                deployment.record_success(10, 100);
            } else {
                deployment.record_failure();
            }
        }));
    }
    for handle in handles {
        handle.join().expect("counter writer should not panic");
    }

    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 160);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 16);
    assert_eq!(
        deployment.state.fails_this_minute.load(Ordering::Relaxed),
        16
    );
    Ok(())
}

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
fn test_roll_minute_window_resets_after_elapsed_window() {
    let state = DeploymentState::new();
    state.tpm_current.store(1000, Ordering::Relaxed);
    state.rpm_current.store(50, Ordering::Relaxed);
    state.fails_this_minute.store(5, Ordering::Relaxed);

    let stale = current_timestamp().saturating_sub(61);
    state.minute_reset_at.store(stale, Ordering::Relaxed);

    state.roll_minute_window(current_timestamp());

    assert_eq!(state.tpm_current.load(Ordering::Relaxed), 0);
    assert_eq!(state.rpm_current.load(Ordering::Relaxed), 0);
    assert_eq!(state.fails_this_minute.load(Ordering::Relaxed), 0);
    assert!(state.minute_reset_at.load(Ordering::Relaxed) > stale);
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

#[tokio::test]
async fn test_deployment_clone_shares_runtime_state()
-> Result<(), crate::core::providers::unified_provider::ProviderError> {
    let deployment = create_test_deployment().await?;
    let cloned = deployment.clone();
    cloned.state.active_requests.store(7, Ordering::Relaxed);

    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 7);
    Ok(())
}

#[test]
fn test_current_timestamp() {
    let ts = current_timestamp();
    assert!(ts > 0);
    assert!(ts > 1577836800);
}

#[test]
fn test_current_timestamp_monotonic() {
    let ts1 = current_timestamp();
    let ts2 = current_timestamp();
    assert!(ts2 >= ts1);
}
