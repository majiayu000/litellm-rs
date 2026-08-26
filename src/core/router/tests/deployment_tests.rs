//! Deployment management tests

use crate::core::providers::Provider;
use crate::core::providers::openai::OpenAIProvider;
use crate::core::router::deployment::{Deployment, DeploymentConfig, HealthStatus};
use std::sync::atomic::Ordering;

async fn create_test_provider() -> Provider {
    // Use a properly formatted test key (sk- prefix required by OpenAI provider validation)
    let openai = OpenAIProvider::with_api_key("sk-test-key-for-unit-testing-only")
        .await
        .expect("Failed to create OpenAI provider");
    Provider::OpenAI(openai)
}

#[tokio::test]
async fn test_deployment_creation() {
    let provider = create_test_provider().await;
    let deployment = Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    );

    assert_eq!(deployment.id, "test-deployment");
    assert_eq!(deployment.model, "gpt-4-turbo");
    assert_eq!(deployment.model_name, "gpt-4");
    assert_eq!(deployment.config.weight, 1);
    assert_eq!(deployment.tags.len(), 0);
}

#[tokio::test]
async fn test_deployment_with_config() {
    let provider = create_test_provider().await;
    let config = DeploymentConfig {
        tpm_limit: Some(100_000),
        rpm_limit: Some(500),
        weight: 2,
        priority: 1,
        ..Default::default()
    };

    let deployment = Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    )
    .with_config(config);

    assert_eq!(deployment.config.tpm_limit, Some(100_000));
    assert_eq!(deployment.config.rpm_limit, Some(500));
    assert_eq!(deployment.config.weight, 2);
    assert_eq!(deployment.config.priority, 1);
}

#[tokio::test]
async fn test_deployment_with_tags() {
    let provider = create_test_provider().await;
    let deployment = Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    )
    .with_tags(vec!["production".to_string(), "fast".to_string()]);

    assert_eq!(deployment.tags.len(), 2);
    assert!(deployment.tags.contains(&"production".to_string()));
    assert!(deployment.tags.contains(&"fast".to_string()));
}

#[tokio::test]
async fn test_record_success() {
    let provider = create_test_provider().await;
    let deployment = Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    );

    deployment.record_success(100, 5000);

    assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
    assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 1);
    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 100);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
    assert_eq!(
        deployment.state.avg_latency_us.load(Ordering::Relaxed),
        5000
    );
}

#[tokio::test]
async fn test_record_failure() {
    let provider = create_test_provider().await;
    let deployment = Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    );

    deployment.record_failure();

    assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
    assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
    assert_eq!(
        deployment.state.fails_this_minute.load(Ordering::Relaxed),
        1
    );
    assert_eq!(
        deployment.state.health.load(Ordering::Relaxed),
        HealthStatus::Degraded as u8
    );
}

#[tokio::test]
async fn test_cooldown() {
    let provider = create_test_provider().await;
    let deployment = Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    );

    // Initially not in cooldown
    assert!(!deployment.is_in_cooldown());

    // Enter cooldown for 60 seconds
    deployment.enter_cooldown(60);

    // Should be in cooldown now
    assert!(deployment.is_in_cooldown());
    assert_eq!(
        deployment.state.health.load(Ordering::Relaxed),
        HealthStatus::Cooldown as u8
    );

    // Enter cooldown with 0 duration (effectively immediate exit)
    deployment.enter_cooldown(0);
    assert!(!deployment.is_in_cooldown());
}

#[tokio::test]
async fn test_is_healthy() {
    let provider = create_test_provider().await;
    let deployment = Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    );

    // Starts with Healthy status
    assert!(deployment.is_healthy());

    // Set to Unknown - not healthy
    deployment
        .state
        .health
        .store(HealthStatus::Unknown as u8, Ordering::Relaxed);
    assert!(!deployment.is_healthy());

    // Set to Healthy
    deployment
        .state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    assert!(deployment.is_healthy());

    // Set to Degraded - still considered healthy for routing
    deployment
        .state
        .health
        .store(HealthStatus::Degraded as u8, Ordering::Relaxed);
    assert!(deployment.is_healthy());

    // Set to Unhealthy
    deployment
        .state
        .health
        .store(HealthStatus::Unhealthy as u8, Ordering::Relaxed);
    assert!(!deployment.is_healthy());

    // Set to Cooldown
    deployment
        .state
        .health
        .store(HealthStatus::Cooldown as u8, Ordering::Relaxed);
    assert!(!deployment.is_healthy());
}

#[tokio::test]
async fn test_reset_minute() {
    let provider = create_test_provider().await;
    let deployment = Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    );

    // Record some activity
    deployment.record_success(100, 5000);
    deployment.record_failure();

    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 100);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
    assert_eq!(
        deployment.state.fails_this_minute.load(Ordering::Relaxed),
        1
    );

    // Reset minute
    deployment.state.reset_minute();

    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
    assert_eq!(
        deployment.state.fails_this_minute.load(Ordering::Relaxed),
        0
    );

    // Lifetime counters should not be reset
    assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn test_exponential_moving_average() {
    let provider = create_test_provider().await;
    let deployment = Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    );

    // First request: latency should be set directly
    deployment.record_success(100, 10000);
    assert_eq!(
        deployment.state.avg_latency_us.load(Ordering::Relaxed),
        10000
    );

    // Second request: should calculate EMA
    // EMA = (new + 4*old) / 5 = (20000 + 4*10000) / 5 = 60000 / 5 = 12000
    deployment.record_success(100, 20000);
    assert_eq!(
        deployment.state.avg_latency_us.load(Ordering::Relaxed),
        12000
    );
}

#[test]
fn test_health_status_conversion() {
    assert_eq!(HealthStatus::from(0), HealthStatus::Unknown);
    assert_eq!(HealthStatus::from(1), HealthStatus::Healthy);
    assert_eq!(HealthStatus::from(2), HealthStatus::Degraded);
    assert_eq!(HealthStatus::from(3), HealthStatus::Unhealthy);
    assert_eq!(HealthStatus::from(4), HealthStatus::Cooldown);
    assert_eq!(HealthStatus::from(99), HealthStatus::Unknown);

    assert_eq!(u8::from(HealthStatus::Unknown), 0);
    assert_eq!(u8::from(HealthStatus::Healthy), 1);
    assert_eq!(u8::from(HealthStatus::Degraded), 2);
    assert_eq!(u8::from(HealthStatus::Unhealthy), 3);
    assert_eq!(u8::from(HealthStatus::Cooldown), 4);
}
