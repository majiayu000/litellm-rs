use super::{Provider, ProviderError};
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::context::RequestContext;
use crate::core::types::health::HealthStatus;
use crate::core::types::message::MessageContent;

impl Provider {
    /// Probe the configured deployment model when the provider's native check
    /// does not otherwise provide reliable upstream evidence.
    pub(crate) async fn health_check_for_model(&self, model: &str) -> HealthStatus {
        match self {
            Provider::Anthropic(_) => self.chat_health_check(model).await,
            #[cfg(feature = "providers-extra")]
            Provider::VertexAI(_) if model.contains("gemini") => {
                self.chat_health_check(model).await
            }
            #[cfg(feature = "providers-extra")]
            Provider::VertexAI(_) => HealthStatus::Unknown,
            #[cfg(feature = "providers-extended")]
            Provider::Gemini(_) => self.chat_health_check(model).await,
            #[cfg(feature = "providers-extended")]
            Provider::GitHubCopilot(_) => self.chat_health_check(model).await,
            #[cfg(feature = "providers-extended")]
            Provider::FalAI(_) => HealthStatus::Unknown,
            Provider::OpenAI(_)
            | Provider::Bedrock(_)
            | Provider::Mistral(_)
            | Provider::Cloudflare(_)
            | Provider::OpenAILike(_) => self.health_check().await,
            #[cfg(feature = "providers-extra")]
            Provider::Azure(_) | Provider::AzureAI(_) => self.health_check().await,
            #[cfg(feature = "providers-extended")]
            Provider::Ollama(_) | Provider::Cohere(_) | Provider::Replicate(_) => {
                self.health_check().await
            }
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

#[cfg(all(test, feature = "providers-extended"))]
mod tests {
    use super::*;
    use crate::core::net::ProviderEndpointAccess;
    use crate::core::providers::fal_ai::{FalAIConfig, FalAIProvider};
    use crate::core::providers::gemini::{GeminiConfig, GeminiProvider};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn gemini_probe_uses_configured_supported_model() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let address = listener.local_addr().expect("test address should exist");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("probe should connect");
            let mut request = [0_u8; 8192];
            let bytes_read = socket
                .read(&mut request)
                .await
                .expect("probe request should be readable");
            let body = concat!(
                r#"{"candidates":[{"content":{"parts":[{"text":"ok"}]},"finishReason":"STOP"}],"#,
                r#""usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":1,"totalTokenCount":2}}"#
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("probe response should be writable");
            String::from_utf8_lossy(&request[..bytes_read]).into_owned()
        });
        let mut config = GeminiConfig::new_google_ai("test-gemini-probe-key");
        config.base_url = format!("http://{address}");
        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        let provider = Provider::Gemini(Arc::new(
            GeminiProvider::new(config).expect("test Gemini provider should be valid"),
        ));

        assert_eq!(
            provider.health_check_for_model("gemini-2.5-flash").await,
            HealthStatus::Healthy
        );
        let request = server.await.expect("test server should stop");
        assert!(request.contains("/models/gemini-2.5-flash:generateContent"));
        assert!(!request.contains("gemini-1.0-pro"));
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
