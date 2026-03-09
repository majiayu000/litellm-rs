//! Execution flow tests

use super::router_tests::create_test_deployment;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::router::config::RouterConfig;
use crate::core::router::error::RouterError;
use crate::core::router::execution::is_retryable_error;
use crate::core::router::fallback::{ExecutionResult, FallbackConfig};
use crate::core::router::unified::Router;
use std::sync::atomic::Ordering;

#[test]
fn test_is_retryable_error() {
    assert!(is_retryable_error(&ProviderError::rate_limit(
        "test",
        Some(60)
    )));
    assert!(is_retryable_error(&ProviderError::timeout(
        "test",
        "Request timed out"
    )));
    assert!(is_retryable_error(&ProviderError::network(
        "test",
        "Connection failed"
    )));
    assert!(is_retryable_error(&ProviderError::ProviderUnavailable {
        provider: "test",
        message: "Service unavailable".to_string(),
    }));

    assert!(!is_retryable_error(&ProviderError::authentication(
        "test",
        "Invalid API key"
    )));
    assert!(!is_retryable_error(&ProviderError::model_not_found(
        "test", "gpt-5"
    )));
    assert!(!is_retryable_error(&ProviderError::invalid_request(
        "test",
        "Bad request"
    )));
}

#[tokio::test]
async fn test_execute_once_success() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let result = router
        .execute_once("gpt-4", |_deployment_id| async move {
            Ok(("success".to_string(), 100u64))
        })
        .await;

    assert!(result.is_ok());
    let exec_result = result.unwrap();
    assert_eq!(exec_result.result, "success");
    assert_eq!(exec_result.attempts, 1);
    assert!(!exec_result.used_fallback);
    assert!(exec_result.latency_us > 0);
}

#[tokio::test]
async fn test_execute_once_deployment_not_found() {
    let router = Router::default();

    let result = router
        .execute_once("gpt-4", |_deployment_id| async move {
            Ok(("success".to_string(), 100u64))
        })
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, RouterError::ModelNotFound(_)));
}

#[tokio::test]
async fn test_execute_once_operation_fails() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let result: Result<ExecutionResult<String>, _> = router
        .execute_once("gpt-4", |_deployment_id| async move {
            Err::<(String, u64), _>(ProviderError::authentication("test", "Invalid API key"))
        })
        .await;

    assert!(result.is_err());

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(d.state.fail_requests.load(Ordering::Relaxed), 1);
    }
}

#[tokio::test]
async fn test_execute_with_retry_success_first_attempt() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let result = router
        .execute_with_retry("gpt-4", |_deployment_id| async move {
            Ok(("success".to_string(), 100u64))
        })
        .await;

    assert!(result.is_ok());
    let (value, deployment_id, attempts, _latency) = result.unwrap();
    assert_eq!(value, "success");
    assert_eq!(attempts, 1);
    assert_eq!(deployment_id, "test-1");
}

