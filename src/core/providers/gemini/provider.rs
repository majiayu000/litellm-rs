//! Gemini Provider Implementation
//!
//! Implementation

use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use crate::core::providers::google_tool_loop::request_requires_tool_capability;
use crate::core::providers::{GeminiNativeRequest, ProviderError};
use crate::core::traits::{
    provider::ProviderConfig, provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    image::ImageGenerationRequest,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse, ImageGenerationResponse},
};

use super::client::GeminiClient;
use super::config::GeminiConfig;
use super::error::{GeminiErrorMapper, gemini_model_error, gemini_validation_error};
use super::models::{
    GeminiModelListings, GeminiUtcClock, GoogleGeminiApiSurface, ModelFeature, get_gemini_registry,
    has_trailing_assistant_prefill, uses_fixed_sampling_contract,
};
use super::streaming::GeminiStream;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

/// Gemini Provider - Unified implementation
#[derive(Debug)]
pub struct GeminiProvider {
    client: GeminiClient,
    surface: GoogleGeminiApiSurface,
    supported_models: GeminiModelListings,
    clock: GeminiUtcClock,
}

impl GeminiProvider {
    /// Create
    pub fn new(config: GeminiConfig) -> Result<Self, ProviderError> {
        Self::new_with_clock(config, GeminiUtcClock::system())
    }

    pub(crate) fn new_with_clock(
        config: GeminiConfig,
        clock: GeminiUtcClock,
    ) -> Result<Self, ProviderError> {
        // Configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("gemini", e))?;

        // Create
        let client = GeminiClient::new(config.clone())?;

        // Get
        let registry = get_gemini_registry();
        let surface = if config.use_vertex_ai {
            GoogleGeminiApiSurface::VertexAi
        } else {
            GoogleGeminiApiSurface::DeveloperApi
        };
        let supported_models = GeminiModelListings::new(registry, surface);

        Ok(Self {
            client,
            surface,
            supported_models,
            clock,
        })
    }

    pub(crate) async fn gemini_generate_content(
        &self,
        request: GeminiNativeRequest,
    ) -> Result<reqwest::Response, ProviderError> {
        let api_key = self.client.api_key();
        let response = self.client.send_native_request(&request).await?;
        crate::core::providers::gemini_response_or_provider_error(response, api_key).await
    }

    /// Request
    fn validate_request(&self, request: &ChatRequest) -> Result<(), ProviderError> {
        let registry = get_gemini_registry();

        let model_spec = registry
            .get_model_spec(&request.model)
            .filter(|spec| self.surface.includes(spec))
            .ok_or_else(|| gemini_model_error(format!("Unsupported model: {}", request.model)))?;

        // Common validation: empty messages + max_tokens
        crate::core::providers::base::validate_chat_request_common(
            "gemini",
            request,
            model_spec.limits.max_output_tokens,
        )?;

        if uses_fixed_sampling_contract(&request.model) {
            if has_trailing_assistant_prefill(request) {
                return Err(gemini_validation_error(format!(
                    "Model {} does not accept a trailing non-empty assistant message",
                    request.model
                )));
            }
        } else {
            if let Some(temperature) = request.temperature
                && !(0.0..=2.0).contains(&temperature)
            {
                return Err(gemini_validation_error(
                    "temperature must be between 0.0 and 2.0",
                ));
            }
            if let Some(top_p) = request.top_p
                && !(0.0..=1.0).contains(&top_p)
            {
                return Err(gemini_validation_error("top_p must be between 0.0 and 1.0"));
            }
        }

        // Check tool calling support
        if request_requires_tool_capability(request)
            && !model_spec.features.contains(&ModelFeature::ToolCalling)
        {
            return Err(gemini_validation_error(format!(
                "Model {} does not support tool calling",
                request.model
            )));
        }

        Ok(())
    }

    /// Get
    pub fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        super::models::CostCalculator::calculate_cost(model, input_tokens, output_tokens)
    }
}

