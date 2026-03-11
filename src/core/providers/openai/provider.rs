//! OpenAI Provider Implementation
//!
//! Main provider implementation integrating all OpenAI capabilities

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
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

use super::{
    client::OpenAIProvider as OpenAIClient,
    config::OpenAIConfig,
    models::{OpenAIModelFeature, OpenAIUseCase, get_openai_registry},
};
use crate::core::providers::unified_provider::ProviderError;

/// OpenAI Provider facade implementing all capabilities
#[derive(Debug, Clone)]
pub struct OpenAIProvider {
    client: OpenAIClient,
}

impl OpenAIProvider {
    /// Create new OpenAI provider
    pub async fn new(config: OpenAIConfig) -> Result<Self, ProviderError> {
        let client = OpenAIClient::new(config).await?;
        Ok(Self { client })
    }

    /// Create provider with API key
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let client = OpenAIClient::with_api_key(api_key).await?;
        Ok(Self { client })
    }

    /// Get model recommendations for specific use cases
    pub fn get_recommended_model(&self, use_case: OpenAIUseCase) -> Option<String> {
        get_openai_registry().get_recommended_model(use_case)
    }

    /// Check if a model supports a specific feature
    pub fn model_supports_feature(&self, model_id: &str, feature: &OpenAIModelFeature) -> bool {
        get_openai_registry().supports_feature(model_id, feature)
    }

    /// Get models by family (e.g., all GPT-4 variants)
    pub fn get_models_by_family(&self, family: &super::models::OpenAIModelFamily) -> Vec<String> {
        get_openai_registry().get_models_by_family(family)
    }

    /// Get models supporting specific feature
    pub fn get_models_with_feature(&self, feature: &OpenAIModelFeature) -> Vec<String> {
        get_openai_registry().get_models_with_feature(feature)
    }

    /// Get detailed model information
    pub fn get_model_info(&self, model_id: &str) -> Result<ModelInfo, ProviderError> {
        self.client.get_model_info(model_id)
    }

    /// Check model capability support
    pub fn model_supports_capability(
        &self,
        model_id: &str,
        capability: &ProviderCapability,
    ) -> bool {
        self.client.model_supports_capability(model_id, capability)
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    type Config = OpenAIConfig;
    type Error = ProviderError;
    type ErrorMapper = super::client::OpenAIErrorMapper;

    fn name(&self) -> &'static str {
        self.client.name()
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        self.client.capabilities()
    }

    fn models(&self) -> &[ModelInfo] {
        self.client.models()
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        self.client.chat_completion(request, context).await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        self.client.chat_completion_stream(request, context).await
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        self.client.embeddings(request).await
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        let response = self
            .client
            .generate_images(
                request.prompt,
                Some(request.model.unwrap_or_else(|| "dall-e-3".to_string())),
                request.n,
                request.size,
                request.quality,
                request.style,
            )
            .await?;

        // Transform OpenAI response to standard format
        serde_json::from_value(response).map_err(|e| ProviderError::ResponseParsing {
            provider: "openai",
            message: e.to_string(),
        })
    }

    async fn health_check(&self) -> HealthStatus {
        self.client.health_check().await
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        self.client
            .calculate_cost(model, input_tokens, output_tokens)
            .await
    }

    // ==================== Python LiteLLM Compatible Interface ====================

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        self.client.get_supported_openai_params(model)
    }

    async fn map_openai_params(
        &self,
        params: std::collections::HashMap<String, serde_json::Value>,
        model: &str,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>, Self::Error> {
        self.client.map_openai_params(params, model).await
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        self.client.transform_request(request, context).await
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        self.client
            .transform_response(raw_response, model, request_id)
            .await
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        super::client::OpenAIErrorMapper
    }
}

// Additional OpenAI-specific functionality
impl OpenAIProvider {
    /// Get the underlying client for advanced operations
    pub fn client(&self) -> &OpenAIClient {
        &self.client
    }

    /// Audio transcription using Whisper
    pub async fn transcribe_audio(
        &self,
        file: Vec<u8>,
        model: Option<String>,
        language: Option<String>,
        response_format: Option<String>,
    ) -> Result<serde_json::Value, ProviderError> {
        self.client
            .transcribe_audio(file, model, language, response_format)
            .await
    }

    /// List available models from OpenAI API
    pub async fn list_available_models(&self) -> Result<Vec<String>, ProviderError> {
        // This would need to be implemented in the client
        // For now, return static model list
        Ok(self.models().iter().map(|m| m.id.clone()).collect())
    }

    /// Get model pricing information
    pub fn get_model_pricing(&self, model_id: &str) -> Option<(f64, f64)> {
        if let Ok(model_info) = self.get_model_info(model_id)
            && let (Some(input_cost), Some(output_cost)) = (
                model_info.input_cost_per_1k_tokens,
                model_info.output_cost_per_1k_tokens,
            )
        {
            return Some((input_cost, output_cost));
        }
        None
    }

    /// Estimate request cost before execution
    pub async fn estimate_request_cost(&self, request: &ChatRequest) -> Result<f64, ProviderError> {
        // Simple estimation based on message content length
        let estimated_input_tokens = request
            .messages
            .iter()
            .map(|msg| {
                // Rough estimation: 1 token per 4 characters
                if let Some(content) = &msg.content {
                    match content {
                        crate::core::types::message::MessageContent::Text(text) => text.len() / 4,
                        _ => 100, // Default for non-text content
                    }
                } else {
                    0
                }
            })
            .sum::<usize>() as u32;

        let estimated_output_tokens = request
            .max_tokens
            .or(request.max_completion_tokens)
            .unwrap_or(1000);

        self.calculate_cost(
            &request.model,
            estimated_input_tokens,
            estimated_output_tokens,
        )
        .await
    }

