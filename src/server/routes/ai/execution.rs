use crate::core::providers::{Provider, ProviderError};
use crate::core::router::UnifiedRouter;
use crate::core::router::deployment::Deployment;
use crate::core::router::execution::{
    infer_cooldown_reason, retryable_budget_scope, router_error_to_provider_error,
};
use crate::core::router::retry_policy::{RetryContext, RetryPolicy};
use crate::core::types::model::ProviderCapability;
use crate::utils::error::gateway_error::GatewayError;
use std::collections::HashSet;
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

pub(super) struct StreamingDeploymentLease {
    router: Arc<UnifiedRouter>,
    deployment: Arc<Deployment>,
    started_at: Instant,
    finalized: bool,
}

impl StreamingDeploymentLease {
    fn new(router: Arc<UnifiedRouter>, deployment: Arc<Deployment>, started_at: Instant) -> Self {
        Self {
            router,
            deployment,
            started_at,
            finalized: false,
        }
    }

    pub(super) fn finish_success(mut self, tokens_used: u64) {
        let latency_us = self.started_at.elapsed().as_micros() as u64;
        self.router
            .record_success_for_deployment(&self.deployment, tokens_used, latency_us);
        self.release();
    }

    pub(super) fn finish_failure(mut self, error: &ProviderError) {
        let cooldown_reason = infer_cooldown_reason(error);
        self.router
            .record_failure_with_reason_for_deployment(&self.deployment, cooldown_reason);
        self.release();
    }

    fn release(&mut self) {
        if !self.finalized {
            UnifiedRouter::release_selected_deployment(&self.deployment);
            self.finalized = true;
        }
    }
}

impl Drop for StreamingDeploymentLease {
    fn drop(&mut self) {
        self.release();
    }
}

pub(super) async fn execute_with_selected_deployment<T, F, Fut>(
    router: &UnifiedRouter,
    requested_model: &str,
    capability: ProviderCapability,
    operation: F,
) -> Result<T, GatewayError>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
{
    let max_attempts = router.config().num_retries + 1;
    let mut attempt = 1;
    let mut last_error = None;
    let mut excluded_budget_deployments = HashSet::new();

    while attempt <= max_attempts {
        let started_at = Instant::now();

        let deployment_lease = match router.select_deployment_lease_for_capability_matching(
            requested_model,
            &capability,
            |deployment| !excluded_budget_deployments.contains(deployment.id.as_str()),
        ) {
            Ok(lease) => lease,
            Err(router_err) => {
                if !excluded_budget_deployments.is_empty()
                    && let Some(err) = last_error.clone()
                {
                    return Err(GatewayError::Provider(err));
                }

                let provider_err = router_error_to_provider_error(router_err);

                let retry_decision = RetryPolicy.decide(
                    router.config(),
                    &provider_err,
                    RetryContext::unary(attempt, max_attempts),
                );
                if retry_decision.should_retry {
                    last_error = Some(provider_err);
                    attempt += 1;
                    if let Some(delay) = retry_decision.delay {
                        tokio::time::sleep(delay).await;
                    }
                    continue;
                }

                return Err(GatewayError::Provider(provider_err));
            }
        };

        let selected_deployment_id = deployment_lease.clone_deployment_id();
        let provider = deployment_lease.deployment().provider.clone();
        let selected_model = deployment_lease.deployment().model.clone();

        match operation.clone()(provider, selected_model, selected_deployment_id).await {
            Ok((value, tokens_used)) => {
                let latency_us = started_at.elapsed().as_micros() as u64;
                router.record_success_for_deployment(
                    deployment_lease.deployment(),
                    tokens_used,
                    latency_us,
                );
                drop(deployment_lease);
                return Ok(value);
            }
            Err(err) => {
                if retryable_budget_scope(&err).is_some()
                    || super::spend::is_model_not_priced_error(&err)
                {
                    excluded_budget_deployments.insert(deployment_lease.clone_deployment_id());
                    drop(deployment_lease);
                    last_error = Some(err);
                    continue;
                }

                let retry_decision = RetryPolicy.decide(
                    router.config(),
                    &err,
                    RetryContext::unary(attempt, max_attempts),
                );
                if retry_decision.should_retry {
                    router.record_failure_with_reason_for_deployment(
                        deployment_lease.deployment(),
                        crate::core::router::CooldownReason::ConsecutiveFailures,
                    );
                    drop(deployment_lease);
                    last_error = Some(err);
                    attempt += 1;
                    if let Some(delay) = retry_decision.delay {
                        tokio::time::sleep(delay).await;
                    }
                    continue;
                }

                let cooldown_reason = infer_cooldown_reason(&err);
                router.record_failure_with_reason_for_deployment(
                    deployment_lease.deployment(),
                    cooldown_reason,
                );
                drop(deployment_lease);
                return Err(GatewayError::Provider(err));
            }
        }
    }

    Err(GatewayError::Provider(last_error.unwrap_or_else(|| {
        ProviderError::Other {
            provider: "router",
            message: "Unknown error during selected deployment retry".to_string(),
        }
    })))
}