#[tokio::test]
async fn test_execute_with_retry_success_second_attempt() {
    let config = RouterConfig {
        num_retries: 3,
        retry_after_secs: 0,
        cooldown_time_secs: 0,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let attempt_count_clone = attempt_count.clone();

    let result = router
        .execute_with_retry("gpt-4", move |_deployment_id| {
            let attempt_count = attempt_count_clone.clone();
            async move {
                let current = attempt_count.fetch_add(1, Ordering::Relaxed);
                if current == 0 {
                    Err(ProviderError::timeout("test", "Request timed out"))
                } else {
                    Ok(("success".to_string(), 100u64))
                }
            }
        })
        .await;

    assert!(result.is_ok());
    let (_value, _deployment_id, attempts, _latency) = result.unwrap();
    assert_eq!(attempts, 2);
}

#[tokio::test]
async fn test_execute_with_retry_exhausted() {
    let config = RouterConfig {
        num_retries: 2,
        retry_after_secs: 0,
        cooldown_time_secs: 0,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let result = router
        .execute_with_retry("gpt-4", |_deployment_id| async move {
            Err::<(String, u64), _>(ProviderError::timeout("test", "Request timed out"))
        })
        .await;

    assert!(result.is_err());
    let (error, attempts) = result.unwrap_err();
    assert_eq!(attempts, 3);
    assert!(matches!(error, ProviderError::Timeout { .. }));
}

#[tokio::test]
async fn test_execute_with_retry_non_retryable_error() {
    let config = RouterConfig {
        num_retries: 3,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let result = router
        .execute_with_retry("gpt-4", |_deployment_id| async move {
            Err::<(String, u64), _>(ProviderError::authentication("test", "Invalid API key"))
        })
        .await;

    assert!(result.is_err());
    let (_error, attempts) = result.unwrap_err();
    assert_eq!(attempts, 1);
}

#[tokio::test]
async fn test_execute_with_fallback() {
    let config = RouterConfig {
        num_retries: 1,
        retry_after_secs: 0,
        max_fallbacks: 2,
        ..Default::default()
    };

    let fallback_config =
        FallbackConfig::new().add_general("gpt-4", vec!["gpt-3.5-turbo".to_string()]);

    let router = Router::new(config).with_fallback_config(fallback_config);

    let deployment1 = create_test_deployment("test-gpt4", "gpt-4").await;
    let deployment2 = create_test_deployment("test-gpt3.5", "gpt-3.5-turbo").await;
    router.add_deployment(deployment1);
    router.add_deployment(deployment2);

    let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let call_count_clone = call_count.clone();

    let result: Result<ExecutionResult<String>, _> = router
        .execute("gpt-4", move |deployment_id| {
            let call_count = call_count_clone.clone();
            async move {
                let count = call_count.fetch_add(1, Ordering::Relaxed);
                if deployment_id.contains("gpt4") {
                    Err(ProviderError::timeout("test", "gpt-4 timed out"))
                } else {
                    Ok((format!("fallback-{}", count), 100u64))
                }
            }
        })
        .await;

    assert!(result.is_ok());
    let exec_result = result.unwrap();
    assert!(exec_result.used_fallback);
    assert!(exec_result.model_used.contains("gpt-3.5-turbo"));
    assert!(exec_result.attempts > 1);
}

#[tokio::test]
async fn test_execute_all_models_fail() {
    let config = RouterConfig {
        num_retries: 1,
        retry_after_secs: 0,
        max_fallbacks: 2,
        ..Default::default()
    };

    let fallback_config =
        FallbackConfig::new().add_general("gpt-4", vec!["gpt-3.5-turbo".to_string()]);

    let router = Router::new(config).with_fallback_config(fallback_config);

    let deployment1 = create_test_deployment("test-gpt4", "gpt-4").await;
    let deployment2 = create_test_deployment("test-gpt3.5", "gpt-3.5-turbo").await;
    router.add_deployment(deployment1);
    router.add_deployment(deployment2);

    let result: Result<ExecutionResult<String>, _> = router
        .execute("gpt-4", |_deployment_id| async move {
            Err::<(String, u64), _>(ProviderError::timeout("test", "All models timed out"))
        })
        .await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        RouterError::NoAvailableDeployment(_)
    ));
}

#[tokio::test]
async fn test_execute_respects_max_fallbacks() {
    let config = RouterConfig {
        num_retries: 0,
        max_fallbacks: 1,
        retry_after_secs: 0,
        ..Default::default()
    };

    let fallback_config = FallbackConfig::new().add_general(
        "gpt-4",
        vec![
            "fallback-1".to_string(),
            "fallback-2".to_string(),
            "fallback-3".to_string(),
        ],
    );

    let router = Router::new(config).with_fallback_config(fallback_config);

    let deployment1 = create_test_deployment("test-gpt4", "gpt-4").await;
    let deployment2 = create_test_deployment("test-fb1", "fallback-1").await;
    let deployment3 = create_test_deployment("test-fb2", "fallback-2").await;
    let deployment4 = create_test_deployment("test-fb3", "fallback-3").await;
    router.add_deployment(deployment1);
    router.add_deployment(deployment2);
    router.add_deployment(deployment3);
    router.add_deployment(deployment4);

    let tried_models = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let tried_models_clone = tried_models.clone();

    let result: Result<ExecutionResult<String>, _> = router
        .execute("gpt-4", move |deployment_id| {
            let tried = tried_models_clone.clone();
            async move {
                tried.lock().unwrap().push(deployment_id.clone());
                Err::<(String, u64), _>(ProviderError::timeout("test", "Failed"))
            }
        })
        .await;

    assert!(result.is_err());

    let models = tried_models.lock().unwrap();
    assert!(
        models.len() <= 2,
        "Tried {} models, expected <= 2",
        models.len()
    );
}

#[tokio::test]
async fn test_execute_records_metrics() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let result = router
        .execute_once("gpt-4", |_deployment_id| async move {
            Ok(("success".to_string(), 500u64))
        })
        .await;

    assert!(result.is_ok());

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(d.state.total_requests.load(Ordering::Relaxed), 1);
        assert!(d.state.avg_latency_us.load(Ordering::Relaxed) > 0);
    }
}

// ==================== Additional Edge Case Tests ====================

#[tokio::test]
async fn test_execute_with_rate_limit_error() {
    let config = RouterConfig {
        num_retries: 2,
        retry_after_secs: 0,
        cooldown_time_secs: 0,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let attempt_count_clone = attempt_count.clone();

    let result = router
        .execute_with_retry("gpt-4", move |_deployment_id| {
            let attempt_count = attempt_count_clone.clone();
            async move {
                let current = attempt_count.fetch_add(1, Ordering::Relaxed);
                if current < 2 {
                    Err(ProviderError::rate_limit("test", Some(1)))
                } else {
                    Ok(("success after rate limit".to_string(), 100u64))
                }
            }
        })
        .await;

    assert!(result.is_ok());
    let (_value, _deployment_id, attempts, _latency) = result.unwrap();
    assert_eq!(attempts, 3);
}

#[tokio::test]
async fn test_execute_deployment_selection_failure() {
    let router = Router::default();

    // No deployments added
    let result = router
        .execute_with_retry("nonexistent-model", |_deployment_id| async move {
            Ok(("should not reach here".to_string(), 100u64))
        })
        .await;

    assert!(result.is_err());
    let (_error, attempts) = result.unwrap_err();
    assert_eq!(attempts, 1); // Should fail immediately on deployment selection
}

#[tokio::test]
async fn test_execute_max_retries_exceeded() {
    let config = RouterConfig {
        num_retries: 3,
        retry_after_secs: 0,
        cooldown_time_secs: 0,
        ..Default::default()
    };
    let router = Router::new(config);
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let attempt_count_clone = attempt_count.clone();

    let result = router
        .execute_with_retry("gpt-4", move |_deployment_id| {
            let attempt_count = attempt_count_clone.clone();
            async move {
                attempt_count.fetch_add(1, Ordering::Relaxed);
                Err::<(String, u64), _>(ProviderError::timeout("test", "Always timeout"))
            }
        })
        .await;

    assert!(result.is_err());
    let (_error, attempts) = result.unwrap_err();
    assert_eq!(attempts, 4); // Initial attempt + 3 retries
}
