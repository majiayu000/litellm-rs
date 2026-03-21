//! Gemini Provider Implementation
//!
//! Implementation

use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::GlobalPoolManager;
use crate::core::providers::unified_provider::ProviderError;
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
use super::models::{ModelFeature, get_gemini_registry};
use super::streaming::GeminiStream;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

/// Gemini Provider - Unified implementation
#[derive(Debug)]
pub struct GeminiProvider {
    client: GeminiClient,
    supported_models: Vec<ModelInfo>,
}

impl GeminiProvider {
    /// Create
    pub fn new(config: GeminiConfig) -> Result<Self, ProviderError> {
        // Configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("gemini", e))?;

        // Create
        let client = GeminiClient::new(config.clone())?;

        // Get
        let _pool_manager = Arc::new(GlobalPoolManager::new()?);

        // Get
        let registry = get_gemini_registry();
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

    /// Request
    fn validate_request(&self, request: &ChatRequest) -> Result<(), ProviderError> {
        let registry = get_gemini_registry();

        let model_spec = registry
            .get_model_spec(&request.model)
            .ok_or_else(|| gemini_model_error(format!("Unsupported model: {}", request.model)))?;

        // Common validation: empty messages + max_tokens
        crate::core::providers::base::validate_chat_request_common(
            "gemini",
            request,
            model_spec.limits.max_output_tokens,
        )?;

        // Check temperature range
        if let Some(temperature) = request.temperature
            && !(0.0..=2.0).contains(&temperature)
        {
            return Err(gemini_validation_error(
                "temperature must be between 0.0 and 2.0",
            ));
        }

        // Check top_p range
        if let Some(top_p) = request.top_p
            && !(0.0..=1.0).contains(&top_p)
        {
            return Err(gemini_validation_error("top_p must be between 0.0 and 1.0"));
        }

        // Check tool calling support
        if request.tools.is_some() && !model_spec.features.contains(&ModelFeature::ToolCalling) {
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
    ) -> Option<f64> {
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
            // NOTE: Vision capability not yet added to ProviderCapability enum
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn supports_model(&self, model: &str) -> bool {
        get_gemini_registry().get_model_spec(model).is_some()
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

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
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
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        let mut mapped = HashMap::new();

        for (key, value) in params {
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
        Ok(
            super::models::CostCalculator::calculate_cost(model, input_tokens, output_tokens)
                .unwrap_or(0.0),
        )
    }
}

// GeminiError is a type alias for ProviderError, so we don't need to implement traits for it
// The error mapping is handled by GeminiErrorMapper in error.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    // Helper function to create a basic valid request
    fn create_valid_request(model: &str) -> ChatRequest {
        ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            temperature: None,
            max_tokens: None,
            max_completion_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            stream_options: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
            response_format: None,
            user: None,
            seed: None,
            n: None,
            logit_bias: None,
            functions: None,
            function_call: None,
            logprobs: None,
            top_logprobs: None,
            thinking: None,
            extra_params: std::collections::HashMap::new(),
        }
    }

    // ==================== Provider Creation Tests ====================

    #[test]
    fn test_provider_creation() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_creation_with_short_key() {
        let config = GeminiConfig::new_google_ai("short-key");
        let provider = GeminiProvider::new(config);
        // Should fail validation for short API key
        assert!(provider.is_err());
    }

    // ==================== Provider Capabilities Tests ====================

    #[test]
    fn test_provider_capabilities() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        assert_eq!(provider.name(), "gemini");
        assert!(provider.supports_streaming());
        assert!(provider.supports_tools());
        assert!(provider.supports_vision());
        assert!(!provider.supports_embeddings());
        assert!(!provider.supports_image_generation());
    }

    #[test]
    fn test_capabilities_array() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let caps = provider.capabilities();
        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::ToolCalling));
    }

    // ==================== Model Support Tests ====================

    #[test]
    fn test_model_support() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        assert!(provider.supports_model("gemini-1.0-pro"));
        assert!(provider.supports_model("gemini-1.5-flash"));
        assert!(!provider.supports_model("gpt-4"));
    }

    #[test]
    fn test_model_support_gemini_1_0_pro() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        assert!(provider.supports_model("gemini-1.0-pro"));
    }

    #[test]
    fn test_model_support_unsupported() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        assert!(!provider.supports_model("claude-3"));
        assert!(!provider.supports_model("llama-2"));
        assert!(!provider.supports_model("unknown-model"));
    }

    #[test]
    fn test_models_list() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let models = provider.models();
        assert!(!models.is_empty());
    }

    // ==================== Request Validation Tests ====================

    #[test]
    fn test_request_validation_empty_messages() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let empty_request = ChatRequest {
            model: "gemini-1.0-pro".to_string(),
            messages: vec![],
            ..Default::default()
        };

        assert!(provider.validate_request(&empty_request).is_err());
    }

    #[test]
    fn test_request_validation_invalid_temperature_high() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut request = create_valid_request("gemini-1.0-pro");
        request.temperature = Some(3.0); // Out of range

        assert!(provider.validate_request(&request).is_err());
    }

    #[test]
    fn test_request_validation_invalid_temperature_negative() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut request = create_valid_request("gemini-1.0-pro");
        request.temperature = Some(-0.5);

        assert!(provider.validate_request(&request).is_err());
    }

    #[test]
    fn test_request_validation_valid_temperature() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut request = create_valid_request("gemini-1.0-pro");
        request.temperature = Some(1.0);

        assert!(provider.validate_request(&request).is_ok());
    }

    #[test]
    fn test_request_validation_temperature_edge_low() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut request = create_valid_request("gemini-1.0-pro");
        request.temperature = Some(0.0);

        assert!(provider.validate_request(&request).is_ok());
    }

    #[test]
    fn test_request_validation_temperature_edge_high() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut request = create_valid_request("gemini-1.0-pro");
        request.temperature = Some(2.0);

        assert!(provider.validate_request(&request).is_ok());
    }

    #[test]
    fn test_request_validation_invalid_top_p_high() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut request = create_valid_request("gemini-1.0-pro");
        request.top_p = Some(1.5);

        assert!(provider.validate_request(&request).is_err());
    }

    #[test]
    fn test_request_validation_invalid_top_p_negative() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut request = create_valid_request("gemini-1.0-pro");
        request.top_p = Some(-0.1);

        assert!(provider.validate_request(&request).is_err());
    }

    #[test]
    fn test_request_validation_valid_top_p() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut request = create_valid_request("gemini-1.0-pro");
        request.top_p = Some(0.9);

        assert!(provider.validate_request(&request).is_ok());
    }

    #[test]
    fn test_request_validation_unsupported_model() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let request = create_valid_request("unsupported-model");
        assert!(provider.validate_request(&request).is_err());
    }

    // ==================== Supported Params Tests ====================

    #[test]
    fn test_supported_openai_params() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let params = provider.get_supported_openai_params("gemini-1.0-pro");
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"top_p"));
        assert!(params.contains(&"stop"));
        assert!(params.contains(&"stream"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"tool_choice"));
    }

    #[tokio::test]
    async fn test_map_openai_params_max_tokens() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("max_tokens".to_string(), serde_json::json!(100));

        let mapped = provider
            .map_openai_params(params, "gemini-1.0-pro")
            .await
            .unwrap();
        assert!(mapped.contains_key("max_output_tokens"));
        assert!(!mapped.contains_key("max_tokens"));
    }

    #[tokio::test]
    async fn test_map_openai_params_temperature() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("temperature".to_string(), serde_json::json!(0.7));

        let mapped = provider
            .map_openai_params(params, "gemini-1.0-pro")
            .await
            .unwrap();
        assert!(mapped.contains_key("temperature"));
    }

    #[tokio::test]
    async fn test_map_openai_params_unsupported_ignored() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("frequency_penalty".to_string(), serde_json::json!(0.5));
        params.insert("presence_penalty".to_string(), serde_json::json!(0.5));

        let mapped = provider
            .map_openai_params(params, "gemini-1.0-pro")
            .await
            .unwrap();
        assert!(!mapped.contains_key("frequency_penalty"));
        assert!(!mapped.contains_key("presence_penalty"));
    }

    #[tokio::test]
    async fn test_map_openai_params_tools() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("tools".to_string(), serde_json::json!([]));
        params.insert("tool_choice".to_string(), serde_json::json!("auto"));

        let mapped = provider
            .map_openai_params(params, "gemini-1.0-pro")
            .await
            .unwrap();
        assert!(mapped.contains_key("tools"));
        assert!(mapped.contains_key("tool_choice"));
    }

    // ==================== Cost Calculation Tests ====================

    #[test]
    fn test_calculate_cost() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let cost = provider.calculate_cost("gemini-1.0-pro", 1000, 500);
        // Cost should be Some value (may be 0 for free models)
        assert!(cost.is_some());
    }

    #[test]
    fn test_calculate_cost_unknown_model() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let cost = provider.calculate_cost("unknown-model", 1000, 500);
        assert!(cost.is_none());
    }

    #[test]
    fn test_calculate_cost_zero_tokens() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let cost = provider.calculate_cost("gemini-1.0-pro", 0, 0);
        if let Some(c) = cost {
            assert!((c - 0.0).abs() < 0.0001);
        }
    }

    #[tokio::test]
    async fn test_async_calculate_cost() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let cost = LLMProvider::calculate_cost(&provider, "gemini-1.0-pro", 1000, 500).await;
        assert!(cost.is_ok());
    }

    // ==================== Unsupported Feature Tests ====================

    #[tokio::test]
    async fn test_embeddings_not_supported() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let request = EmbeddingRequest {
            model: "gemini-pro".to_string(),
            input: crate::core::types::embedding::EmbeddingInput::Text("test".to_string()),
            encoding_format: None,
            dimensions: None,
            user: None,
            task_type: None,
        };
        let context = RequestContext::default();

        let result = provider.embeddings(request, context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_image_generation_not_supported() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let request = ImageGenerationRequest {
            model: Some("gemini-pro".to_string()),
            prompt: "test".to_string(),
            n: None,
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };
        let context = RequestContext::default();

        let result = provider.image_generation(request, context).await;
        assert!(result.is_err());
    }

    // ==================== Provider Name and Identity Tests ====================

    #[test]
    fn test_provider_name() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        assert_eq!(provider.name(), "gemini");
    }

    #[test]
    fn test_error_mapper() {
        let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
        let provider = GeminiProvider::new(config).unwrap();

        let _mapper = provider.get_error_mapper();
        // If it compiles, the mapper exists
    }
}
