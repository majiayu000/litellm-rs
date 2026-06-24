//! Shared execution helpers for AI routes.

use crate::core::providers::{Provider, ProviderError};
use crate::core::router::UnifiedRouter;
use crate::core::router::execution::{
    calculate_retry_delay, infer_cooldown_reason, is_retryable_error,
    router_error_to_provider_error,
};
use crate::core::types::model::ProviderCapability;
use crate::utils::error::gateway_error::GatewayError;
use std::sync::Arc;
use std::time::Instant;

/// Holds a selected deployment active for the lifetime of a streaming response.
pub(super) struct StreamingDeploymentLease {
    router: Arc<UnifiedRouter>,
    deployment_id: String,
    started_at: Instant,
    finalized: bool,
}

impl StreamingDeploymentLease {
    fn new(router: Arc<UnifiedRouter>, deployment_id: String, started_at: Instant) -> Self {
        Self {
            router,
            deployment_id,
            started_at,
            finalized: false,
        }
    }

    pub(super) fn finish_success(mut self, tokens_used: u64) {
        let latency_us = self.started_at.elapsed().as_micros() as u64;
        self.router
            .record_success(&self.deployment_id, tokens_used, latency_us);
        self.release();
    }

    pub(super) fn finish_failure(mut self, error: &ProviderError) {
        let cooldown_reason = infer_cooldown_reason(error);
        self.router
            .record_failure_with_reason(&self.deployment_id, cooldown_reason);
        self.release();
    }

    fn release(&mut self) {
        if !self.finalized {
            self.router.release_deployment(&self.deployment_id);
            self.finalized = true;
        }
    }
}

impl Drop for StreamingDeploymentLease {
    fn drop(&mut self) {
        self.release();
    }
}

/// Execute an AI route operation against the deployment selected by `UnifiedRouter`.
///
/// This centralizes the repeated
/// `execute_with_retry -> get_deployment -> provider/model clone` skeleton.
pub(super) async fn execute_with_selected_deployment<T, F, Fut>(
    router: &UnifiedRouter,
    requested_model: &str,
    capability: ProviderCapability,
    operation: F,
) -> Result<T, GatewayError>
where
    F: Fn(Provider, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
{
    let execution = router
        .execute_with_capability_retry(requested_model, &capability, move |deployment_id| {
            let operation = operation.clone();
            async move {
                let deployment = router.get_deployment(&deployment_id).ok_or_else(|| {
                    ProviderError::other("router", "Selected deployment not found")
                })?;

                let provider = deployment.provider.clone();
                let selected_model = deployment.model.clone();
                drop(deployment);

                operation(provider, selected_model).await
            }
        })
        .await
        .map_err(|(e, _)| GatewayError::Provider(e))?;

    Ok(execution.0)
}

/// Start a streaming operation while keeping the selected deployment active.
///
/// The returned lease must live until the stream completes, errors, or is
/// cancelled by client disconnect. Dropping it releases the deployment without
/// recording success or failure; explicit finish methods record final outcome.
pub(super) async fn execute_stream_with_selected_deployment<T, F, Fut>(
    router: Arc<UnifiedRouter>,
    requested_model: &str,
    capability: ProviderCapability,
    operation: F,
) -> Result<(T, StreamingDeploymentLease), GatewayError>
where
    F: Fn(Provider, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
{
    let max_attempts = router.config().num_retries + 1;
    let mut last_error = None;

    for attempt in 1..=max_attempts {
        let started_at = Instant::now();

        let deployment_lease =
            match router.select_deployment_lease_for_capability(requested_model, &capability) {
                Ok(lease) => lease,
                Err(router_err) => {
                    let provider_err = router_error_to_provider_error(router_err);

                    if is_retryable_error(&provider_err) && attempt < max_attempts {
                        let delay = calculate_retry_delay(router.config(), attempt);
                        last_error = Some(provider_err);
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Err(GatewayError::Provider(provider_err));
                }
            };
        let deployment_id = deployment_lease.clone_deployment_id();

        let Some(deployment) = router.get_deployment(&deployment_id) else {
            drop(deployment_lease);
            let err = ProviderError::other("router", "Selected deployment not found");
            if is_retryable_error(&err) && attempt < max_attempts {
                let delay = calculate_retry_delay(router.config(), attempt);
                last_error = Some(err);
                tokio::time::sleep(delay).await;
                continue;
            }
            return Err(GatewayError::Provider(err));
        };

        let provider = deployment.provider.clone();
        let selected_model = deployment.model.clone();
        drop(deployment);

        match operation.clone()(provider, selected_model).await {
            Ok(stream) => {
                let deployment_id = deployment_lease.into_deployment_id();
                let lease =
                    StreamingDeploymentLease::new(router.clone(), deployment_id, started_at);
                return Ok((stream, lease));
            }
            Err(err) => {
                drop(deployment_lease);

                if is_retryable_error(&err) && attempt < max_attempts {
                    router.record_failure_with_reason(
                        &deployment_id,
                        crate::core::router::CooldownReason::ConsecutiveFailures,
                    );
                    let delay = calculate_retry_delay(router.config(), attempt);
                    last_error = Some(err);
                    tokio::time::sleep(delay).await;
                    continue;
                }

                let cooldown_reason = infer_cooldown_reason(&err);
                router.record_failure_with_reason(&deployment_id, cooldown_reason);
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

    #[tokio::test]
    async fn test_execute_with_selected_deployment_uses_actual_deployment_model() {
        let router = build_test_router().await;

        let model = execute_with_selected_deployment(
            &router,
            "gpt-4",
            ProviderCapability::ChatCompletion,
            |_provider, model| async { Ok((model, 0)) },
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
            |provider, model| async move { Ok(((provider.name().to_string(), model), 0)) },
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
            |_provider, _model| async { Ok::<_, ProviderError>(("should not run", 0)) },
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
            ProviderCapability::TextToSpeech,
            |_provider, _model| async { Ok::<_, ProviderError>(("should not run", 0)) },
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
            |_provider, _model| async {
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
    async fn test_execute_stream_holds_deployment_active_until_success() {
        let router = Arc::new(build_test_router().await);

        let (_stream, lease) = execute_stream_with_selected_deployment(
            router.clone(),
            "gpt-4",
            ProviderCapability::ChatCompletionStream,
            |_provider, model| async move { Ok(model) },
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
            |_provider, model| async move { Ok(model) },
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
            |_provider, model| async move { Ok(model) },
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
                    move |_provider, _model| {
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
