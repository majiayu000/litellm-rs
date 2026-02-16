//! iFlytek Spark Provider Implementation
//!
//! Implementation of LLMProvider for Spark with WebSocket support

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use crate::core::providers::base_provider::{BaseHttpClient, BaseProviderConfig};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    error_mapper::trait_def::ErrorMapper, provider::ProviderConfig,
    provider::llm_provider::trait_definition::LLMProvider,
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

use super::config::SparkConfig;
use super::model_info::{ModelFeature, get_spark_registry};

/// iFlytek Spark provider
#[derive(Debug, Clone)]
pub struct SparkProvider {
    config: SparkConfig,
    supported_models: Vec<ModelInfo>,
}

impl SparkProvider {
    /// Create new Spark provider
    pub fn new(config: SparkConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("spark", e))?;

        let base_config = BaseProviderConfig {
            api_key: config.api_key.clone(),
            api_base: Some(config.api_base.clone()),
            timeout: Some(config.request_timeout),
            max_retries: Some(config.max_retries),
            headers: None,
            organization: None,
            api_version: None,
        };

        let _base_client = BaseHttpClient::new(base_config)
            .map_err(|e| ProviderError::configuration("spark", e.to_string()))?;

        // Get supported models from registry
        let registry = get_spark_registry();
        let supported_models = registry
            .list_models()
            .into_iter()
            .map(|spec| spec.model_info.clone())
            .collect();

        Ok(Self {
            config,
            supported_models,
        })
    }

    /// Validate request
    fn validate_request(&self, request: &ChatRequest) -> Result<(), ProviderError> {
        let registry = get_spark_registry();

        let model_spec = registry.get_model_spec(&request.model).ok_or_else(|| {
            ProviderError::invalid_request("spark", format!("Unsupported model: {}", request.model))
        })?;

        // Common validation: empty messages + max_tokens
        crate::core::providers::base_provider::validate_chat_request_common(
            "spark",
            request,
            model_spec.limits.max_output_tokens,
        )?;

        // Check function calling support
        if request.tools.is_some() && !model_spec.features.contains(&ModelFeature::FunctionCalling)
        {
            return Err(ProviderError::not_supported(
                "spark",
                format!("Model {} does not support function calling", request.model),
            ));
        }

        Ok(())
    }
}

/// Spark error mapper
#[derive(Debug)]
pub struct SparkErrorMapper;

impl ErrorMapper<ProviderError> for SparkErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        match status_code {
            401 => ProviderError::authentication(
                "spark",
                format!("Invalid credentials: {}", response_body),
            ),
            403 => ProviderError::authentication(
                "spark",
                format!("Permission denied: {}", response_body),
            ),
            404 => ProviderError::model_not_found(
                "spark",
                format!("Model not found: {}", response_body),
            ),
            429 => ProviderError::rate_limit("spark", None),
            500..=599 => ProviderError::api_error(
                "spark",
                status_code,
                format!("Server error: {}", response_body),
            ),
            _ => ProviderError::api_error(
                "spark",
                status_code,
                format!("HTTP {}: {}", status_code, response_body),
            ),
        }
    }
}

#[async_trait]
impl LLMProvider for SparkProvider {
    type Config = SparkConfig;
    type Error = ProviderError;
    type ErrorMapper = SparkErrorMapper;

