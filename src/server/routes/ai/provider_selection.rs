//! Provider selection helpers for AI routes

use crate::core::router::UnifiedRouter;
use crate::core::types::model::ProviderCapability;
use crate::utils::error::gateway_error::GatewayError;

pub fn select_provider_for_model(
    router: &UnifiedRouter,
    model: &str,
    capability: ProviderCapability,
) -> Result<String, GatewayError> {
    if model.trim().is_empty() {
        return Err(GatewayError::validation("Model is required"));
    }

    let deployments = router.get_deployments_for_model(model);
    let supports_capability = deployments.iter().any(|deployment_id| {
        router
            .get_deployment(deployment_id)
            .map(|deployment| deployment.supports_capability(&capability))
            .unwrap_or(false)
    });

    if !supports_capability {
        return Err(GatewayError::validation(format!(
            "Model '{}' does not support {:?}",
            model, capability
        )));
    }

    let deployment = router
        .select_capability_deployment(model, &capability)
        .ok_or_else(|| {
            GatewayError::service_unavailable(format!(
                "No available deployment for model '{}' with capability {:?}",
                model, capability
            ))
        })?;

    Ok(deployment.model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::Provider;
    use crate::core::providers::openai::OpenAIProvider;
    use crate::core::router::{Deployment, HealthStatus, UnifiedRouter};
    use std::sync::atomic::Ordering;

    async fn build_embeddings_router() -> UnifiedRouter {
        let router = UnifiedRouter::default();
        let provider = Provider::OpenAI(
            OpenAIProvider::with_api_key("sk-test-key")
                .await
                .expect("test provider should build"),
        );

        router.add_deployment(Deployment::new(
            "embeddings-1".to_string(),
            provider,
            "text-embedding-3-small".to_string(),
            "embedding-model".to_string(),
        ));

        router
    }

    async fn build_audio_router() -> UnifiedRouter {
        let router = UnifiedRouter::default();
        let provider = Provider::OpenAI(
            OpenAIProvider::with_api_key("sk-test-key")
                .await
                .expect("test provider should build"),
        );

        router.add_deployment(Deployment::new(
            "whisper-1".to_string(),
            provider.clone(),
            "whisper-1".to_string(),
            "whisper-1".to_string(),
        ));
        router.add_deployment(Deployment::new(
            "tts-1".to_string(),
            provider,
            "tts-1".to_string(),
            "tts-1".to_string(),
        ));

        router
    }

    #[tokio::test]
    async fn select_provider_for_model_reports_unavailable_for_unroutable_capability() {
        let router = build_embeddings_router().await;
        let deployment = router
            .get_deployment("embeddings-1")
            .expect("deployment should exist");
        deployment
            .state
            .health
            .store(HealthStatus::Unhealthy as u8, Ordering::Relaxed);
        drop(deployment);

        let err =
            select_provider_for_model(&router, "embedding-model", ProviderCapability::Embeddings)
                .expect_err("unavailable capable deployment should not look unsupported");

        assert!(matches!(err, GatewayError::Unavailable(_)));
    }

    #[tokio::test]
    async fn select_provider_for_model_reports_validation_for_unsupported_capability() {
        let router = build_embeddings_router().await;

        let err = select_provider_for_model(
            &router,
            "embedding-model",
            ProviderCapability::CodeExecution,
        )
        .expect_err("unsupported capability should be a validation failure");

        assert!(matches!(err, GatewayError::Validation(_)));
    }

    #[tokio::test]
    async fn select_provider_for_model_routes_audio_capabilities() {
        let router = build_audio_router().await;

        let transcription =
            select_provider_for_model(&router, "whisper-1", ProviderCapability::AudioTranscription)
                .expect("audio transcription should route through a capable provider");
        let translation =
            select_provider_for_model(&router, "whisper-1", ProviderCapability::AudioTranslation)
                .expect("audio translation should route through a capable provider");
        let speech = select_provider_for_model(&router, "tts-1", ProviderCapability::TextToSpeech)
            .expect("text-to-speech should route through a capable provider");

        assert_eq!(transcription, "whisper-1");
        assert_eq!(translation, "whisper-1");
        assert_eq!(speech, "tts-1");
    }
}