#[cfg(test)]
fn retry_delay_for_error(
    config: &crate::core::router::config::RouterConfig,
    attempt: u32,
    error: &ProviderError,
) -> Option<Duration> {
    if retryable_budget_scope(error).is_some() || super::spend::is_model_not_priced_error(error) {
        return None;
    }

    let decision = RetryPolicy.decide(config, error, RetryContext::unary(attempt, attempt + 1));
    if decision.should_retry {
        decision.delay
    } else {
        None
    }
}

#[cfg(test)]
#[path = "execution_retry_delay_tests.rs"]
mod retry_delay_tests;

pub(super) async fn execute_stream_with_selected_deployment<T, F, Fut>(
    router: Arc<UnifiedRouter>,
    requested_model: &str,
    capability: ProviderCapability,
    operation: F,
) -> Result<(T, StreamingDeploymentLease), GatewayError>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
{
    let max_attempts = router.config().num_retries + 1;
    let mut attempt = 1;
    let mut last_error = None;
    let mut excluded_budget_deployments = HashSet::new();

    while attempt <= max_attempts {
        let started_at = Instant::now();

        let deployment_lease = match router.select_deployment_lease_for_capability_matching(
            requested_model,
            &capability,
            |deployment| !excluded_budget_deployments.contains(deployment.id.as_str()),
        ) {
            Ok(lease) => lease,
            Err(router_err) => {
                if !excluded_budget_deployments.is_empty()
                    && let Some(err) = last_error.clone()
                {
                    return Err(GatewayError::Provider(err));
                }

                let provider_err = router_error_to_provider_error(router_err);

                let retry_decision = RetryPolicy.decide(
                    router.config(),
                    &provider_err,
                    RetryContext::stream_pre_output(attempt, max_attempts),
                );
                if retry_decision.should_retry {
                    last_error = Some(provider_err);
                    attempt += 1;
                    if let Some(delay) = retry_decision.delay {
                        tokio::time::sleep(delay).await;
                    }
                    continue;
                }

                return Err(GatewayError::Provider(provider_err));
            }
        };
        let deployment = deployment_lease.clone_deployment();
        let selected_deployment_id = deployment_lease.clone_deployment_id();
        let provider = deployment.provider.clone();
        let selected_model = deployment.model.clone();

        match operation.clone()(provider, selected_model, selected_deployment_id).await {
            Ok(stream) => {
                let _deployment_id = deployment_lease.into_deployment_id();
                let lease = StreamingDeploymentLease::new(router.clone(), deployment, started_at);
                return Ok((stream, lease));
            }
            Err(err) => {
                if retryable_budget_scope(&err).is_some()
                    || super::spend::is_model_not_priced_error(&err)
                {
                    excluded_budget_deployments.insert(deployment_lease.clone_deployment_id());
                    drop(deployment_lease);
                    last_error = Some(err);
                    continue;
                }

                let retry_decision = RetryPolicy.decide(
                    router.config(),
                    &err,
                    RetryContext::stream_pre_output(attempt, max_attempts),
                );
                if retry_decision.should_retry {
                    router.record_failure_with_reason_for_deployment(
                        deployment_lease.deployment(),
                        crate::core::router::CooldownReason::ConsecutiveFailures,
                    );
                    drop(deployment_lease);
                    last_error = Some(err);
                    attempt += 1;
                    if let Some(delay) = retry_decision.delay {
                        tokio::time::sleep(delay).await;
                    }
                    continue;
                }

                let cooldown_reason = infer_cooldown_reason(&err);
                router.record_failure_with_reason_for_deployment(
                    deployment_lease.deployment(),
                    cooldown_reason,
                );
                drop(deployment_lease);
                return Err(GatewayError::Provider(err));
            }
        }
    }

    Err(GatewayError::Provider(last_error.unwrap_or_else(|| {
        ProviderError::Other {
            provider: "router",
            message: "Unknown error during streaming retry".to_string(),
        }
    })))
}

