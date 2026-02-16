//! Replicate Provider Implementation
//!
//! Main provider implementation using the unified base infrastructure

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

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
    responses::{ChatChunk, ChatResponse, ImageGenerationResponse},
};

use super::{
    ReplicateClient, ReplicateConfig, ReplicateErrorMapper,
    models::ReplicateModelType,
    prediction::{CreatePredictionRequest, PredictionResponse, PredictionStatus},
};

/// Replicate provider implementation
#[derive(Debug, Clone)]
pub struct ReplicateProvider {
    config: ReplicateConfig,
    pool_manager: Arc<GlobalPoolManager>,
    supported_models: Vec<ModelInfo>,
}

impl ReplicateProvider {
    /// Create a new Replicate provider
    pub fn new(config: ReplicateConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("replicate", e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration("replicate", e.to_string()))?,
        );

        let supported_models = ReplicateClient::supported_models();

        Ok(Self {
            config,
            pool_manager,
            supported_models,
        })
    }

    /// Create provider with API token
    pub async fn with_api_token(api_token: impl Into<String>) -> Result<Self, ProviderError> {
        let config = ReplicateConfig::new(api_token);
        Self::new(config)
    }

    /// Create provider from environment
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = ReplicateConfig::from_env();
        Self::new(config)
    }

    /// Generate headers for Replicate API requests
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(2);

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Token {}", api_key)));
        }

        headers.push(header("Content-Type", "application/json".to_string()));

        headers
    }

    /// Create a prediction and wait for completion
    async fn create_prediction_and_wait(
        &self,
        model: &str,
        input: Value,
        stream: bool,
    ) -> Result<PredictionResponse, ProviderError> {
        // Create prediction request
        let version_hash = ReplicateConfig::extract_version_hash(model);
        let prediction_request =
            ReplicateClient::create_prediction_request(input, version_hash, stream);

        // Submit prediction
        let prediction_url = self.config.get_prediction_url(model);
        let prediction = self
            .submit_prediction(&prediction_url, &prediction_request)
            .await?;

        // Get polling URL
        let polling_url = prediction
            .get_prediction_url()
            .ok_or_else(|| {
                ProviderError::replicate_response_parsing("No polling URL in prediction response")
            })?
            .to_string();

        // Poll until completion
        self.poll_prediction(&polling_url).await
    }

    /// Submit a prediction request
    async fn submit_prediction(
        &self,
        url: &str,
        request: &CreatePredictionRequest,
    ) -> Result<PredictionResponse, ProviderError> {
        let headers = self.get_request_headers();
        let body = serde_json::to_value(request)
            .map_err(|e| ProviderError::serialization("replicate", e.to_string()))?;

        let response = self
            .pool_manager
            .execute_request(url, HttpMethod::POST, headers, Some(body))
            .await?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("replicate", e.to_string()))?;

        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&response_bytes);
            return Err(ProviderError::replicate_api_error(
                status.as_u16(),
                error_text.to_string(),
            ));
        }

        serde_json::from_slice(&response_bytes)
            .map_err(|e| ProviderError::replicate_response_parsing(e.to_string()))
    }

    /// Poll a prediction until completion
    async fn poll_prediction(&self, url: &str) -> Result<PredictionResponse, ProviderError> {
        let headers = self.get_request_headers();
        let polling_delay = std::time::Duration::from_secs(self.config.polling_delay_seconds);

        for _ in 0..self.config.polling_retries {
            tokio::time::sleep(polling_delay).await;

            let response = self
                .pool_manager
                .execute_request(url, HttpMethod::GET, headers.clone(), None)
                .await?;

            let status = response.status();
            let response_bytes = response
                .bytes()
                .await
                .map_err(|e| ProviderError::network("replicate", e.to_string()))?;

            if !status.is_success() {
                // Temporary failure, continue polling
                continue;
            }

            let prediction: PredictionResponse = serde_json::from_slice(&response_bytes)
                .map_err(|e| ProviderError::replicate_response_parsing(e.to_string()))?;

            match prediction.status {
                PredictionStatus::Succeeded => return Ok(prediction),
                PredictionStatus::Failed => {
                    let error = prediction
                        .error
                        .clone()
                        .unwrap_or_else(|| "Prediction failed".to_string());
                    return Err(ProviderError::replicate_prediction_failed(error));
                }
                PredictionStatus::Canceled => {
                    return Err(ProviderError::replicate_prediction_canceled(
                        "Prediction was canceled",
                    ));
                }
                _ => {
                    // Still processing, continue polling
                }
            }
        }

        Err(ProviderError::replicate_prediction_timeout(
            "Maximum retries exceeded waiting for prediction",
        ))
    }

    /// Execute image generation
    async fn execute_image_generation(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        let model = request.model.as_deref().unwrap_or("stability-ai/sdxl");

        let input = ReplicateClient::transform_image_request(&request, model);
        let prediction = self.create_prediction_and_wait(model, input, false).await?;

        ReplicateClient::transform_prediction_to_image_response(&prediction)
    }
}

