use super::{Provider, ProviderError};
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::context::RequestContext;
use crate::core::types::health::HealthStatus;
use crate::core::types::message::MessageContent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeHealthProbeSemantics {
    ModelSpecific,
    Unsupported,
}

impl Provider {
    pub(crate) fn native_health_probe_semantics(&self, _model: &str) -> NativeHealthProbeSemantics {
        match self {
            Provider::Anthropic(_) => NativeHealthProbeSemantics::ModelSpecific,
            #[cfg(feature = "providers-extra")]
            Provider::VertexAI(_) => NativeHealthProbeSemantics::Unsupported,
            #[cfg(feature = "providers-extended")]
            Provider::GitHubCopilot(_) => NativeHealthProbeSemantics::ModelSpecific,
            #[cfg(feature = "providers-extended")]
            Provider::Gemini(_) | Provider::FalAI(_) => NativeHealthProbeSemantics::Unsupported,
            Provider::OpenAI(_)
            | Provider::Bedrock(_)
            | Provider::Mistral(_)
            | Provider::Cloudflare(_)
            | Provider::OpenAILike(_) => NativeHealthProbeSemantics::Unsupported,
            #[cfg(feature = "providers-extra")]
            Provider::Azure(_) | Provider::AzureAI(_) => NativeHealthProbeSemantics::Unsupported,
            #[cfg(feature = "providers-extended")]
            Provider::Ollama(_)
            | Provider::Cohere(_)
            | Provider::Replicate(_)
            | Provider::Stability(_)
            | Provider::BlackForestLabs(_) => NativeHealthProbeSemantics::Unsupported,
        }
    }

    pub(crate) fn has_safe_native_health_probe(&self, model: &str) -> bool {
        !matches!(
            self.native_health_probe_semantics(model),
            NativeHealthProbeSemantics::Unsupported
        )
    }

    /// Probe the configured deployment model when the provider's native check
    /// does not otherwise provide reliable upstream evidence.
    pub(crate) async fn health_check_for_model(&self, model: &str) -> HealthStatus {
        match self.native_health_probe_semantics(model) {
            NativeHealthProbeSemantics::ModelSpecific => self.chat_health_check(model).await,
            NativeHealthProbeSemantics::Unsupported => HealthStatus::Unknown,
        }
    }

    async fn chat_health_check(&self, model: &str) -> HealthStatus {
        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                content: Some(MessageContent::Text("ping".to_string())),
                ..Default::default()
            }],
            max_tokens: Some(1),
            ..Default::default()
        };
        match self
            .chat_completion(request, RequestContext::default())
            .await
        {
            Ok(_) => HealthStatus::Healthy,
            Err(
                ProviderError::RateLimit { .. }
                | ProviderError::Network { .. }
                | ProviderError::ProviderUnavailable { .. }
                | ProviderError::Timeout { .. },
            ) => HealthStatus::Degraded,
            Err(_) => HealthStatus::Unhealthy,
        }
    }
}

#[cfg(test)]
mod native_semantics_tests {
    use super::*;
    use crate::core::providers::bedrock::{BedrockConfig, BedrockProvider};
    use crate::core::providers::openai::OpenAIProvider;
    use crate::core::providers::openai::config::test_openai_config;
    use crate::core::providers::openai_like::{OpenAILikeConfig, OpenAILikeProvider};

    #[tokio::test]
    async fn multi_capability_providers_do_not_guess_a_native_probe_operation() {
        let openai = Provider::OpenAI(
            OpenAIProvider::new(test_openai_config(
                "http://127.0.0.1:9".to_string(),
                "sk-model-probe-test",
            ))
            .await
            .expect("test OpenAI provider should be valid"),
        );
        let bedrock = Provider::Bedrock(
            BedrockProvider::new(BedrockConfig {
                aws_access_key_id: "AKIATEST123456789012".to_string(),
                aws_secret_access_key: "test_secret".to_string(),
                ..BedrockConfig::default()
            })
            .await
            .expect("test Bedrock provider should be valid"),
        );
        let openai_like = Provider::OpenAILike(
            OpenAILikeProvider::new_openai_compatible(OpenAILikeConfig::with_api_key(
                "https://api.example.com/v1".to_string(),
                "test-openai-like-key".to_string(),
            ))
            .await
            .expect("test OpenAI-like provider should be valid"),
        );

        assert_eq!(
            openai.native_health_probe_semantics("configured-openai-model"),
            NativeHealthProbeSemantics::Unsupported
        );
        assert_eq!(
            bedrock.native_health_probe_semantics("configured-bedrock-model"),
            NativeHealthProbeSemantics::Unsupported
        );
        assert_eq!(
            openai_like.native_health_probe_semantics("unavailable-model"),
            NativeHealthProbeSemantics::Unsupported
        );
        assert_eq!(
            openai
                .health_check_for_model("text-embedding-3-small")
                .await,
            HealthStatus::Unknown
        );
        assert_eq!(
            bedrock
                .health_check_for_model("amazon.titan-embed-text-v2:0")
                .await,
            HealthStatus::Unknown
        );
        assert_eq!(
            openai_like
                .health_check_for_model("unavailable-model")
                .await,
            HealthStatus::Unknown
        );
    }
}

#[cfg(all(test, feature = "providers-extra"))]
mod vertex_semantics_tests {
    use super::*;
    use crate::core::providers::vertex_ai::{
        VertexAIProvider, VertexAIProviderConfig, VertexCredentials,
    };

    #[tokio::test]
    async fn vertex_model_names_do_not_select_a_native_probe_operation() {
        let provider = Provider::VertexAI(
            VertexAIProvider::new(VertexAIProviderConfig {
                project_id: "health-probe-test".to_string(),
                credentials: VertexCredentials::AccessToken("test-token".to_string()),
                ..Default::default()
            })
            .await
            .expect("test Vertex provider should be valid"),
        );

        for model in [
            "gemini-2.5-flash",
            "claude-3-5-sonnet-v2@20241022",
            "text-embedding-005",
        ] {
            assert_eq!(
                provider.native_health_probe_semantics(model),
                NativeHealthProbeSemantics::Unsupported,
                "model names must not guess the request operation for {model}"
            );
        }
    }
}

#[cfg(all(test, feature = "providers-extended"))]
mod tests {
    use super::*;
    use crate::core::providers::fal_ai::{FalAIConfig, FalAIProvider};
    use crate::core::providers::gemini::{GeminiConfig, GeminiProvider};
    use std::sync::Arc;

    #[tokio::test]
    async fn gemini_native_probe_does_not_guess_from_the_model_name() {
        let config = GeminiConfig::new_google_ai("test-gemini-probe-key");
        let provider = Provider::Gemini(Arc::new(
            GeminiProvider::new(config).expect("test Gemini provider should be valid"),
        ));

        assert_eq!(
            provider.health_check_for_model("gemini-2.5-flash").await,
            HealthStatus::Unknown
        );
    }

    #[tokio::test]
    async fn fal_ai_key_presence_is_not_upstream_health_evidence() {
        let provider = Provider::FalAI(
            FalAIProvider::new(FalAIConfig::with_api_key("test-fal-probe-key"))
                .expect("test Fal AI provider should be valid"),
        );

        assert_eq!(
            provider.health_check_for_model("fal-ai/flux/schnell").await,
            HealthStatus::Unknown
        );
    }
}