#[cfg(test)]
mod tests {
    use super::{execute_stream_with_selected_deployment, execute_with_selected_deployment};
    use crate::core::providers::Provider;
    use crate::core::providers::ProviderError;
    use crate::core::providers::anthropic::{AnthropicConfig, AnthropicProvider};
    use crate::core::providers::openai::OpenAIProvider;
    use crate::core::router::RouterConfig;
    use crate::core::router::{
        Deployment, DeploymentConfig, HealthStatus, UnifiedRouter, UnifiedRoutingStrategy,
    };
    use crate::core::types::model::ProviderCapability;
    use crate::utils::error::gateway_error::GatewayError;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::Ordering;

    async fn build_test_router() -> UnifiedRouter {
        let router = UnifiedRouter::default();
        let provider = Provider::OpenAI(
            OpenAIProvider::with_api_key("sk-test-key")
                .await
                .expect("test provider should build"),
        );

        router.add_deployment(Deployment::new(
            "deployment-1".to_string(),
            provider,
            "gpt-4o-mini".to_string(),
            "gpt-4".to_string(),
        ));

        router
    }

    async fn build_mixed_capability_router() -> UnifiedRouter {
        let router = UnifiedRouter::new(RouterConfig {
            routing_strategy: UnifiedRoutingStrategy::PriorityBased,
            ..Default::default()
        });

        let chat_only_provider = Provider::Anthropic(
            AnthropicProvider::new(AnthropicConfig::new("sk-test-key"))
                .expect("test provider should build"),
        );
        let embedding_provider = Provider::OpenAI(
            OpenAIProvider::with_api_key("sk-test-key")
                .await
                .expect("test provider should build"),
        );

        router.add_deployment(
            Deployment::new(
                "chat-only".to_string(),
                chat_only_provider,
                "claude-3-haiku".to_string(),
                "shared-model".to_string(),
            )
            .with_config(DeploymentConfig {
                priority: 0,
                ..Default::default()
            }),
        );
        router.add_deployment(
            Deployment::new(
                "embedding-capable".to_string(),
                embedding_provider,
                "text-embedding-3-small".to_string(),
                "shared-model".to_string(),
            )
            .with_config(DeploymentConfig {
                priority: 10,
                ..Default::default()
            }),
        );

        router
    }

    async fn build_provider_budget_fallback_router() -> UnifiedRouter {
        let router = UnifiedRouter::new(RouterConfig {
            routing_strategy: UnifiedRoutingStrategy::PriorityBased,
            num_retries: 1,
            ..Default::default()
        });
        let primary = Provider::Anthropic(
            AnthropicProvider::new(AnthropicConfig::new("sk-test-key"))
                .expect("test provider should build"),
        );
        let fallback = Provider::OpenAI(
            OpenAIProvider::with_api_key("sk-test-key")
                .await
                .expect("test provider should build"),
        );

        router.add_deployment(
            Deployment::new(
                "primary-budget-exhausted".to_string(),
                primary,
                "claude-3-haiku".to_string(),
                "shared-model".to_string(),
            )
            .with_config(DeploymentConfig {
                priority: 0,
                ..Default::default()
            }),
        );
        router.add_deployment(
            Deployment::new(
                "fallback-provider".to_string(),
                fallback,
                "gpt-4o-mini".to_string(),
                "shared-model".to_string(),
            )
            .with_config(DeploymentConfig {
                priority: 10,
                ..Default::default()
            }),
        );

        router
    }

    #[tokio::test]
    async fn test_execute_with_selected_deployment_uses_actual_deployment_model() {
        let router = build_test_router().await;

        let model = execute_with_selected_deployment(
            &router,
            "gpt-4",
            ProviderCapability::ChatCompletion,
            |_provider, model, _deployment_id| async { Ok((model, 0)) },
        )
        .await
        .expect("execution should succeed");

        assert_eq!(model, "gpt-4o-mini");
    }

