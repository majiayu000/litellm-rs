use super::*;
use crate::core::providers::openai::OpenAIProvider;
use crate::core::router::{DeploymentConfig, RouterConfig, UnifiedRoutingStrategy};
use std::sync::Mutex;

async fn router() -> Arc<UnifiedRouter> {
    let router = Arc::new(UnifiedRouter::new(RouterConfig {
        routing_strategy: UnifiedRoutingStrategy::PriorityBased,
        num_retries: 1,
        ..Default::default()
    }));
    let provider = Provider::OpenAI(OpenAIProvider::with_api_key("sk-test").await.unwrap());
    for (id, priority) in [("first", 0), ("second", 1)] {
        router.add_deployment(
            Deployment::new(
                id.into(),
                provider.clone(),
                "gpt-4o-mini".into(),
                "shared".into(),
            )
            .with_config(DeploymentConfig {
                priority,
                ..Default::default()
            }),
        );
    }
    router
}

#[tokio::test]
async fn operation_idempotency_controls_pre_header_failover() {
    for (idempotency, expected) in [
        (RequestIdempotency::NonIdempotent, vec!["first"]),
        (RequestIdempotency::Idempotent, vec!["first", "second"]),
    ] {
        let router = router().await;
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let result = execute_stream_with_selected_deployment_matching_with_idempotency(
            router.clone(),
            "shared",
            ProviderCapability::Responses,
            |_| true,
            idempotency,
            {
                let attempts = attempts.clone();
                move |_, _, deployment| {
                    let attempts = attempts.clone();
                    async move {
                        attempts.lock().unwrap().push(deployment);
                        Err::<(), _>(ProviderError::network(
                            "openai",
                            "accepted POST lost headers",
                        ))
                    }
                }
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(GatewayError::Provider(ProviderError::Network { .. }))
        ));
        assert_eq!(*attempts.lock().unwrap(), expected);
        for id in ["first", "second"] {
            assert_eq!(
                router
                    .get_deployment(id)
                    .unwrap()
                    .state
                    .active_requests
                    .load(std::sync::atomic::Ordering::Relaxed),
                0
            );
        }
    }
}

#[tokio::test]
async fn non_idempotent_creation_still_allows_preflight_budget_fallback() {
    let router = router().await;
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let (selected, lease) = execute_stream_with_selected_deployment_matching_with_idempotency(
        router,
        "shared",
        ProviderCapability::Responses,
        |_| true,
        RequestIdempotency::NonIdempotent,
        {
            let attempts = attempts.clone();
            move |_, _, deployment| {
                let attempts = attempts.clone();
                async move {
                    attempts.lock().unwrap().push(deployment.clone());
                    if deployment == "first" {
                        Err(ProviderError::quota_exceeded(
                            "budget",
                            "provider 'first' budget exceeded",
                        ))
                    } else {
                        Ok(deployment)
                    }
                }
            }
        },
    )
    .await
    .unwrap();
    assert_eq!(selected, "second");
    assert_eq!(*attempts.lock().unwrap(), ["first", "second"]);
    lease.finish_success(0).await;
}
