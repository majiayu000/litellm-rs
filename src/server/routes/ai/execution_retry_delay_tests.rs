use super::*;
use crate::core::providers::Provider;
use crate::core::providers::openai::OpenAIProvider;
use crate::core::router::config::RouterConfig;
use crate::core::router::{Deployment, DeploymentConfig, UnifiedRouter, UnifiedRoutingStrategy};
use crate::core::types::model::ProviderCapability;
use std::sync::{Arc, Mutex};

#[test]
fn budget_retry_fallbacks_skip_retry_delay() {
    let config = RouterConfig {
        retry_after_secs: 5,
        ..Default::default()
    };

    let provider_budget =
        ProviderError::quota_exceeded("budget", "provider 'openai' budget exceeded");
    let model_budget = ProviderError::quota_exceeded("budget", "model 'gpt-4o' budget exceeded");
    let rate_limit = ProviderError::rate_limit("openai", Some(60));

    assert_eq!(retry_delay_for_error(&config, 1, &provider_budget), None);
    assert_eq!(retry_delay_for_error(&config, 1, &model_budget), None);
    assert_eq!(
        retry_delay_for_error(&config, 1, &rate_limit),
        Some(std::time::Duration::from_secs(60))
    );
}

async fn build_same_provider_budget_fallback_router(num_retries: u32) -> UnifiedRouter {
    let router = UnifiedRouter::new(RouterConfig {
        routing_strategy: UnifiedRoutingStrategy::PriorityBased,
        num_retries,
        ..Default::default()
    });
    let primary = Provider::OpenAI(
        OpenAIProvider::with_api_key("sk-test-key")
            .await
            .expect("test provider should build"),
    );
    let fallback = Provider::OpenAI(
        OpenAIProvider::with_api_key("sk-test-key")
            .await
            .expect("test provider should build"),
    );

    router.add_deployment(
        Deployment::new(
            "same-provider-expensive".to_string(),
            primary,
            "gpt-expensive".to_string(),
            "shared-model".to_string(),
        )
        .with_config(DeploymentConfig {
            priority: 0,
            ..Default::default()
        }),
    );
    router.add_deployment(
        Deployment::new(
            "same-provider-cheap".to_string(),
            fallback,
            "gpt-cheap".to_string(),
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
async fn budget_fallback_ignores_retry_limit_and_keeps_same_provider_candidates() {
    let router = build_same_provider_budget_fallback_router(0).await;
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
                    attempts
                        .lock()
                        .unwrap()
                        .push(format!("{}:{model}", provider.name()));
                    if model == "gpt-expensive" {
                        Err(ProviderError::quota_exceeded(
                            "budget",
                            "provider 'openai' budget exceeded",
                        ))
                    } else {
                        Ok((model, 0))
                    }
                }
            }
        },
    )
    .await
    .expect("same-provider budget fallback should not depend on retry count");

    assert_eq!(result, "gpt-cheap");
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["openai:gpt-expensive", "openai:gpt-cheap"]
    );
    let primary = router
        .get_deployment("same-provider-expensive")
        .expect("primary deployment should exist");
    assert_eq!(
        primary
            .state
            .fail_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[tokio::test]
async fn stream_budget_fallback_ignores_retry_limit() {
    let router = Arc::new(build_same_provider_budget_fallback_router(0).await);
    let attempts = Arc::new(Mutex::new(Vec::new()));

    let (model, lease) = execute_stream_with_selected_deployment(
        router.clone(),
        "shared-model",
        ProviderCapability::ChatCompletionStream,
        {
            let attempts = attempts.clone();
            move |provider, model, _selected_deployment_id| {
                let attempts = attempts.clone();
                async move {
                    attempts
                        .lock()
                        .unwrap()
                        .push(format!("{}:{model}", provider.name()));
                    if model == "gpt-expensive" {
                        Err(ProviderError::quota_exceeded(
                            "budget",
                            "provider 'openai' budget exceeded",
                        ))
                    } else {
                        Ok(model)
                    }
                }
            }
        },
    )
    .await
    .expect("stream same-provider budget fallback should not depend on retry count");

    assert_eq!(model, "gpt-cheap");
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        ["openai:gpt-expensive", "openai:gpt-cheap"]
    );
    lease.finish_success(0);
}
