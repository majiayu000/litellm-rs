//! Fal AI Provider Implementation
//!
//! Main provider implementation for Fal AI image generation

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;

use crate::core::providers::base::{GlobalPoolManager, HeaderPair, HttpMethod, header};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    provider::ProviderConfig, provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    image::ImageGenerationRequest,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, ImageData, ImageGenerationResponse},
};

use super::models::map_openai_to_fal_params;
use super::{FalAIConfig, FalAIErrorMapper, FalAIModelRegistry};

/// Fal AI Provider for image generation
#[derive(Debug, Clone)]
pub struct FalAIProvider {
    config: FalAIConfig,
    pool_manager: Arc<GlobalPoolManager>,
    model_registry: FalAIModelRegistry,
    supported_models: Vec<ModelInfo>,
}

impl FalAIProvider {
    /// Create a new Fal AI provider with configuration
    pub fn new(config: FalAIConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("fal_ai", e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration("fal_ai", e.to_string()))?,
        );

        let model_registry = FalAIModelRegistry::new();
        let supported_models = Self::build_model_info(&model_registry);

        Ok(Self {
            config,
            pool_manager,
            model_registry,
            supported_models,
        })
    }

    /// Create provider from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = FalAIConfig::from_env();
        Self::new(config)
    }

    /// Create provider with API key
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = FalAIConfig::with_api_key(api_key);
        Self::new(config)
    }

    /// Build ModelInfo list from registry
    fn build_model_info(registry: &FalAIModelRegistry) -> Vec<ModelInfo> {
        registry
            .list_models()
            .iter()
            .map(|m| ModelInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                provider: "fal_ai".to_string(),
                max_context_length: 0,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: true,
                input_cost_per_1k_tokens: None,
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: vec![ProviderCapability::ImageGeneration],
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            })
            .collect()
    }

    /// Generate request headers for Fal AI API
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(2);

        if let Some(api_key) = self.config.get_api_key() {
            headers.push(header("Authorization", format!("Key {}", api_key)));
        }

        headers.push(header("Content-Type", "application/json".to_string()));
        headers
    }

    /// Get the endpoint URL for a model
    fn get_model_endpoint(&self, model: &str) -> String {
        let base = self.config.get_api_base().trim_end_matches('/');
        format!("{}/{}", base, model)
    }

    /// Transform Fal AI response to ImageGenerationResponse
    fn transform_image_response(
        &self,
        response_data: Value,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        let images = response_data
            .get("images")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                ProviderError::response_parsing("fal_ai", "Missing 'images' field in response")
            })?;

        let data: Vec<ImageData> = images
            .iter()
            .filter_map(|img| {
                if let Some(url) = img.get("url").and_then(|v| v.as_str()) {
                    Some(ImageData {
                        url: Some(url.to_string()),
                        b64_json: img
                            .get("b64_json")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        revised_prompt: None,
                    })
                } else {
                    img.as_str().map(|url| ImageData {
                        url: Some(url.to_string()),
                        b64_json: None,
                        revised_prompt: None,
                    })
                }
            })
            .collect();

        let created = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(ImageGenerationResponse { created, data })
    }
}

#[async_trait]
impl LLMProvider for FalAIProvider {
    type Config = FalAIConfig;
    type Error = ProviderError;
    type ErrorMapper = FalAIErrorMapper;

    fn name(&self) -> &'static str {
        "fal_ai"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[ProviderCapability::ImageGeneration]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        super::models::SUPPORTED_OPENAI_PARAMS
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        let params_value = serde_json::to_value(&params)
            .map_err(|e| ProviderError::invalid_request("fal_ai", e.to_string()))?;
        let mapped = map_openai_to_fal_params(&params_value);

