// Retry/failover state-machine tests for deployment-selected execution.

use super::{execute_stream_with_selected_deployment, execute_with_selected_deployment};
use crate::core::providers::Provider;
use crate::core::providers::ProviderError;
use crate::core::providers::anthropic::{AnthropicConfig, AnthropicProvider};
use crate::core::providers::openai::OpenAIProvider;
use crate::core::router::RouterConfig;
use crate::core::router::{Deployment, DeploymentConfig, UnifiedRouter, UnifiedRoutingStrategy};
use crate::core::types::model::ProviderCapability;
use crate::utils::error::gateway_error::GatewayError;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

async fn build_retry_failover_router() -> UnifiedRouter {
    build_retry_failover_router_with_config(
        RouterConfig {
            routing_strategy: UnifiedRoutingStrategy::PriorityBased,
            num_retries: 3,
            // Keep the cooldown breaker out of the picture so any failover in
            // these tests is driven by per-request exclusion alone.
            allowed_fails: 100,
            ..Default::default()
        },
        None,
    )
    .await
}

pub(super) async fn build_retry_failover_router_with_config(
    config: RouterConfig,
    fallback_max_parallel_requests: Option<u32>,
) -> UnifiedRouter {
    let router = UnifiedRouter::new(config);
    let primary = Provider::Anthropic(
        AnthropicProvider::new(AnthropicConfig::new("sk-test-key"))
            .expect("test provider should build"),
    );
    let fallback = Provider::OpenAI(
        OpenAIProvider::with_api_key("sk-test-key")
            .await
            .expect("test provider should build"),
    );
    let fast_retries = crate::core::router::deployment::RetrySchedule {
        base_delay_ms: 1,
        max_delay_ms: 5,
        backoff_multiplier: 2.0,
        jitter_ratio: 0.0,
    };

    router.add_deployment(
        Deployment::new(
            "primary-retry-target".to_string(),
            primary,
            "claude-3-haiku".to_string(),
            "shared-model".to_string(),
        )
        .with_config(DeploymentConfig {
            priority: 0,
            retry_schedule: Some(fast_retries.clone()),
            ..Default::default()
        }),
    );
    router.add_deployment(
        Deployment::new(
            "fallback-retry-target".to_string(),
            fallback,
            "gpt-4o-mini".to_string(),
            "shared-model".to_string(),
        )
        .with_config(DeploymentConfig {
            priority: 10,
            retry_schedule: Some(fast_retries),
            max_parallel_requests: fallback_max_parallel_requests,
            ..Default::default()
        }),
    );

    router
}

#[tokio::test]
async fn test_execute_with_selected_deployment_failover_excludes_failed_deployment() {
    let router = build_retry_failover_router().await;
    let attempts = Arc::new(Mutex::new(Vec::new()));

    let result = execute_with_selected_deployment(
        &router,
        "shared-model",
        ProviderCapability::ChatCompletion,
        {
            let attempts = attempts.clone();
            move |provider, model, _deployment_id| {
                let attempts = attempts.clone();
                async move {
                    let provider_name = provider.name().to_string();
                    attempts.lock().unwrap().push(provider_name.clone());
                    if provider_name == "anthropic" {
                        Err(ProviderError::timeout("anthropic", "primary timed out"))
                    } else {
                        Ok(((provider_name, model), 0))
                    }
                }
            }
        },
    )
    .await
    .expect("retry should fail over to an untried deployment");

    assert_eq!(result.0, "openai");
    assert_eq!(attempts.lock().unwrap().as_slice(), ["anthropic", "openai"]);
}

#[tokio::test]
async fn test_execute_stream_failover_excludes_failed_deployment() {
    let router = Arc::new(build_retry_failover_router().await);
    let attempts = Arc::new(Mutex::new(Vec::new()));

    let ((provider_name, _model), lease) = execute_stream_with_selected_deployment(
        router.clone(),
        "shared-model",
        ProviderCapability::ChatCompletionStream,
        {
            let attempts = attempts.clone();
            move |provider, model, _selected_deployment_id| {
                let attempts = attempts.clone();
                async move {
                    let provider_name = provider.name().to_string();
                    attempts.lock().unwrap().push(provider_name.clone());
                    if provider_name == "anthropic" {
                        Err(ProviderError::timeout("anthropic", "primary timed out"))
                    } else {
                        Ok((provider_name, model))
                    }
                }
            }
        },
    )
    .await
    .expect("stream retry should fail over to an untried deployment");

    assert_eq!(provider_name, "openai");
    assert_eq!(attempts.lock().unwrap().as_slice(), ["anthropic", "openai"]);
    lease.finish_success(0);
}