    #[tokio::test]
    async fn test_execute_with_selected_deployment_uses_capability_selected_deployment() {
        let router = build_mixed_capability_router().await;

        let (provider, model) = execute_with_selected_deployment(
            &router,
            "shared-model",
            ProviderCapability::Embeddings,
            |provider, model, _deployment_id| async move {
                Ok(((provider.name().to_string(), model), 0))
            },
        )
        .await
        .expect("execution should use an embeddings-capable deployment");

        assert_eq!(provider, "openai");
        assert_eq!(model, "text-embedding-3-small");
    }

    #[tokio::test]
    async fn test_execute_with_selected_deployment_rejects_unavailable_capability() {
        let router = build_mixed_capability_router().await;
        let deployment = router
            .get_deployment("embedding-capable")
            .expect("deployment should exist");
        deployment
            .state
            .health
            .store(HealthStatus::Unhealthy as u8, Ordering::Relaxed);
        drop(deployment);

        let err = execute_with_selected_deployment(
            &router,
            "shared-model",
            ProviderCapability::Embeddings,
            |_provider, _model, _deployment_id| async {
                Ok::<_, ProviderError>(("should not run", 0))
            },
        )
        .await
        .expect_err("unavailable capability should fail before execution");

        assert!(matches!(
            err,
            GatewayError::Provider(ProviderError::ProviderUnavailable { .. })
        ));
    }

    #[tokio::test]
    async fn test_execute_with_selected_deployment_rejects_unsupported_capability() {
        let router = build_mixed_capability_router().await;

        let err = execute_with_selected_deployment(
            &router,
            "shared-model",
            ProviderCapability::CodeExecution,
            |_provider, _model, _deployment_id| async {
                Ok::<_, ProviderError>(("should not run", 0))
            },
        )
        .await
        .expect_err("unsupported capability should fail before execution");

        assert!(matches!(
            err,
            GatewayError::Provider(ProviderError::InvalidRequest { .. })
        ));
    }

    #[tokio::test]
    async fn test_execute_with_selected_deployment_maps_provider_error() {
        let router = build_test_router().await;

        let err = execute_with_selected_deployment(
            &router,
            "gpt-4",
            ProviderCapability::ChatCompletion,
            |_provider, _model, _deployment_id| async {
                Err::<(String, u64), _>(ProviderError::timeout("test", "timed out"))
            },
        )
        .await
        .expect_err("provider error should be mapped");

        assert!(matches!(
            err,
            GatewayError::Provider(ProviderError::Timeout { .. })
        ));
    }

