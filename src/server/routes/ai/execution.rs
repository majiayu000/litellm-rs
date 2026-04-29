//! Shared execution helpers for AI routes.

use crate::core::providers::{Provider, ProviderError};
use crate::core::router::UnifiedRouter;
use crate::utils::error::gateway_error::GatewayError;

/// Execute an AI route operation against the deployment selected by `UnifiedRouter`.
///
/// This centralizes the repeated
/// `execute_with_retry -> get_deployment -> provider/model clone` skeleton.
pub(super) async fn execute_with_selected_deployment<T, F, Fut>(
    router: &UnifiedRouter,
    requested_model: &str,
    operation: F,
) -> Result<T, GatewayError>
where
    F: Fn(Provider, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
{
    let execution = router
        .execute_with_retry(requested_model, move |deployment_id| {
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

#[cfg(test)]
mod tests {
    use super::execute_with_selected_deployment;
    use crate::core::providers::Provider;
    use crate::core::providers::ProviderError;
    use crate::core::providers::openai::OpenAIProvider;
    use crate::core::router::{Deployment, UnifiedRouter};
    use crate::utils::error::gateway_error::GatewayError;

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

    #[tokio::test]
    async fn test_execute_with_selected_deployment_uses_actual_deployment_model() {
        let router = build_test_router().await;

        let model = execute_with_selected_deployment(&router, "gpt-4", |_provider, model| async {
            Ok((model, 0))
        })
        .await
        .expect("execution should succeed");

        assert_eq!(model, "gpt-4o-mini");
    }

    #[tokio::test]
    async fn test_execute_with_selected_deployment_maps_provider_error() {
        let router = build_test_router().await;

        let err = execute_with_selected_deployment(&router, "gpt-4", |_provider, _model| async {
            Err::<(String, u64), _>(ProviderError::timeout("test", "timed out"))
        })
        .await
        .expect_err("provider error should be mapped");

        assert!(matches!(
            err,
            GatewayError::Provider(ProviderError::Timeout { .. })
        ));
    }
}