#[tokio::test]
async fn test_execute_with_selected_deployment_single_target_still_retries() {
    let router = build_single_retry_target_router().await;
    let attempts = Arc::new(Mutex::new(Vec::new()));

    // The only deployment fails once with a retryable error; the request
    // must still be retried against it via the full-pool fallback.
    let result =
        execute_with_selected_deployment(&router, "gpt-4", ProviderCapability::ChatCompletion, {
            let attempts = attempts.clone();
            move |_provider, _model, deployment_id| {
                let attempts = attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment_id);
                    let first_attempt = attempts.lock().unwrap().len() == 1;
                    if first_attempt {
                        Err(ProviderError::timeout("openai", "transient timeout"))
                    } else {
                        Ok((String::from("ok"), 0))
                    }
                }
            }
        })
        .await
        .expect("single-deployment setup should still get same-target retries");

    assert_eq!(result, "ok");
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["deployment-1", "deployment-1"]
    );
    let deployment = router
        .get_deployment("deployment-1")
        .expect("deployment should exist");
    assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
}

async fn build_single_retry_target_router() -> UnifiedRouter {
    let router = UnifiedRouter::default();
    let provider = Provider::OpenAI(
        OpenAIProvider::with_api_key("sk-test-key")
            .await
            .expect("test provider should build"),
    );
    router.add_deployment(
        Deployment::new(
            "deployment-1".to_string(),
            provider,
            "gpt-4o-mini".to_string(),
            "gpt-4".to_string(),
        )
        .with_config(DeploymentConfig {
            retry_schedule: Some(crate::core::router::deployment::RetrySchedule {
                base_delay_ms: 1,
                max_delay_ms: 5,
                backoff_multiplier: 2.0,
                jitter_ratio: 0.0,
            }),
            ..Default::default()
        }),
    );

    router
}

#[tokio::test]
async fn test_execute_stream_single_target_still_retries() {
    let router = Arc::new(build_single_retry_target_router().await);
    let attempts = Arc::new(Mutex::new(Vec::new()));

    let (result, lease) = execute_stream_with_selected_deployment(
        router.clone(),
        "gpt-4",
        ProviderCapability::ChatCompletionStream,
        {
            let attempts = attempts.clone();
            move |_provider, _model, deployment_id| {
                let attempts = attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment_id);
                    let first_attempt = attempts.lock().unwrap().len() == 1;
                    if first_attempt {
                        Err(ProviderError::timeout("openai", "transient timeout"))
                    } else {
                        Ok(String::from("ok"))
                    }
                }
            }
        },
    )
    .await
    .expect("single streaming deployment should still get same-target retries");

    assert_eq!(result, "ok");
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["deployment-1", "deployment-1"]
    );
    let deployment = router
        .get_deployment("deployment-1")
        .expect("deployment should exist");
    assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
    drop(deployment);
    lease.finish_success(0);
}

#[tokio::test]
async fn test_execute_with_selected_deployment_rotates_soft_exclusions_each_sweep() {
    let router = build_retry_failover_router().await;
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let fallback_attempts = Arc::new(Mutex::new(0_u32));

    let result = execute_with_selected_deployment(
        &router,
        "shared-model",
        ProviderCapability::ChatCompletion,
        {
            let attempts = attempts.clone();
            let fallback_attempts = fallback_attempts.clone();
            move |_provider, _model, deployment_id| {
                let attempts = attempts.clone();
                let fallback_attempts = fallback_attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment_id.clone());
                    if deployment_id == "fallback-retry-target" {
                        let mut count = fallback_attempts.lock().unwrap();
                        *count += 1;
                        if *count == 2 {
                            return Ok((String::from("fallback recovered"), 0));
                        }
                    }
                    Err(ProviderError::timeout("test", "transient timeout"))
                }
            }
        },
    )
    .await
    .expect("the fallback must remain reachable in the second retry sweep");

    assert_eq!(result, "fallback recovered");
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        [
            "primary-retry-target",
            "fallback-retry-target",
            "primary-retry-target",
            "fallback-retry-target",
        ]
    );
}

