//! Anthropic Provider Implementation
//!
//! Implementation

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::GlobalPoolManager;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse},
};

use super::client::AnthropicClient;
use super::config::AnthropicConfig;
use super::models::{ModelFeature, get_anthropic_registry};
use super::streaming::AnthropicStream;

/// Anthropic Provider - unified implementation
#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    client: AnthropicClient,
    supported_models: Vec<ModelInfo>,
}

impl AnthropicProvider {
    /// Create
    pub fn new(config: AnthropicConfig) -> Result<Self, ProviderError> {
        // Create client
        let client = AnthropicClient::new(config.clone())?;

        // Get pool manager
        let _pool_manager = Arc::new(GlobalPoolManager::new()?);

        // Get supported models
        let registry = get_anthropic_registry();
        let supported_models = registry
            .list_models()
            .into_iter()
            .map(|spec| spec.model_info.clone())
            .collect();

        Ok(Self {
            client,
            supported_models,
        })
    }

    /// Validate request
    fn validate_request(&self, request: &ChatRequest) -> Result<(), ProviderError> {
        let registry = get_anthropic_registry();

        let model_spec = registry.get_model_spec(&request.model).ok_or_else(|| {
            ProviderError::invalid_request(
                "anthropic",
                format!("Unsupported model: {}", request.model),
            )
        })?;

        // Common validation: empty messages + max_tokens
        crate::core::providers::base_provider::validate_chat_request_common(
            "anthropic",
            request,
            model_spec.limits.max_output_tokens,
        )?;

        // Check multimodal content
        let has_multimodal_content = request.messages.iter().any(|msg| {
            if let Some(crate::core::types::message::MessageContent::Parts(parts)) = &msg.content {
                parts.iter().any(|part| {
                    !matches!(part, crate::core::types::content::ContentPart::Text { .. })
                })
            } else {
                false
            }
        });

        if has_multimodal_content
            && !model_spec
                .features
                .contains(&ModelFeature::MultimodalSupport)
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!(
                    "Model {} does not support multimodal content",
                    request.model
                ),
            ));
        }

        // Check tool calling support
        if request.tools.is_some() && !model_spec.features.contains(&ModelFeature::ToolCalling) {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!("Model {} does not support tool calling", request.model),
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    type Config = AnthropicConfig;
    type Error = ProviderError;
    type ErrorMapper = super::error::AnthropicErrorMapper;

    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ToolCalling,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "temperature",
            "max_tokens",
            "top_p",
            "top_k",
            "tools",
            "tool_choice",
            "stream",
            "stop",
        ]
    }

    async fn map_openai_params(
        &self,
        mut params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // Anthropic uses max_tokens instead of max_tokens_to_sample
        if let Some(max_tokens) = params.remove("max_tokens") {
            params.insert("max_tokens".to_string(), max_tokens);
        }

        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        self.validate_request(&request)?;

        // Request
        let mut anthropic_request = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
        });

        // Add optional parameters
        if let Some(max_tokens) = request.max_tokens {
            anthropic_request["max_tokens"] = Value::Number(max_tokens.into());
        }

        if let Some(temperature) = request.temperature {
            anthropic_request["temperature"] = Value::Number(
                serde_json::Number::from_f64(temperature.into())
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            );
        }

        if let Some(top_p) = request.top_p {
            anthropic_request["top_p"] = Value::Number(
                serde_json::Number::from_f64(top_p.into())
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            );
        }

        if request.stream {
            anthropic_request["stream"] = Value::Bool(request.stream);
        }

        if let Some(tools) = request.tools {
            anthropic_request["tools"] = serde_json::to_value(tools)?;
        }

        Ok(anthropic_request)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response_text = String::from_utf8_lossy(raw_response);
        let anthropic_response: Value = serde_json::from_str(&response_text)?;

        // Response
        let response = serde_json::from_value(anthropic_response)?;
        Ok(response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        super::error::AnthropicErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        self.validate_request(&request)?;
        let response = self.client.chat(request.clone()).await?;
        Ok(response)
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        self.validate_request(&request)?;

        let registry = get_anthropic_registry();
        let model_spec = registry.get_model_spec(&request.model).ok_or_else(|| {
            ProviderError::not_supported("anthropic", format!("Unknown model: {}", request.model))
        })?;

        if !model_spec
            .features
            .contains(&ModelFeature::StreamingSupport)
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!("Model {} does not support streaming", request.model),
            ));
        }

        let response = self.client.chat_stream(request.clone()).await?;
        let stream = AnthropicStream::from_response(response, request.model);

        Ok(Box::pin(stream))
    }

    async fn health_check(&self) -> HealthStatus {
        let test_request = ChatRequest {
            model: "claude-3-haiku-20240307".to_string(),
            messages: vec![crate::core::types::chat::ChatMessage {
                role: crate::core::types::message::MessageRole::User,
                content: Some(crate::core::types::message::MessageContent::Text(
                    "ping".to_string(),
                )),
                ..Default::default()
            }],
            max_tokens: Some(1),
            ..Default::default()
        };

        match self.client.chat(test_request).await {
            Ok(_) => HealthStatus::Healthy,
            Err(ProviderError::Authentication { .. }) => HealthStatus::Unhealthy,
            Err(ProviderError::Network { .. }) => HealthStatus::Degraded,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        Ok(
            super::models::CostCalculator::calculate_cost(model, input_tokens, output_tokens)
                .unwrap_or(0.0),
        )
    }
}

/// Provider builder
pub struct AnthropicProviderBuilder {
    config: Option<AnthropicConfig>,
}

impl AnthropicProviderBuilder {
    /// Create
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Set configuration
    pub fn with_config(mut self, config: AnthropicConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        let api_key = api_key.into();
        if let Some(ref mut config) = self.config {
            config.api_key = Some(api_key);
        } else {
            self.config = Some(AnthropicConfig::new(api_key));
        }
        self
    }

    /// Set base URL
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        if let Some(ref mut config) = self.config {
            config.base_url = base_url.into();
        }
        self
    }

    /// Build provider
    pub fn build(self) -> Result<AnthropicProvider, ProviderError> {
        let config = self.config.ok_or_else(|| {
            ProviderError::configuration("anthropic", "Configuration is required")
        })?;

        AnthropicProvider::new(config)
    }
}

impl Default for AnthropicProviderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create
pub fn create_anthropic_provider(
    config: AnthropicConfig,
) -> Result<AnthropicProvider, ProviderError> {
    AnthropicProvider::new(config)
}

/// Create
pub fn create_anthropic_provider_from_env() -> Result<AnthropicProvider, ProviderError> {
    let config = AnthropicConfig::from_env()?;
    AnthropicProvider::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let config = AnthropicConfig::new_test("test-key");
        let provider = AnthropicProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_capabilities() {
        let config = AnthropicConfig::new_test("test-key");
        let provider = AnthropicProvider::new(config).unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::ToolCalling));
    }

    #[test]
    fn test_provider_builder() {
        let provider = AnthropicProviderBuilder::new()
            .with_api_key("test-key")
            .with_base_url("https://api.anthropic.com")
            .build();

        assert!(provider.is_ok());
    }

    #[test]
    fn test_model_support() {
        let config = AnthropicConfig::new_test("test-key");
        let provider = AnthropicProvider::new(config).unwrap();

        assert!(provider.supports_model("claude-3-5-sonnet-20241022"));
        assert!(provider.supports_model("claude-3-haiku-20240307"));
        assert!(!provider.supports_model("gpt-4"));
    }
}