#[async_trait]
impl LLMProvider for ReplicateProvider {
    type Config = ReplicateConfig;
    type Error = ProviderError;
    type ErrorMapper = ReplicateErrorMapper;

    fn name(&self) -> &'static str {
        "replicate"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ImageGeneration,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        ReplicateClient::supported_openai_params()
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // Map OpenAI params to Replicate format
        let mut mapped = HashMap::new();

        for (key, value) in params {
            let mapped_key = match key.as_str() {
                "max_tokens" => "max_new_tokens".to_string(),
                "stop" => "stop_sequences".to_string(),
                _ => key,
            };
            mapped.insert(mapped_key, value);
        }

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        Ok(ReplicateClient::transform_chat_request(&request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let prediction: PredictionResponse = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::replicate_response_parsing(e.to_string()))?;

        ReplicateClient::transform_prediction_to_chat_response(&prediction, model)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        ReplicateErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        let model = &request.model;

        // Check if this is an image model
        let model_type = ReplicateClient::get_model_type(model);
        if model_type == ReplicateModelType::ImageGeneration {
            return Err(ProviderError::invalid_request(
                "replicate",
                "Cannot use image model for chat completion",
            ));
        }

        let input = ReplicateClient::transform_chat_request(&request);
        let prediction = self.create_prediction_and_wait(model, input, false).await?;

        ReplicateClient::transform_prediction_to_chat_response(&prediction, model)
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        let model = &request.model;

        // Check if this is an image model
        let model_type = ReplicateClient::get_model_type(model);
        if model_type == ReplicateModelType::ImageGeneration {
            return Err(ProviderError::invalid_request(
                "replicate",
                "Cannot use image model for chat completion",
            ));
        }

        let input = ReplicateClient::transform_chat_request(&request);
        let version_hash = ReplicateConfig::extract_version_hash(model);
        let prediction_request =
            ReplicateClient::create_prediction_request(input, version_hash, true);

        // Submit prediction
        let prediction_url = self.config.get_prediction_url(model);
        let prediction = self
            .submit_prediction(&prediction_url, &prediction_request)
            .await?;

        // Get stream URL if available
        if let Some(stream_url) = prediction.get_stream_url() {
            // Use SSE streaming
            let api_key =
                self.config.base.api_key.as_ref().ok_or_else(|| {
                    ProviderError::authentication("replicate", "API token required")
                })?;

            let client = reqwest::Client::new();
            let response = client
                .get(stream_url)
                .header("Authorization", format!("Token {}", api_key))
                .header("Accept", "text/event-stream")
                .send()
                .await
                .map_err(|e| ProviderError::network("replicate", e.to_string()))?;

            if !response.status().is_success() {
                let status_code = response.status().as_u16();
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                return Err(ProviderError::replicate_api_error(status_code, error_text));
            }

            let stream = response.bytes_stream();
            Ok(Box::pin(super::streaming::create_replicate_stream(stream)))
        } else {
            // Fallback to polling and emit as single chunk
            let polling_url = prediction
                .get_prediction_url()
                .ok_or_else(|| {
                    ProviderError::replicate_response_parsing(
                        "No polling URL in prediction response",
                    )
                })?
                .to_string();

            let final_prediction = self.poll_prediction(&polling_url).await?;
            let response =
                ReplicateClient::transform_prediction_to_chat_response(&final_prediction, model)?;

            // Convert to a single chunk stream
            let content = response.choices.first().and_then(|c| {
                c.message.content.as_ref().and_then(|mc| match mc {
                    crate::core::types::message::MessageContent::Text(s) => Some(s.clone()),
                    _ => None,
                })
            });

            let chunk = ChatChunk {
                id: response.id.clone(),
                object: "chat.completion.chunk".to_string(),
                created: response.created,
                model: response.model.clone(),
                system_fingerprint: None,
                choices: vec![crate::core::types::responses::ChatStreamChoice {
                    index: 0,
                    delta: crate::core::types::responses::ChatDelta {
                        role: Some(crate::core::types::message::MessageRole::Assistant),
                        content,
                        thinking: None,
                        tool_calls: None,
                        function_call: None,
                    },
                    logprobs: None,
                    finish_reason: Some(crate::core::types::responses::FinishReason::Stop),
                }],
                usage: None,
            };

            Ok(Box::pin(futures::stream::once(async move { Ok(chunk) })))
        }
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        self.execute_image_generation(request, context).await
    }

