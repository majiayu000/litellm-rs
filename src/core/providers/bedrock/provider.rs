//! Main Bedrock Provider Implementation
//!
//! Contains the BedrockProvider struct and its LLMProvider trait implementation.

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use tracing::debug;

use super::client::BedrockClient;
use super::config::BedrockConfig;
use super::error::{BedrockError, BedrockErrorMapper};
use super::model_config::get_model_config;
use super::transformation;
use super::utils::{CostCalculator, validate_region};
use crate::core::traits::provider::ProviderConfig as _;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    message::MessageContent,
    message::MessageRole,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse},
};

/// Static capabilities for Bedrock provider
pub(super) const BEDROCK_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::FunctionCalling,
    ProviderCapability::Embeddings,
];

/// AWS Bedrock provider implementation
#[derive(Debug, Clone)]
pub struct BedrockProvider {
    client: BedrockClient,
    models: Vec<ModelInfo>,
}

impl BedrockProvider {
    /// Create a new Bedrock provider instance
    pub async fn new(config: BedrockConfig) -> Result<Self, BedrockError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("bedrock", e))?;

        // Validate AWS region
        validate_region(&config.aws_region)?;

        // Create Bedrock client
        let client = BedrockClient::new(config)?;

        // Define supported models using cost calculator data
        let mut models = Vec::new();
        let available_models = CostCalculator::get_all_models();

        for model_id in available_models {
            if let Some(pricing) = CostCalculator::get_model_pricing(model_id) {
                if let Ok(model_config) = get_model_config(model_id) {
                    models.push(ModelInfo {
                        id: model_id.to_string(),
                        name: format!(
                            "{} (Bedrock)",
                            model_id.split('.').next_back().unwrap_or(model_id)
                        ),
                        provider: "bedrock".to_string(),
                        max_context_length: model_config.max_context_length,
                        max_output_length: model_config.max_output_length,
                        supports_streaming: model_config.supports_streaming,
                        supports_tools: model_config.supports_function_calling,
                        supports_multimodal: model_config.supports_multimodal,
                        input_cost_per_1k_tokens: Some(pricing.input_cost_per_1k),
                        output_cost_per_1k_tokens: Some(pricing.output_cost_per_1k),
                        currency: pricing.currency.to_string(),
                        capabilities: vec![],
                        created_at: None,
                        updated_at: None,
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        Ok(Self { client, models })
    }

    /// Generate images using Bedrock image models
    pub async fn generate_image(
        &self,
        request: &crate::core::types::image::ImageGenerationRequest,
    ) -> Result<crate::core::types::responses::ImageGenerationResponse, BedrockError> {
        super::images::execute_image_generation(&self.client, request).await
    }

    /// Access the Agents client
    pub fn agents_client(&self) -> super::agents::AgentClient<'_> {
        super::agents::AgentClient::new(&self.client)
    }

    /// Access the Knowledge Bases client
    pub fn knowledge_bases_client(&self) -> super::knowledge_bases::KnowledgeBaseClient<'_> {
        super::knowledge_bases::KnowledgeBaseClient::new(&self.client)
    }

    /// Access the Batch processing client
    pub fn batch_client(&self) -> super::batch::BatchClient<'_> {
        super::batch::BatchClient::new(&self.client)
    }

    /// Access the Guardrails client
    pub fn guardrails_client(&self) -> super::guardrails::GuardrailClient<'_> {
        super::guardrails::GuardrailClient::new(&self.client)
    }

    /// Check if model is an embedding model
    pub(super) fn is_embedding_model(&self, model: &str) -> bool {
        model.contains("embed")
    }

    /// Create a test provider for unit testing
    #[cfg(test)]
    pub(super) fn new_for_test(client: BedrockClient, models: Vec<ModelInfo>) -> Self {
        Self { client, models }
    }

    /// Convert messages to a single prompt string for models that require it
    pub(super) fn messages_to_prompt(
        &self,
        messages: &[ChatMessage],
    ) -> Result<String, BedrockError> {
        let mut prompt = String::new();

        for message in messages {
            let content = match &message.content {
                Some(MessageContent::Text(text)) => text.clone(),
                Some(MessageContent::Parts(parts)) => {
                    // Extract text from parts
                    parts
                        .iter()
                        .filter_map(|part| {
                            if let crate::core::types::content::ContentPart::Text { text } = part {
                                Some(text.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                }
                None => continue,
            };

            match message.role {
                MessageRole::System => prompt.push_str(&format!("System: {}\n\n", content)),
                MessageRole::User => prompt.push_str(&format!("Human: {}\n\n", content)),
                MessageRole::Assistant => prompt.push_str(&format!("Assistant: {}\n\n", content)),
                MessageRole::Function | MessageRole::Tool => {
                    prompt.push_str(&format!("Tool: {}\n\n", content));
                }
            }
        }

        // Add Assistant prompt at the end for completion
        prompt.push_str("Assistant:");

        Ok(prompt)
    }
}

#[async_trait]
impl LLMProvider for BedrockProvider {
    type Config = BedrockConfig;
    type Error = BedrockError;
    type ErrorMapper = BedrockErrorMapper;

    fn name(&self) -> &'static str {
        "bedrock"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        BEDROCK_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "temperature",
            "top_p",
            "max_tokens",
            "stream",
            "stop",
            "tools",
            "tool_choice",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // Bedrock has some differences from OpenAI format
        let mut mapped = HashMap::new();

        for (key, value) in params {
            match key.as_str() {
                // Map OpenAI parameters to Bedrock format
                "max_tokens" => mapped.insert("max_tokens_to_sample".to_string(), value),
                "temperature" | "top_p" | "stream" | "stop" => mapped.insert(key, value),
                // Skip unsupported parameters
                _ => None,
            };
        }

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        transformation::transform_chat_request(
            &request.model,
            &request.messages,
            request.max_tokens,
            request.temperature,
            request.top_p,
            |msgs| self.messages_to_prompt(msgs),
        )
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        transformation::transform_chat_response(raw_response, model)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Bedrock chat request: model={}", request.model);

        // Check if it's an embedding model
        if self.is_embedding_model(&request.model) {
            return Err(ProviderError::invalid_request(
                "bedrock",
                "Use embeddings endpoint for embedding models".to_string(),
            ));
        }

        // Use the chat module's routing logic
        let response_value = super::chat::route_chat_request(&self.client, &request).await?;

        // Convert the response to bytes for transform_response
        let response_bytes = serde_json::to_vec(&response_value)
            .map_err(|e| ProviderError::serialization("bedrock", e.to_string()))?;

        self.transform_response(&response_bytes, &request.model, "bedrock-request")
            .await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Bedrock streaming chat request: model={}", request.model);

        // Check if it's an embedding model
        if self.is_embedding_model(&request.model) {
            return Err(ProviderError::invalid_request(
                "bedrock",
                "Use embeddings endpoint for embedding models".to_string(),
            ));
        }

        // Get model configuration
        let model_config = get_model_config(&request.model)?;

        if !model_config.supports_streaming {
            return Err(ProviderError::not_supported(
                "bedrock",
                format!("Model {} does not support streaming", request.model),
            ));
        }

        // Transform request
        let body = self.transform_request(request.clone(), context).await?;

        // Use streaming endpoint
        let operation = match model_config.api_type {
            super::model_config::BedrockApiType::ConverseStream => "converse-stream",
            super::model_config::BedrockApiType::InvokeStream => "invoke-with-response-stream",
            _ => {
                return Err(ProviderError::not_supported(
                    "bedrock",
                    format!(
                        "Model {} does not support streaming with API type {:?}",
                        request.model, model_config.api_type
                    ),
                ));
            }
        };

        // Send streaming request
        let response = self
            .client
            .send_streaming_request(&request.model, operation, &body)
            .await?;

        // Create BedrockStream
        let stream = super::streaming::BedrockStream::new(
            response.bytes_stream(),
            model_config.family.clone(),
        );

        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        debug!("Bedrock embedding request: model={}", request.model);

        // Use the embeddings module
        super::embeddings::execute_embedding(&self.client, &request).await
    }

    async fn health_check(&self) -> HealthStatus {
        match self.client.health_check().await {
            Ok(is_healthy) => {
                if is_healthy {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Unhealthy
                }
            }
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        BedrockErrorMapper
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        CostCalculator::calculate_cost(model, input_tokens, output_tokens)
            .ok_or_else(|| ProviderError::model_not_found("bedrock", model.to_string()))
    }
}