impl LLMProvider for GeminiProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ToolCalling,
            ProviderCapability::GeminiGenerateContent,
            // NOTE: Vision capability not yet added to ProviderCapability enum
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        self.supported_models.at(self.clock.now())
    }

    fn supports_model(&self, model: &str) -> bool {
        get_gemini_registry()
            .get_model_spec(model)
            .is_some_and(|spec| self.surface.includes(spec))
    }

    fn supports_tools(&self) -> bool {
        true // Gemini supports tool calling
    }

    fn supports_streaming(&self) -> bool {
        true // Streaming support
    }

    fn supports_image_generation(&self) -> bool {
        false // Gemini currently does not support image generation
    }

    fn supports_embeddings(&self) -> bool {
        false // NOTE: could be supported via dedicated embedding models
    }

    fn supports_vision(&self) -> bool {
        true // Gemini supports vision understanding
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        if uses_fixed_sampling_contract(model) {
            return &["max_tokens", "stop", "stream", "tools", "tool_choice"];
        }
        &[
            "temperature",
            "max_tokens",
            "top_p",
            "stop",
            "stream",
            "tools",
            "tool_choice",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        let mut mapped = HashMap::new();

        for (key, value) in params {
            if uses_fixed_sampling_contract(model)
                && matches!(key.as_str(), "temperature" | "top_p" | "top_k")
            {
                continue;
            }
            match key.as_str() {
                // Directly mapped parameters
                "temperature" | "top_p" | "stop" | "stream" => {
                    mapped.insert(key, value);
                }
                "max_tokens" => {
                    mapped.insert("max_output_tokens".to_string(), value);
                }
                // Handle tools
                "tools" | "tool_choice" => {
                    mapped.insert(key, value);
                }
                // Ignore unsupported parameters
                "frequency_penalty" | "presence_penalty" | "logit_bias" => {
                    // Gemini doesn't support these parameters, skip
                }
                // Keep other parameters as-is
                _ => {
                    mapped.insert(key, value);
                }
            }
        }

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        // Request
        self.validate_request(&request)?;

        // Use client's transformation method
        let transformed = self.client.transform_chat_request(&request)?;
        Ok(transformed)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let response_text = String::from_utf8_lossy(raw_response);
        let response_json: Value = serde_json::from_str(&response_text).map_err(|e| {
            ProviderError::serialization("gemini", format!("Failed to parse response: {}", e))
        })?;

        // Error
        if response_json.get("error").is_some() {
            return Err(GeminiErrorMapper::from_api_response(&response_json));
        }

        // Request
        let dummy_request = ChatRequest {
            model: model.to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            max_completion_tokens: None,
            top_p: None,
            n: None,
            stream: false,
            stream_options: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            logprobs: None,
            top_logprobs: None,
            user: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
            response_format: None,
            seed: None,
            functions: None,
            function_call: None,
            thinking: None,
            reasoning_effort: None,
            store: None,
            metadata: None,
            service_tier: None,
            extra_params: std::collections::HashMap::new(),
        };

        self.client
            .transform_chat_response(response_json, &dummy_request)
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(GeminiErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        // Request
        self.validate_request(&request)?;

        // Request
        self.client.chat(request).await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        // Request
        self.validate_request(&request)?;

        // Request
        let response = self.client.chat_stream(request.clone()).await?;

        // Create stream
        let stream = GeminiStream::from_response(response, request.model);

        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::NotSupported {
            provider: "gemini",
            feature: "embeddings: not yet implemented for Gemini provider".to_string(),
        })
    }

    async fn image_generation(
        &self,
        _request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        Err(ProviderError::NotSupported {
            provider: "gemini",
            feature: "image_generation: not supported by Gemini provider".to_string(),
        })
    }

    async fn health_check(&self) -> HealthStatus {
        // Health check request
        let test_request = ChatRequest {
            model: "gemini-1.0-pro".to_string(),
            messages: vec![crate::core::types::chat::ChatMessage {
                role: crate::core::types::message::MessageRole::User,
                content: Some(crate::core::types::message::MessageContent::Text(
                    "Hi".to_string(),
                )),
                ..Default::default()
            }],
            temperature: Some(0.1),
            max_tokens: Some(5),
            max_completion_tokens: None,
            top_p: None,
            n: None,
            stream: false,
            stream_options: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            logprobs: None,
            top_logprobs: None,
            user: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
            response_format: None,
            seed: None,
            functions: None,
            function_call: None,
            thinking: None,
            reasoning_effort: None,
            store: None,
            metadata: None,
            service_tier: None,
            extra_params: std::collections::HashMap::new(),
        };

        match self.client.chat(test_request).await {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => match &e {
                ProviderError::Authentication { .. } => HealthStatus::Unhealthy,
                ProviderError::RateLimit { .. } => HealthStatus::Degraded,
                ProviderError::Network { .. } => HealthStatus::Degraded,
                _ => HealthStatus::Unhealthy,
            },
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        super::calculate_gemini_cost(model, input_tokens, output_tokens)
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use crate::core::net::ProviderEndpointAccess;
    use crate::core::types::chat::ChatMessage;
    use crate::core::types::message::{MessageContent, MessageRole};
    use futures::StreamExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[test]
    fn native_transport_timeout_keeps_timeout_classification() {
        let error = crate::core::providers::gemini_transport_error(true);
        assert!(matches!(error, ProviderError::Timeout { .. }));
    }
    async fn error_provider(status: u16, headers: &str, body: &str, key: &str) -> ProviderError {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status} Error\r\n{headers}content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let bytes_read = socket.read(&mut request).await.unwrap();
            assert!(bytes_read > 0);
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let mut config = GeminiConfig::new_google_ai(key);
        config.base_url = format!("http://{address}");
        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        let provider = GeminiProvider::new(config).unwrap();
        let error = provider
            .gemini_generate_content(GeminiNativeRequest {
                api_version: "v1beta".to_string(),
                model: "gemini-test".to_string(),
                method: "generateContent",
                stream: false,
                body: serde_json::json!({}),
            })
            .await
            .unwrap_err();
        task.await.unwrap();
        error
    }

    async fn stream_provider(mut config: GeminiConfig, body: &str) -> Vec<ChatChunk> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            assert!(socket.read(&mut request).await.unwrap() > 0);
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        config.base_url = format!("http://{address}");
        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        let provider = GeminiProvider::new(config).unwrap();
        let request = ChatRequest {
            model: "gemini-2.5-flash".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("hello".to_string())),
                ..Default::default()
            }],
            stream: true,
            ..Default::default()
        };
        let stream = provider
            .chat_completion_stream(request, RequestContext::default())
            .await
            .unwrap();
        task.await.unwrap();
        stream.map(|chunk| chunk.unwrap()).collect().await
    }

    #[tokio::test]
    async fn public_provider_stream_never_exposes_invalid_usage_marker_or_zero_usage() {
        let body = concat!(
            "data: {\"candidates\":[],\"usageMetadata\":{\"promptTokenCount\":1,",
            "\"candidatesTokenCount\":2,\"totalTokenCount\":3}}\n\n",
            "data: {\"candidates\":[],\"usageMetadata\":{\"promptTokenCount\":4,",
            "\"totalTokenCount\":4}}\n\n"
        );
        for config in [
            GeminiConfig::new_google_ai("test-key-12345678901234567890"),
            GeminiConfig::new_vertex_ai("project", "location"),
        ] {
            let chunks = stream_provider(config, body).await;
            assert_eq!(chunks.len(), 1);
            assert!(chunks[0].choices.is_empty());
            assert!(chunks[0].usage.is_none());
            assert!(
                !serde_json::to_string(&chunks[0])
                    .unwrap()
                    .contains("__litellm")
            );
        }
    }

    #[tokio::test]
    async fn native_error_redacts_raw_and_form_encoded_key() {
        let key = "secret/key+value-12345678901234567890";
        let encoded: String = url::form_urlencoded::byte_serialize(key.as_bytes()).collect();
        let error = error_provider(500, "", &format!("{key} {encoded}"), key).await;
        for text in [error.to_string(), format!("{error:?}")] {
            assert!(text.contains("[REDACTED]"));
            assert!(!text.contains(key));
            assert!(!text.contains(&encoded));
        }
    }
    #[tokio::test]
    async fn native_rate_limit_prefers_header_then_body_retry_after() {
        let key = "test-key-12345678901234567890";
        let header = error_provider(429, "retry-after: 7\r\n", r#"{"retry_after":3}"#, key).await;
        let body = error_provider(429, "", r#"{"retry_after":3}"#, key).await;
        let retries = [header, body].map(|error| match error {
            ProviderError::RateLimit { retry_after, .. } => retry_after,
            _ => None,
        });
        assert_eq!(retries, [Some(7), Some(3)]);
    }
    #[tokio::test]
    async fn native_non_rate_limit_empty_body_is_api_error() {
        let error = error_provider(503, "", "", "test-key-12345678901234567890").await;
        assert!(matches!(error, ProviderError::ApiError { status: 503, .. }));
        let message = error.to_string();
        assert!(message.contains("Gemini upstream returned HTTP 503"));
    }
}

// GeminiError is a type alias for ProviderError, so we don't need to implement traits for it
// The error mapping is handled by GeminiErrorMapper in error.rs

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