    async fn health_check(&self) -> HealthStatus {
        // Check if we can list models
        let url = format!("{}/models", self.config.get_api_base());
        let headers = self.get_request_headers();

        match self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None)
            .await
        {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            _ => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        // Replicate pricing is per-second of compute time, not per token
        // We approximate based on model type and token counts
        if let Some(spec) = super::models::get_replicate_registry().get_model_spec(model) {
            let input_cost = spec.model_info.input_cost_per_1k_tokens.unwrap_or(0.0)
                * (input_tokens as f64 / 1000.0);
            let output_cost = spec.model_info.output_cost_per_1k_tokens.unwrap_or(0.0)
                * (output_tokens as f64 / 1000.0);
            Ok(input_cost + output_cost)
        } else {
            // Default pricing estimate
            Ok((input_tokens + output_tokens) as f64 * 0.0001)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation_without_api_key() {
        let config = ReplicateConfig::default();
        let result = ReplicateProvider::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_creation_with_api_key() {
        let config = ReplicateConfig::new("test-token");
        let result = ReplicateProvider::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = ReplicateConfig::new("test-token");
        let provider = ReplicateProvider::new(config).unwrap();
        assert_eq!(provider.name(), "replicate");
    }

    #[test]
    fn test_provider_capabilities() {
        let config = ReplicateConfig::new("test-token");
        let provider = ReplicateProvider::new(config).unwrap();
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ImageGeneration));
    }

    #[test]
    fn test_provider_models() {
        let config = ReplicateConfig::new("test-token");
        let provider = ReplicateProvider::new(config).unwrap();
        let models = provider.models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id.contains("llama")));
        assert!(models.iter().any(|m| m.id.contains("sdxl")));
    }

    #[test]
    fn test_supported_openai_params() {
        let config = ReplicateConfig::new("test-token");
        let provider = ReplicateProvider::new(config).unwrap();
        let params = provider.get_supported_openai_params("any-model");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
    }

    #[tokio::test]
    async fn test_map_openai_params() {
        let config = ReplicateConfig::new("test-token");
        let provider = ReplicateProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("max_tokens".to_string(), serde_json::json!(100));
        params.insert("temperature".to_string(), serde_json::json!(0.7));

        let mapped = provider.map_openai_params(params, "model").await.unwrap();

        assert!(mapped.contains_key("max_new_tokens"));
        assert!(mapped.contains_key("temperature"));
        assert!(!mapped.contains_key("max_tokens"));
    }

    #[test]
    fn test_get_request_headers() {
        let config = ReplicateConfig::new("test-token");
        let provider = ReplicateProvider::new(config).unwrap();
        let headers = provider.get_request_headers();

        assert!(headers.iter().any(|h| h.0 == "Authorization"));
        assert!(headers.iter().any(|h| h.0 == "Content-Type"));
    }

    #[test]
    fn test_from_env_missing_token() {
        // Clear any existing env var
        // SAFETY: Tests are single-threaded and this is just for testing
        unsafe {
            std::env::remove_var("REPLICATE_API_TOKEN");
        }

        let result = ReplicateProvider::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn test_error_mapper() {
        let config = ReplicateConfig::new("test-token");
        let provider = ReplicateProvider::new(config).unwrap();
        let _mapper = provider.get_error_mapper();
        // Just verify it compiles
    }

    #[tokio::test]
    async fn test_transform_request() {
        use crate::core::types::{
            chat::ChatMessage, message::MessageContent, message::MessageRole,
        };

        let config = ReplicateConfig::new("test-token");
        let provider = ReplicateProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "meta/llama-2-70b-chat".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stream: false,
            tools: None,
            tool_choice: None,
            user: None,
            response_format: None,
            seed: None,
            max_completion_tokens: None,
            stop: None,
            parallel_tool_calls: None,
            n: None,
            logit_bias: None,
            functions: None,
            function_call: None,
            logprobs: None,
            top_logprobs: None,
            thinking: None,
            extra_params: HashMap::new(),
        };

        let context = RequestContext::default();
        let result = provider.transform_request(request, context).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("prompt").is_some());
    }

    #[tokio::test]
    async fn test_calculate_cost() {
        let config = ReplicateConfig::new("test-token");
        let provider = ReplicateProvider::new(config).unwrap();

        let cost = provider
            .calculate_cost("meta/llama-2-70b-chat", 100, 50)
            .await
            .unwrap();

        assert!(cost >= 0.0);
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let config = ReplicateConfig::new("test-token");
        let provider = ReplicateProvider::new(config).unwrap();

        let cost = provider
            .calculate_cost("unknown/model", 100, 50)
            .await
            .unwrap();

        // Should return a default estimate
        assert!(cost >= 0.0);
    }
}