#[tokio::test]
async fn test_execute_stream_rotates_soft_exclusions_each_sweep() {
    let router = Arc::new(build_retry_failover_router().await);
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let fallback_attempts = Arc::new(Mutex::new(0_u32));

    let (result, lease) = execute_stream_with_selected_deployment(
        router.clone(),
        "shared-model",
        ProviderCapability::ChatCompletionStream,
        {
            let attempts = attempts.clone();
            let fallback_attempts = fallback_attempts.clone();
            move |_provider, _model, deployment_id| {
                let attempts = attempts.clone();
                let fallback_attempts = fallback_attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment_id.clone());
                    if deployment_id == "fallback-retry-target" {
                        let mut count = fallback_attempts.lock().unwrap();
                        *count += 1;
                        if *count == 2 {
                            return Ok(String::from("fallback recovered"));
                        }
                    }
                    Err(ProviderError::timeout("test", "transient timeout"))
                }
            }
        },
    )
    .await
    .expect("the streaming fallback must remain reachable in the second retry sweep");

    assert_eq!(result, "fallback recovered");
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        [
            "primary-retry-target",
            "fallback-retry-target",
            "primary-retry-target",
            "fallback-retry-target",
        ]
    );
    lease.finish_success(0);
}

async fn release_fallback_at_attempt_two_deadline<T>(
    handle: &tokio::task::JoinHandle<T>,
    fallback: &Deployment,
) {
    // The first provider retry uses its one-millisecond deployment schedule.
    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    tokio::task::yield_now().await;

    // Attempt 2 must use the router's exact two-second exponential delay.
    // Keep the untried fallback busy until one millisecond before that
    // deadline. A wrong one-second delay consumes the final attempt here.
    tokio::time::advance(std::time::Duration::from_millis(1_999)).await;
    tokio::task::yield_now().await;
    assert!(
        !handle.is_finished(),
        "execution must remain pending before the attempt-2 deadline"
    );

    fallback.state.active_requests.store(0, Ordering::Relaxed);
    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    tokio::task::yield_now().await;
    assert!(
        handle.is_finished(),
        "execution must resume at the exact attempt-2 deadline"
    );
}

#[tokio::test(start_paused = true)]
async fn test_execute_with_selected_deployment_waits_for_temporarily_unavailable_untried_target() {
    let router = Arc::new(
        build_retry_failover_router_with_config(
            RouterConfig {
                routing_strategy: UnifiedRoutingStrategy::PriorityBased,
                num_retries: 2,
                retry_after_secs: 0,
                allowed_fails: 100,
                ..Default::default()
            },
            Some(1),
        )
        .await,
    );
    let fallback = router
        .get_deployment("fallback-retry-target")
        .expect("fallback deployment should exist");
    fallback.state.active_requests.store(1, Ordering::Relaxed);
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let primary_entered = Arc::new(tokio::sync::Notify::new());
    let handle = {
        let router = router.clone();
        let attempts = attempts.clone();
        let primary_entered = primary_entered.clone();
        tokio::spawn(async move {
            execute_with_selected_deployment(
                router.as_ref(),
                "shared-model",
                ProviderCapability::ChatCompletion,
                move |_provider, _model, deployment_id| {
                    let attempts = attempts.clone();
                    let primary_entered = primary_entered.clone();
                    async move {
                        attempts.lock().unwrap().push(deployment_id.clone());
                        if deployment_id == "primary-retry-target" {
                            primary_entered.notify_one();
                            Err(ProviderError::timeout("test", "primary timed out"))
                        } else {
                            Ok((String::from("fallback recovered"), 0))
                        }
                    }
                },
            )
            .await
        })
    };

    primary_entered.notified().await;
    release_fallback_at_attempt_two_deadline(&handle, fallback.as_ref()).await;
    let result = handle
        .await
        .expect("unary execution task should finish")
        .expect("a temporarily unavailable untried target should be retried");

    assert_eq!(result, "fallback recovered");
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["primary-retry-target", "fallback-retry-target"]
    );
}