    fn name(&self) -> &'static str {
        "spark"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        static CAPABILITIES: &[ProviderCapability] = &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
        ];
        CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["temperature", "max_tokens", "top_k", "stream"]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // Spark API accepts most OpenAI params directly
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        self.validate_request(&request)?;

        use serde_json::json;

        let mut body = json!({
            "model": request.model,
            "messages": request.messages,
        });

        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
        }

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }

        if request.stream {
            body["stream"] = json!(true);
        }

        if let Some(tools) = request.tools {
            body["functions"] = json!(tools);
        }

        Ok(body)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response_text = String::from_utf8_lossy(raw_response);
        let spark_response: Value = serde_json::from_str(&response_text)?;

        // Transform Spark response to ChatResponse
        let response = serde_json::from_value(spark_response)?;
        Ok(response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        SparkErrorMapper
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        Ok(
            super::model_info::CostCalculator::calculate_cost(model, input_tokens, output_tokens)
                .unwrap_or(0.0),
        )
    }

    fn supports_model(&self, model: &str) -> bool {
        model.contains("spark") || model.contains("iflytek")
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.app_id.is_some()
            && self.config.api_key.is_some()
            && self.config.api_secret.is_some()
        {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        self.validate_request(&request)?;

        // Note: This is a placeholder. Real implementation would use WebSocket
        // connection with HMAC authentication for streaming/non-streaming requests
        Err(ProviderError::not_implemented(
            "spark",
            "Chat completion requires WebSocket implementation with HMAC auth",
        ))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        self.validate_request(&request)?;

        let registry = get_spark_registry();
        let model_spec = registry.get_model_spec(&request.model).ok_or_else(|| {
            ProviderError::not_supported("spark", format!("Unknown model: {}", request.model))
        })?;

        if !model_spec
            .features
            .contains(&ModelFeature::StreamingSupport)
        {
            return Err(ProviderError::not_supported(
                "spark",
                format!("Model {} does not support streaming", request.model),
            ));
        }

        // Note: This is a placeholder. Real implementation would use WebSocket
        Err(ProviderError::not_implemented(
            "spark",
            "Streaming requires WebSocket implementation with HMAC auth",
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(ProviderError::not_supported("spark", "Embeddings"))
    }

    async fn image_generation(
        &self,
        _request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        Err(ProviderError::not_supported("spark", "Image generation"))
    }
}

/// Provider builder
pub struct SparkProviderBuilder {
    config: Option<SparkConfig>,
}

impl SparkProviderBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Set configuration
    pub fn with_config(mut self, config: SparkConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set app ID
    pub fn with_app_id(mut self, app_id: impl Into<String>) -> Self {
        if let Some(ref mut config) = self.config {
            config.app_id = Some(app_id.into());
        } else {
            self.config = Some(SparkConfig {
                app_id: Some(app_id.into()),
                ..SparkConfig::default()
            });
        }
        self
    }

    /// Set API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        if let Some(ref mut config) = self.config {
            config.api_key = Some(api_key.into());
        } else {
            self.config = Some(SparkConfig {
                api_key: Some(api_key.into()),
                ..SparkConfig::default()
            });
        }
        self
    }

    /// Set API secret
    pub fn with_api_secret(mut self, api_secret: impl Into<String>) -> Self {
        if let Some(ref mut config) = self.config {
            config.api_secret = Some(api_secret.into());
        } else {
            self.config = Some(SparkConfig {
                api_secret: Some(api_secret.into()),
                ..SparkConfig::default()
            });
        }
        self
    }

    /// Build provider
    pub fn build(self) -> Result<SparkProvider, ProviderError> {
        let config = self
            .config
            .ok_or_else(|| ProviderError::configuration("spark", "Configuration is required"))?;

        SparkProvider::new(config)
    }
}

impl Default for SparkProviderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let config = SparkConfig::new("test-app-id", "test-api-key", "test-api-secret");
        let provider = SparkProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_capabilities() {
        let config = SparkConfig::new("test-app-id", "test-api-key", "test-api-secret");
        let provider = SparkProvider::new(config).unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    }

    #[test]
    fn test_provider_builder() {
        let provider = SparkProviderBuilder::new()
            .with_app_id("test-app-id")
            .with_api_key("test-api-key")
            .with_api_secret("test-api-secret")
            .build();

        assert!(provider.is_ok());
    }

    #[test]
    fn test_model_support() {
        let config = SparkConfig::new("test-app-id", "test-api-key", "test-api-secret");
        let provider = SparkProvider::new(config).unwrap();

        assert!(provider.supports_model("spark-desk-v3.5"));
        assert!(provider.supports_model("spark-desk-v3"));
        assert!(!provider.supports_model("gpt-4"));
    }

    #[test]
    fn test_supported_models_list() {
        let config = SparkConfig::new("test-app-id", "test-api-key", "test-api-secret");
        let provider = SparkProvider::new(config).unwrap();
        let models = provider.models();

        assert_eq!(models.len(), 4);
        assert!(models.iter().any(|m| m.id == "spark-desk-v3.5"));
        assert!(models.iter().any(|m| m.id == "spark-desk-v3"));
        assert!(models.iter().any(|m| m.id == "spark-desk-v2"));
        assert!(models.iter().any(|m| m.id == "spark-desk-v1.5"));
    }
}
