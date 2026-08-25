//! Deployment management tests

use crate::core::providers::Provider;
use crate::core::providers::openai::OpenAIProvider;
use crate::core::router::deployment::{
    Deployment, DeploymentConfig, DeploymentState, HealthStatus,
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
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
async fn test_request_recording_lazily_resets_expired_minute_window() {
    let provider = create_test_provider().await;
    let deployment = Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    );

    deployment.record_success(100, 5000);
    deployment.record_failure();
    deployment.state.minute_reset_at.store(0, Ordering::Release);

    deployment.record_failure();

    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
    assert_eq!(
        deployment.state.fails_this_minute.load(Ordering::Relaxed),
        1
    );
    assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 2);

    deployment.state.reset_minute_if_elapsed();
    assert_eq!(
        deployment.state.fails_this_minute.load(Ordering::Relaxed),
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_requests_stress_lazy_minute_rollover() {
    const REQUESTS: usize = 32;
    let provider = create_test_provider().await;
    let deployment = std::sync::Arc::new(Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    ));
    deployment.state.tpm_current.store(999, Ordering::Relaxed);
    deployment.state.rpm_current.store(999, Ordering::Relaxed);
    deployment
        .state
        .fails_this_minute
        .store(999, Ordering::Relaxed);
    deployment.state.minute_reset_at.store(0, Ordering::Release);

    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(REQUESTS));
    let mut tasks = Vec::with_capacity(REQUESTS);
    for index in 0..REQUESTS {
        let deployment = deployment.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            if index % 2 == 0 {
                deployment.record_success(10, 100);
            } else {
                deployment.record_failure();
            }
        }));
    }
    for task in tasks {
        task.await.expect("recording task should complete");
    }

    assert_eq!(
        deployment.state.rpm_current.load(Ordering::Relaxed),
        (REQUESTS / 2) as u64
    );
    assert_eq!(
        deployment.state.fails_this_minute.load(Ordering::Relaxed),
        (REQUESTS / 2) as u32
    );
    assert_eq!(
        deployment.state.tpm_current.load(Ordering::Relaxed),
        (REQUESTS / 2 * 10) as u64
    );
}

#[test]
fn test_future_minute_timestamp_rebases_after_clock_rollback() {
    let state = DeploymentState::new();
    state.tpm_current.store(100, Ordering::Relaxed);
    state.rpm_current.store(1, Ordering::Relaxed);
    state.fails_this_minute.store(1, Ordering::Relaxed);
    state.minute_reset_at.store(u64::MAX, Ordering::Release);

    let now = state.reset_minute_if_elapsed();

    assert_eq!(state.minute_reset_at.load(Ordering::Acquire), now);
    assert_eq!(state.minute_counters(), (0, 0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_success_and_reset_preserve_minute_counter_tuple() {
    const UPDATES: usize = 2_000;
    let provider = create_test_provider().await;
    let deployment = Arc::new(Deployment::new(
        "test-deployment".to_string(),
        provider,
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
    ));
    let inconsistent = Arc::new(AtomicBool::new(false));

    let writer = {
        let deployment = Arc::clone(&deployment);
        tokio::spawn(async move {
            for _ in 0..UPDATES {
                deployment.record_success(10, 100);
                tokio::task::yield_now().await;
            }
        })
    };
    let resetter = {
        let state = deployment.state.clone();
        tokio::spawn(async move {
            for _ in 0..UPDATES {
                state.reset_minute();
                tokio::task::yield_now().await;
            }
        })
    };
    let observer = {
        let state = deployment.state.clone();
        let inconsistent = Arc::clone(&inconsistent);
        tokio::spawn(async move {
            for _ in 0..UPDATES * 2 {
                let (tpm, rpm, _) = state.minute_counters();
                if tpm != rpm * 10 {
                    inconsistent.store(true, Ordering::Relaxed);
                }
                tokio::task::yield_now().await;
            }
        })
    };

    writer.await.expect("writer should complete");
    resetter.await.expect("resetter should complete");
    observer.await.expect("observer should complete");
    assert!(!inconsistent.load(Ordering::Relaxed));
    assert_eq!(
        deployment.state.total_requests.load(Ordering::Relaxed),
        UPDATES as u64
    );
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