    #[tokio::test]
    async fn test_execute_with_selected_deployment_excludes_provider_budget_failures() {
        let router = build_provider_budget_fallback_router().await;
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
                            Err(ProviderError::quota_exceeded(
                                "budget",
                                "provider 'anthropic' budget exceeded",
                            ))
                        } else {
                            Ok(((provider_name, model), 0))
                        }
                    }
                }
            },
        )
        .await
        .expect("fallback provider should be selected after provider budget exhaustion");

        assert_eq!(result.0, "openai");
        assert_eq!(attempts.lock().unwrap().as_slice(), ["anthropic", "openai"]);
        let primary = router
            .get_deployment("primary-budget-exhausted")
            .expect("primary deployment should exist");
        assert_eq!(primary.state.fail_requests.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_execute_with_selected_deployment_excludes_model_budget_failures() {
        let router = build_provider_budget_fallback_router().await;
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
                        attempts.lock().unwrap().push(model.clone());
                        if model == "claude-3-haiku" {
                            Err(ProviderError::quota_exceeded(
                                "budget",
                                "model 'claude-3-haiku' budget exceeded",
                            ))
                        } else {
                            Ok(((provider.name().to_string(), model), 0))
                        }
                    }
                }
            },
        )
        .await
        .expect("fallback model should be selected after model budget exhaustion");

        assert_eq!(result.0, "openai");
        assert_eq!(result.1, "gpt-4o-mini");
        assert_eq!(
            attempts.lock().unwrap().as_slice(),
            ["claude-3-haiku", "gpt-4o-mini"]
        );
        let primary = router
            .get_deployment("primary-budget-exhausted")
            .expect("primary deployment should exist");
        assert_eq!(primary.state.fail_requests.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_execute_stream_holds_deployment_active_until_success() {
        let router = Arc::new(build_test_router().await);

        let (_stream, lease) = execute_stream_with_selected_deployment(
            router.clone(),
            "gpt-4",
            ProviderCapability::ChatCompletionStream,
            |_provider, model, _selected_deployment_id| async move { Ok(model) },
        )
        .await
        .expect("stream creation should succeed");

        let deployment = router
            .get_deployment("deployment-1")
            .expect("deployment should exist");
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 0);
        drop(deployment);

        lease.finish_success(42);

        let deployment = router
            .get_deployment("deployment-1")
            .expect("deployment should exist");
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 42);
    }

    #[tokio::test]
    async fn test_stream_lease_drop_releases_without_recording_outcome() {
        let router = Arc::new(build_test_router().await);

        let (_stream, lease) = execute_stream_with_selected_deployment(
            router.clone(),
            "gpt-4",
            ProviderCapability::ChatCompletionStream,
            |_provider, model, _selected_deployment_id| async move { Ok(model) },
        )
        .await
        .expect("stream creation should succeed");

        drop(lease);

        let deployment = router
            .get_deployment("deployment-1")
            .expect("deployment should exist");
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_execute_stream_records_stream_failure() {
        let router = Arc::new(build_test_router().await);

        let (_stream, lease) = execute_stream_with_selected_deployment(
            router.clone(),
            "gpt-4",
            ProviderCapability::ChatCompletionStream,
            |_provider, model, _selected_deployment_id| async move { Ok(model) },
        )
        .await
        .expect("stream creation should succeed");

        let error = ProviderError::rate_limit("test", Some(1));
        lease.finish_failure(&error);

        let deployment = router
            .get_deployment("deployment-1")
            .expect("deployment should exist");
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_execute_stream_excludes_provider_budget_failures() {
        let router = Arc::new(build_provider_budget_fallback_router().await);
        let attempts = Arc::new(Mutex::new(Vec::new()));

        let ((provider_name, model), lease) = execute_stream_with_selected_deployment(
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
                            Err(ProviderError::quota_exceeded(
                                "budget",
                                "provider 'anthropic' budget exceeded",
                            ))
                        } else {
                            Ok((provider_name, model))
                        }
                    }
                }
            },
        )
        .await
        .expect("fallback stream provider should be selected after provider budget exhaustion");

        assert_eq!(provider_name, "openai");
        assert_eq!(model, "gpt-4o-mini");
        assert_eq!(attempts.lock().unwrap().as_slice(), ["anthropic", "openai"]);
        let primary = router
            .get_deployment("primary-budget-exhausted")
            .expect("primary deployment should exist");
        assert_eq!(primary.state.fail_requests.load(Ordering::Relaxed), 0);
        lease.finish_success(0);
    }

    #[tokio::test]
    async fn test_execute_stream_startup_abort_releases_parallel_slot() {
        let router = Arc::new(UnifiedRouter::default());
        let Ok(openai_provider) = OpenAIProvider::with_api_key("sk-test-key").await else {
            panic!("test provider should build");
        };
        let provider = Provider::OpenAI(openai_provider);
        router.add_deployment(
            Deployment::new(
                "deployment-1".to_string(),
                provider,
                "gpt-4o-mini".to_string(),
                "gpt-4".to_string(),
            )
            .with_config(DeploymentConfig {
                max_parallel_requests: Some(1),
                ..Default::default()
            }),
        );

        let operation_entered = Arc::new(tokio::sync::Notify::new());
        let handle = {
            let router = router.clone();
            let operation_entered = operation_entered.clone();
            tokio::spawn(async move {
                execute_stream_with_selected_deployment(
                    router,
                    "gpt-4",
                    ProviderCapability::ChatCompletionStream,
                    move |_provider, _model, _selected_deployment_id| {
                        let operation_entered = operation_entered.clone();
                        async move {
                            operation_entered.notify_one();
                            std::future::pending::<Result<String, ProviderError>>().await
                        }
                    },
                )
                .await
            })
        };

        operation_entered.notified().await;

        let Some(deployment) = router.get_deployment("deployment-1") else {
            panic!("deployment should exist");
        };
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
        drop(deployment);

        handle.abort();
        match handle.await {
            Err(cancelled) => assert!(cancelled.is_cancelled()),
            Ok(_) => panic!("task should be cancelled"),
        }

        let Some(deployment) = router.get_deployment("deployment-1") else {
            panic!("deployment should exist");
        };
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    }
}
