// Error-precedence tests for mixed hard and soft deployment exclusions.

use super::failover_tests::build_retry_failover_router_with_config;
use super::{execute_stream_with_selected_deployment, execute_with_selected_deployment};
use crate::core::providers::ProviderError;
use crate::core::router::{RouterConfig, UnifiedRouter, UnifiedRoutingStrategy};
use crate::core::types::model::ProviderCapability;
use crate::utils::error::gateway_error::GatewayError;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

fn assert_mixed_exclusion_result(
    router: &UnifiedRouter,
    attempts: &Mutex<Vec<String>>,
    error: GatewayError,
    expected_message: &str,
) {
    assert!(matches!(
        error,
        GatewayError::Provider(ProviderError::QuotaExceeded {
            provider: "budget",
            ref message,
        }) if message == expected_message
    ));
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["primary-retry-target", "fallback-retry-target"]
    );
    assert_all_leases_released(router);
}

fn assert_all_leases_released(router: &UnifiedRouter) {
    for deployment_id in ["primary-retry-target", "fallback-retry-target"] {
        let deployment = router
            .get_deployment(deployment_id)
            .expect("deployment should exist");
        assert_eq!(
            deployment.state.active_requests.load(Ordering::Relaxed),
            0,
            "{deployment_id} lease should be released"
        );
    }
}

async fn build_mixed_exclusion_router(num_retries: u32) -> Arc<UnifiedRouter> {
    Arc::new(
        build_retry_failover_router_with_config(
            RouterConfig {
                routing_strategy: UnifiedRoutingStrategy::PriorityBased,
                num_retries,
                allowed_fails: 100,
                ..Default::default()
            },
            None,
        )
        .await,
    )
}

#[tokio::test]
async fn test_unary_temporary_full_pool_exhaustion_preserves_last_budget_error() {
    let router = build_mixed_exclusion_router(2).await;
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let expected_message = "provider 'openai' mixed budget exhausted";

    let error = execute_with_selected_deployment(
        router.as_ref(),
        "shared-model",
        ProviderCapability::ChatCompletion,
        {
            let router = router.clone();
            let attempts = attempts.clone();
            move |_provider, _model, deployment_id| {
                let router = router.clone();
                let attempts = attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment_id.clone());
                    if deployment_id == "primary-retry-target" {
                        return Err::<(String, u64), _>(ProviderError::timeout(
                            "test",
                            "primary timed out",
                        ));
                    }

                    router
                        .get_deployment("primary-retry-target")
                        .expect("primary deployment should exist")
                        .enter_cooldown(30);
                    Err::<(String, u64), _>(ProviderError::quota_exceeded(
                        "budget",
                        "provider 'openai' mixed budget exhausted",
                    ))
                }
            }
        },
    )
    .await
    .expect_err("temporary full-pool exhaustion should preserve the last budget error");

    assert_mixed_exclusion_result(router.as_ref(), attempts.as_ref(), error, expected_message);
}

#[tokio::test]
async fn test_stream_temporary_full_pool_exhaustion_preserves_last_budget_error() {
    let router = build_mixed_exclusion_router(2).await;
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let expected_message = "provider 'openai' streaming mixed budget exhausted";

    let result = execute_stream_with_selected_deployment(
        router.clone(),
        "shared-model",
        ProviderCapability::ChatCompletionStream,
        {
            let router = router.clone();
            let attempts = attempts.clone();
            move |_provider, _model, deployment_id| {
                let router = router.clone();
                let attempts = attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment_id.clone());
                    if deployment_id == "primary-retry-target" {
                        return Err::<String, _>(ProviderError::timeout(
                            "test",
                            "primary timed out",
                        ));
                    }

                    router
                        .get_deployment("primary-retry-target")
                        .expect("primary deployment should exist")
                        .enter_cooldown(30);
                    Err::<String, _>(ProviderError::quota_exceeded(
                        "budget",
                        "provider 'openai' streaming mixed budget exhausted",
                    ))
                }
            }
        },
    )
    .await;
    let error = match result {
        Err(error) => error,
        Ok((_stream, lease)) => {
            drop(lease);
            panic!("temporary full-pool exhaustion should not start a stream");
        }
    };

    assert_mixed_exclusion_result(router.as_ref(), attempts.as_ref(), error, expected_message);
}

#[tokio::test]
async fn test_unary_newer_soft_error_supersedes_older_hard_exclusion_error() {
    let router = build_mixed_exclusion_router(1).await;
    let attempts = Arc::new(Mutex::new(Vec::new()));

    let error = execute_with_selected_deployment(
        router.as_ref(),
        "shared-model",
        ProviderCapability::ChatCompletion,
        {
            let router = router.clone();
            let attempts = attempts.clone();
            move |_provider, _model, deployment_id| {
                let router = router.clone();
                let attempts = attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment_id.clone());
                    if deployment_id == "primary-retry-target" {
                        return Err::<(String, u64), _>(ProviderError::quota_exceeded(
                            "budget",
                            "provider 'anthropic' older budget exhaustion",
                        ));
                    }

                    router
                        .get_deployment("fallback-retry-target")
                        .expect("fallback deployment should exist")
                        .enter_cooldown(30);
                    Err::<(String, u64), _>(ProviderError::timeout(
                        "openai",
                        "newer fallback timeout",
                    ))
                }
            }
        },
    )
    .await
    .expect_err("the newer provider timeout should survive selection exhaustion");

    assert!(matches!(
        error,
        GatewayError::Provider(ProviderError::Timeout {
            provider: "openai",
            ref message,
        }) if message == "newer fallback timeout"
    ));
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["primary-retry-target", "fallback-retry-target"]
    );
    assert_all_leases_released(router.as_ref());
}

#[tokio::test]
async fn test_stream_newer_soft_error_supersedes_older_hard_exclusion_error() {
    let router = build_mixed_exclusion_router(1).await;
    let attempts = Arc::new(Mutex::new(Vec::new()));

    let result = execute_stream_with_selected_deployment(
        router.clone(),
        "shared-model",
        ProviderCapability::ChatCompletionStream,
        {
            let router = router.clone();
            let attempts = attempts.clone();
            move |_provider, _model, deployment_id| {
                let router = router.clone();
                let attempts = attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment_id.clone());
                    if deployment_id == "primary-retry-target" {
                        return Err::<String, _>(ProviderError::quota_exceeded(
                            "budget",
                            "provider 'anthropic' older streaming budget exhaustion",
                        ));
                    }

                    router
                        .get_deployment("fallback-retry-target")
                        .expect("fallback deployment should exist")
                        .enter_cooldown(30);
                    Err::<String, _>(ProviderError::timeout(
                        "openai",
                        "newer streaming fallback timeout",
                    ))
                }
            }
        },
    )
    .await;
    let error = match result {
        Err(error) => error,
        Ok((_stream, lease)) => {
            drop(lease);
            panic!("selection exhaustion should not start a stream");
        }
    };

    assert!(matches!(
        error,
        GatewayError::Provider(ProviderError::Timeout {
            provider: "openai",
            ref message,
        }) if message == "newer streaming fallback timeout"
    ));
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["primary-retry-target", "fallback-retry-target"]
    );
    assert_all_leases_released(router.as_ref());
}
