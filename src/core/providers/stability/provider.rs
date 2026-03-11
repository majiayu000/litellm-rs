//! Stability AI Provider Implementation
//!
//! Main provider implementation for Stability AI image generation.

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::{GlobalPoolManager, get_pricing_db};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    error_mapper::trait_def::ErrorMapper, provider::ProviderConfig,
    provider::llm_provider::trait_definition::LLMProvider,
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

use super::{StabilityConfig, StabilityErrorMapper, get_stability_registry};

/// Stability AI image generation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityImageRequest {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_preset: Option<String>,
}

/// Stability AI image generation response
#[derive(Debug, Clone, Deserialize)]
pub struct StabilityImageResponse {
    pub image: Option<String>,
    pub finish_reason: Option<String>,
    pub seed: Option<u64>,
    #[serde(default)]
    pub errors: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct StabilityProvider {
    config: StabilityConfig,
    supported_models: Vec<ModelInfo>,
}

impl StabilityProvider {
    /// Create a new Stability AI provider
    pub fn new(config: StabilityConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("stability", e))?;

        let _pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration("stability", e.to_string()))?,
        );
        let supported_models = get_stability_registry().models().to_vec();

        Ok(Self {
            config,
            supported_models,
        })
    }

    /// Create provider from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = StabilityConfig::from_env();
        Self::new(config)
    }

    /// Create provider with API key
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = StabilityConfig::with_api_key(api_key);
        Self::new(config)
    }

    /// Transform OpenAI-style image request to Stability request
    fn transform_image_request(&self, request: &ImageGenerationRequest) -> StabilityImageRequest {
        let registry = get_stability_registry();

        // Map size to aspect ratio if provided
        let aspect_ratio = request
            .size
            .as_ref()
            .and_then(|size| registry.size_to_aspect_ratio(size).map(|s| s.to_string()));

        StabilityImageRequest {
            prompt: request.prompt.clone(),
            negative_prompt: None,
            aspect_ratio,
            seed: None,
            output_format: Some("png".to_string()),
            model: request.model.clone(),
            mode: None,
            strength: None,
            style_preset: None,
        }
    }

    /// Transform Stability response to OpenAI-compatible response
    fn transform_image_response(
        &self,
        response: StabilityImageResponse,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        // Check for errors
        if let Some(errors) = &response.errors
            && !errors.is_empty()
        {
            return Err(ProviderError::api_error(
                "stability",
                400,
                errors.join(", "),
            ));
        }

        // Check finish reason
        if let Some(ref reason) = response.finish_reason
            && reason == "CONTENT_FILTERED"
        {
            return Err(ProviderError::content_filtered(
                "stability",
                "Content was filtered by Stability AI safety systems",
                None,
                Some(false),
            ));
        }

        let mut data = Vec::new();

        if let Some(image_b64) = response.image {
            data.push(ImageData {
                url: None,
                b64_json: Some(image_b64),
                revised_prompt: None,
            });
        }

        Ok(ImageGenerationResponse {
            created: chrono::Utc::now().timestamp() as u64,
            data,
        })
    }

    /// Get the API endpoint for a model
    fn get_endpoint(&self, model: Option<&str>) -> String {
        let registry = get_stability_registry();
        let model_name = model.unwrap_or("sd3");
        format!(
            "{}{}",
            self.config
                .base
                .api_base
                .as_deref()
                .unwrap_or("https://api.stability.ai"),
            registry.get_endpoint(model_name)
        )
    }
}

#[async_trait]
impl LLMProvider for StabilityProvider {
    type Config = StabilityConfig;
    type Error = ProviderError;
    type ErrorMapper = StabilityErrorMapper;

    fn name(&self) -> &'static str {
        "stability"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[ProviderCapability::ImageGeneration]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["n", "size", "response_format", "model"]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        let mut mapped = HashMap::new();
        let registry = get_stability_registry();