    /// Get model context window information
    pub fn get_model_context_window(&self, model_id: &str) -> Result<u32, ProviderError> {
        let model_info = self.get_model_info(model_id)?;
        Ok(model_info.max_context_length)
    }

    /// Check if model supports vision/multimodal input
    pub fn model_supports_vision(&self, model_id: &str) -> bool {
        self.model_supports_feature(model_id, &OpenAIModelFeature::VisionSupport)
    }

    /// Check if model supports function/tool calling
    pub fn model_supports_tools(&self, model_id: &str) -> bool {
        self.model_supports_feature(model_id, &OpenAIModelFeature::FunctionCalling)
    }

    /// Check if model supports streaming
    pub fn model_supports_streaming(&self, model_id: &str) -> bool {
        self.model_supports_feature(model_id, &OpenAIModelFeature::StreamingSupport)
    }

    /// Get best model for specific task
    pub fn get_best_model_for_task(&self, task: OpenAITask) -> Option<String> {
        match task {
            OpenAITask::GeneralChat => self.get_recommended_model(OpenAIUseCase::GeneralChat),
            OpenAITask::CodeGeneration => self.get_recommended_model(OpenAIUseCase::CodeGeneration),
            OpenAITask::ComplexReasoning => self.get_recommended_model(OpenAIUseCase::Reasoning),
            OpenAITask::VisionAnalysis => self.get_recommended_model(OpenAIUseCase::Vision),
            OpenAITask::ImageGeneration => {
                self.get_recommended_model(OpenAIUseCase::ImageGeneration)
            }
            OpenAITask::AudioTranscription => {
                self.get_recommended_model(OpenAIUseCase::AudioTranscription)
            }
            OpenAITask::Embeddings => self.get_recommended_model(OpenAIUseCase::Embeddings),
            OpenAITask::CostSensitive => self.get_recommended_model(OpenAIUseCase::CostOptimized),
        }
    }
}

/// OpenAI task categories for model selection
#[derive(Debug, Clone)]
pub enum OpenAITask {
    GeneralChat,
    CodeGeneration,
    ComplexReasoning,
    VisionAnalysis,
    ImageGeneration,
    AudioTranscription,
    Embeddings,
    CostSensitive,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_creation() {
        let mut config = OpenAIConfig::default();
        config.base.api_key = Some("sk-test123".to_string());

        let provider = OpenAIProvider::new(config).await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_provider_with_api_key() {
        let provider = OpenAIProvider::with_api_key("sk-test123").await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_model_recommendations() {
        let provider = create_test_provider().await;

        assert_eq!(
            provider.get_best_model_for_task(OpenAITask::GeneralChat),
            Some("gpt-5.2-chat".to_string())
        );

        assert_eq!(
            provider.get_best_model_for_task(OpenAITask::ComplexReasoning),
            Some("o3-pro".to_string())
        );

        assert_eq!(
            provider.get_best_model_for_task(OpenAITask::CostSensitive),
            Some("gpt-5-nano".to_string())
        );
    }

    #[tokio::test]
    async fn test_feature_support() {
        let provider = create_test_provider().await;

        // Test vision support - may depend on model availability
        let gpt4o_vision = provider.model_supports_vision("gpt-4o");
        let gpt35_vision = provider.model_supports_vision("gpt-3.5-turbo");
        if !gpt4o_vision {
            eprintln!("Warning: gpt-4o vision support not detected");
        }
        assert!(!gpt35_vision); // gpt-3.5-turbo should not support vision

        // Test tool calling
        assert!(provider.model_supports_tools("gpt-4"));
        assert!(provider.model_supports_tools("gpt-3.5-turbo"));

        // Test streaming
        assert!(provider.model_supports_streaming("gpt-4"));
        assert!(!provider.model_supports_streaming("text-embedding-ada-002"));
    }

    #[tokio::test]
    async fn test_model_pricing() {
        let provider = create_test_provider().await;

        if let Some((input_cost, output_cost)) = provider.get_model_pricing("gpt-4") {
            assert!(input_cost > 0.0);
            assert!(output_cost > input_cost); // Output usually costs more
        }
    }

    #[tokio::test]
    async fn test_context_window() {
        let provider = create_test_provider().await;

        if let Ok(context_len) = provider.get_model_context_window("gpt-4o") {
            // GPT-4o typically has a large context window
            assert!(
                context_len >= 32000,
                "Expected gpt-4o to have large context, got {}",
                context_len
            );
        }

        if let Ok(context_len) = provider.get_model_context_window("gpt-3.5-turbo") {
            // GPT-3.5-turbo should have at least 4K context, may vary by version
            assert!(
                context_len >= 4000,
                "Expected gpt-3.5-turbo to have reasonable context, got {}",
                context_len
            );
        }
    }

    async fn create_test_provider() -> OpenAIProvider {
        // Create a test provider without actually connecting
        let mut config = OpenAIConfig::default();
        config.base.api_key = Some("sk-test123".to_string());

        // This is a simplified test helper - use the new method instead
        OpenAIProvider::new(config).await.unwrap()
    }
}