#[tokio::test(start_paused = true)]
async fn test_execute_stream_waits_for_temporarily_unavailable_untried_target() {
    let router = Arc::new(
        build_retry_failover_router_with_config(
            RouterConfig {
                routing_strategy: UnifiedRoutingStrategy::PriorityBased,
                num_retries: 2,
                retry_after_secs: 0,
                allowed_fails: 100,
                ..Default::default()
            },
            Some(1),
        )
        .await,
    );
    let fallback = router
        .get_deployment("fallback-retry-target")
        .expect("fallback deployment should exist");
    fallback.state.active_requests.store(1, Ordering::Relaxed);
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let primary_entered = Arc::new(tokio::sync::Notify::new());
    let handle = {
        let router = router.clone();
        let attempts = attempts.clone();
        let primary_entered = primary_entered.clone();
        tokio::spawn(async move {
            execute_stream_with_selected_deployment(
                router,
                "shared-model",
                ProviderCapability::ChatCompletionStream,
                move |_provider, _model, deployment_id| {
                    let attempts = attempts.clone();
                    let primary_entered = primary_entered.clone();
                    async move {
                        attempts.lock().unwrap().push(deployment_id.clone());
                        if deployment_id == "primary-retry-target" {
                            primary_entered.notify_one();
                            Err(ProviderError::timeout("test", "primary timed out"))
                        } else {
                            Ok(String::from("fallback recovered"))
                        }
                    }
                },
            )
            .await
        })
    };

    primary_entered.notified().await;
    release_fallback_at_attempt_two_deadline(&handle, fallback.as_ref()).await;
    let (result, lease) = handle
        .await
        .expect("stream execution task should finish")
        .expect("a temporarily unavailable streaming target should be retried");

    assert_eq!(result, "fallback recovered");
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["primary-retry-target", "fallback-retry-target"]
    );
    lease.finish_success(0);
}

#[tokio::test]
async fn test_execute_with_selected_deployment_preserves_last_operation_error_at_attempt_limit() {
    let router = build_retry_failover_router_with_config(
        RouterConfig {
            routing_strategy: UnifiedRoutingStrategy::PriorityBased,
            num_retries: 1,
            allowed_fails: 100,
            ..Default::default()
        },
        Some(1),
    )
    .await;
    let fallback = router
        .get_deployment("fallback-retry-target")
        .expect("fallback deployment should exist");
    fallback.state.active_requests.store(1, Ordering::Relaxed);
    let attempts = Arc::new(Mutex::new(Vec::new()));

    let error = execute_with_selected_deployment(
        &router,
        "shared-model",
        ProviderCapability::ChatCompletion,
        {
            let attempts = attempts.clone();
            move |_provider, _model, deployment_id| {
                let attempts = attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment_id);
                    Err::<(String, u64), _>(ProviderError::timeout("test", "primary timed out"))
                }
            }
        },
    )
    .await
    .expect_err("the final selection failure must stop at max_attempts");

    assert!(matches!(
        error,
        GatewayError::Provider(ProviderError::Timeout {
            provider: "test",
            ref message,
        }) if message == "primary timed out"
    ));
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["primary-retry-target"]
    );
}

#[tokio::test]
async fn test_execute_stream_preserves_last_operation_error_at_attempt_limit() {
    let router = Arc::new(
        build_retry_failover_router_with_config(
            RouterConfig {
                routing_strategy: UnifiedRoutingStrategy::PriorityBased,
                num_retries: 1,
                allowed_fails: 100,
                ..Default::default()
            },
            Some(1),
        )
        .await,
    );
    let fallback = router
        .get_deployment("fallback-retry-target")
        .expect("fallback deployment should exist");
    fallback.state.active_requests.store(1, Ordering::Relaxed);
    let attempts = Arc::new(Mutex::new(Vec::new()));

    let result = execute_stream_with_selected_deployment(
        router,
        "shared-model",
        ProviderCapability::ChatCompletionStream,
        {
            let attempts = attempts.clone();
            move |_provider, _model, deployment_id| {
                let attempts = attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment_id);
                    Err::<String, _>(ProviderError::timeout("test", "primary timed out"))
                }
            }
        },
    )
    .await;
    let error = match result {
        Err(error) => error,
        Ok((_stream, lease)) => {
            drop(lease);
            panic!("the final streaming selection failure must stop at max_attempts");
        }
    };

    assert!(matches!(
        error,
        GatewayError::Provider(ProviderError::Timeout {
            provider: "test",
            ref message,
        }) if message == "primary timed out"
    ));
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["primary-retry-target"]
    );
}