        for (key, value) in params {
            match key.as_str() {
                "size" => {
                    if let Some(size_str) = value.as_str()
                        && let Some(ratio) = registry.size_to_aspect_ratio(size_str)
                    {
                        mapped.insert("aspect_ratio".to_string(), Value::String(ratio.to_string()));
                    }
                }
                "n" => {
                    // Store n for later (Stability returns 1 image per request)
                    mapped.insert("_n".to_string(), value);
                }
                "response_format" => {
                    // Store for response handling
                    mapped.insert("_response_format".to_string(), value);
                }
                _ => {
                    mapped.insert(key, value);
                }
            }
        }

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        // Stability AI is primarily for image generation, not chat
        Err(ProviderError::not_supported(
            "stability",
            "Chat completion is not supported by Stability AI. Use image_generation instead.",
        ))
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        Err(ProviderError::not_supported(
            "stability",
            "Chat completion is not supported by Stability AI",
        ))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        StabilityErrorMapper
    }

    async fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        Err(ProviderError::not_supported(
            "stability",
            "Chat completion is not supported by Stability AI. Use image_generation instead.",
        ))
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        Err(ProviderError::not_supported(
            "stability",
            "Streaming is not supported by Stability AI",
        ))
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        let url = self.get_endpoint(request.model.as_deref());
        let stability_request = self.transform_image_request(&request);

        // Stability AI uses multipart/form-data for image generation
        // Build form data
        let api_key = self
            .config
            .base
            .api_key
            .as_ref()
            .ok_or_else(|| ProviderError::authentication("stability", "API key is required"))?;

        let client = reqwest::Client::new();
        let mut form = reqwest::multipart::Form::new()
            .text("prompt", stability_request.prompt.clone())
            .text(
                "output_format",
                stability_request
                    .output_format
                    .unwrap_or_else(|| "png".to_string()),
            );

        if let Some(aspect_ratio) = &stability_request.aspect_ratio {
            form = form.text("aspect_ratio", aspect_ratio.clone());
        }
        if let Some(negative_prompt) = &stability_request.negative_prompt {
            form = form.text("negative_prompt", negative_prompt.clone());
        }
        if let Some(seed) = stability_request.seed {
            form = form.text("seed", seed.to_string());
        }
        if let Some(model) = &stability_request.model {
            form = form.text("model", model.clone());
        }
        if let Some(style_preset) = &stability_request.style_preset {
            form = form.text("style_preset", style_preset.clone());
        }

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Accept", "application/json")
            .multipart(form)
            .send()
            .await
            .map_err(|e| ProviderError::network("stability", e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            let mapper = self.get_error_mapper();
            return Err(mapper.map_http_error(status.as_u16(), &error_text));
        }

        let response_body: StabilityImageResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("stability", e.to_string()))?;

        self.transform_image_response(response_body)
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.base.api_key.is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        // Stability AI pricing is per image, not per token
        // Use the pricing database for estimation
        let usage = crate::core::providers::base::pricing::Usage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
            reasoning_tokens: None,
        };

        Ok(get_pricing_db().calculate(model, &usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stability_provider_name() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();
        assert_eq!(provider.name(), "stability");
    }

    #[test]
    fn test_stability_provider_capabilities() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();
        let caps = provider.capabilities();
        assert!(caps.contains(&ProviderCapability::ImageGeneration));
    }

    #[test]
    fn test_stability_provider_models() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();
        let models = provider.models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_transform_image_request() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();

        let request = ImageGenerationRequest {
            prompt: "A beautiful sunset".to_string(),
            model: Some("sd3".to_string()),
            n: Some(1),
            size: Some("1024x1024".to_string()),
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };

        let stability_request = provider.transform_image_request(&request);
        assert_eq!(stability_request.prompt, "A beautiful sunset");
        assert_eq!(stability_request.aspect_ratio, Some("1:1".to_string()));
    }

    #[test]
    fn test_transform_image_request_landscape() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();

        let request = ImageGenerationRequest {
            prompt: "A mountain landscape".to_string(),
            model: None,
            n: None,
            size: Some("1792x1024".to_string()),
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };

        let stability_request = provider.transform_image_request(&request);
        assert_eq!(stability_request.aspect_ratio, Some("16:9".to_string()));
    }

    #[test]
    fn test_transform_image_response_success() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();

        let response = StabilityImageResponse {
            image: Some("base64encodedimage".to_string()),
            finish_reason: Some("SUCCESS".to_string()),
            seed: Some(12345),
            errors: None,
        };

        let result = provider.transform_image_response(response);
        assert!(result.is_ok());

        let gen_response = result.unwrap();
        assert_eq!(gen_response.data.len(), 1);
        assert!(gen_response.data[0].b64_json.is_some());
    }

    #[test]
    fn test_transform_image_response_content_filtered() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();

        let response = StabilityImageResponse {
            image: None,
            finish_reason: Some("CONTENT_FILTERED".to_string()),
            seed: None,
            errors: None,
        };

        let result = provider.transform_image_response(response);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::ContentFiltered { .. }
        ));
    }

    #[test]
    fn test_transform_image_response_with_errors() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();

        let response = StabilityImageResponse {
            image: None,
            finish_reason: None,
            seed: None,
            errors: Some(vec!["Invalid prompt".to_string()]),
        };

        let result = provider.transform_image_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_endpoint_sd3() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();

        let endpoint = provider.get_endpoint(Some("sd3"));
        assert!(endpoint.contains("/v2beta/stable-image/generate/sd3"));
    }

    #[test]
    fn test_get_endpoint_ultra() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();

        let endpoint = provider.get_endpoint(Some("stable-image-ultra"));
        assert!(endpoint.contains("/v2beta/stable-image/generate/ultra"));
    }

    #[test]
    fn test_get_supported_openai_params() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();

        let params = provider.get_supported_openai_params("sd3");
        assert!(params.contains(&"size"));
        assert!(params.contains(&"n"));
    }

    #[tokio::test]
    async fn test_chat_completion_not_supported() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "sd3".to_string(),
            messages: vec![],
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.chat_completion(request, context).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::NotSupported { .. }
        ));
    }

    #[test]
    fn test_health_check_with_api_key() {
        let config = StabilityConfig::with_api_key("test-key");
        let provider = StabilityProvider::new(config).unwrap();

        // Create a runtime for the async test
        let rt = tokio::runtime::Runtime::new().unwrap();
        let health = rt.block_on(provider.health_check());
        assert_eq!(health, HealthStatus::Healthy);
    }
}