        serde_json::from_value(mapped)
            .map_err(|e| ProviderError::invalid_request("fal_ai", e.to_string()))
    }

    async fn transform_request(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        // Fal AI is primarily for image generation, not chat
        Err(ProviderError::not_implemented(
            "fal_ai",
            "Chat completion not supported. Use image_generation instead.",
        ))
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        Err(ProviderError::not_implemented(
            "fal_ai",
            "Chat response transformation not supported",
        ))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        FalAIErrorMapper
    }

    async fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        Err(ProviderError::not_implemented(
            "fal_ai",
            "Chat completion not supported. Fal AI is an image generation provider.",
        ))
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        Err(ProviderError::not_implemented(
            "fal_ai",
            "Streaming not supported for image generation",
        ))
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        let model = request.model.as_deref().unwrap_or("fal-ai/flux/schnell");
        let url = self.get_model_endpoint(model);

        // Build request body
        let mut body = serde_json::json!({
            "prompt": request.prompt,
        });

        // Map optional parameters
        if let Some(n) = request.n {
            body["num_images"] = serde_json::json!(n);
        }

        if let Some(size) = &request.size {
            let image_size = super::models::ImageSize::from_openai_size(size);
            body["image_size"] = serde_json::to_value(image_size)
                .map_err(|e| ProviderError::invalid_request("fal_ai", e.to_string()))?;
        }

        if let Some(format) = &request.response_format {
            let output_format = match format.as_str() {
                "b64_json" | "url" => "jpeg",
                f => f,
            };
            body["output_format"] = serde_json::json!(output_format);
        }

        let headers = self.get_request_headers();

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("fal_ai", e.to_string()))?;

        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&response_bytes);
            return Err(ProviderError::api_error(
                "fal_ai",
                status.as_u16(),
                error_text.to_string(),
            ));
        }

        let response_data: Value = serde_json::from_slice(&response_bytes)
            .map_err(|e| ProviderError::response_parsing("fal_ai", e.to_string()))?;

        self.transform_image_response(response_data)
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.get_api_key().is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        // For image generation, cost is per image not per token
        Ok(self.model_registry.get_cost_per_image(model))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation_fails_without_api_key() {
        let config = FalAIConfig::default();
        let result = FalAIProvider::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_creation_with_api_key() {
        let config = FalAIConfig::with_api_key("test-key");
        let result = FalAIProvider::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();
        assert_eq!(provider.name(), "fal_ai");
    }

    #[test]
    fn test_provider_capabilities() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();
        let caps = provider.capabilities();
        assert!(caps.contains(&ProviderCapability::ImageGeneration));
    }

    #[test]
    fn test_provider_models() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();
        let models = provider.models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_get_model_endpoint() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();
        let endpoint = provider.get_model_endpoint("fal-ai/flux/schnell");
        assert!(endpoint.contains("fal.run"));
        assert!(endpoint.contains("fal-ai/flux/schnell"));
    }

    #[test]
    fn test_transform_image_response() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let response_data = serde_json::json!({
            "images": [
                {"url": "https://example.com/image1.png"},
                {"url": "https://example.com/image2.png"}
            ]
        });

        let result = provider.transform_image_response(response_data);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.data.len(), 2);
        assert!(response.data[0].url.is_some());
    }

    #[test]
    fn test_transform_image_response_url_strings() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let response_data = serde_json::json!({
            "images": [
                "https://example.com/image1.png",
                "https://example.com/image2.png"
            ]
        });

        let result = provider.transform_image_response(response_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transform_image_response_missing_images() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let response_data = serde_json::json!({
            "error": "Something went wrong"
        });

        let result = provider.transform_image_response(response_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_supported_openai_params() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();
        let params = provider.get_supported_openai_params("any-model");
        assert!(params.contains(&"n"));
        assert!(params.contains(&"size"));
        assert!(params.contains(&"response_format"));
    }

    #[tokio::test]
    async fn test_calculate_cost() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let cost = provider.calculate_cost("fal-ai/flux/schnell", 0, 0).await;
        assert!(cost.is_ok());
        assert!(cost.unwrap() > 0.0);
    }

    #[tokio::test]
    async fn test_health_check_with_api_key() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let status = provider.health_check().await;
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_chat_completion_not_supported() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "test".to_string(),
            messages: vec![],
            ..Default::default()
        };
        let context = RequestContext::default();

        let result = provider.chat_completion(request, context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_map_openai_params() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("n".to_string(), serde_json::json!(2));
        params.insert("size".to_string(), serde_json::json!("1024x1024"));

        let result = provider.map_openai_params(params, "model").await;
        assert!(result.is_ok());

        let mapped = result.unwrap();
        assert!(mapped.contains_key("num_images"));
        assert!(mapped.contains_key("image_size"));
    }
}
