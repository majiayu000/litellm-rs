//! OpenAI Provider Client Implementation
//!
//! Unified client following the new provider architecture

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpMethod, header, header_owned, streaming_client,
};
use crate::core::providers::unified_provider::ProviderError;
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
    config::OpenAIConfig,
    models::{OpenAIModelFeature, OpenAIModelRegistry, OpenAIUseCase, get_openai_registry},
};

/// OpenAI Provider implementation using unified architecture
#[derive(Debug, Clone)]
pub struct OpenAIProvider {
    /// Connection pool manager
    pub(crate) pool_manager: Arc<GlobalPoolManager>,
    /// Provider configuration
    pub(crate) config: OpenAIConfig,
    /// Model registry
    pub(crate) model_registry: &'static OpenAIModelRegistry,
}

impl OpenAIProvider {
    /// Generate headers for OpenAI API requests
    ///
    /// Uses `HeaderPair` with Cow for static keys to avoid allocations.
    pub(crate) fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(4); // Pre-allocate for typical case

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        if let Some(org) = &self.config.organization {
            headers.push(header("OpenAI-Organization", org.clone()));
        }

        if let Some(project) = &self.config.project {
            headers.push(header("OpenAI-Project", project.clone()));
        }

        // Add custom headers (both key and value are dynamic)
        for (key, value) in &self.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }

    /// Create new OpenAI provider
    pub async fn new(config: OpenAIConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::Configuration {
                provider: "openai",
                message: e.to_string(),
            })?;

        // Note: Headers are now built per-request in get_request_headers()
        // This avoids redundant HashMap allocation during initialization.

        let pool_manager =
            Arc::new(
                GlobalPoolManager::new().map_err(|e| ProviderError::Network {
                    provider: "openai",
                    message: e.to_string(),
                })?,
            );
        let model_registry = get_openai_registry();

        Ok(Self {
            pool_manager,
            config,
            model_registry,
        })
    }

    /// Create provider with API key only
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let mut config = OpenAIConfig::default();
        config.base.api_key = Some(api_key.into());
        Self::new(config).await
    }

    /// Execute chat completion request
    async fn execute_chat_completion(
        &self,
        request: ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        // Transform request to OpenAI format
        let openai_request = self.transform_chat_request(request)?;

        // Execute HTTP request using unified connection pool
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let headers = self.get_request_headers();
        let body = Some(openai_request);

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body)
            .await
            .map_err(|e| ProviderError::Network {
                provider: "openai",
                message: e.to_string(),
            })?;

        let response_bytes = response.bytes().await.map_err(|e| ProviderError::Network {
            provider: "openai",
            message: e.to_string(),
        })?;

        let response_json: Value = serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::ResponseParsing {
                provider: "openai",
                message: e.to_string(),
            }
        })?;

        // Transform response back to standard format
        self.transform_chat_response(response_json)
    }

    /// Execute streaming chat completion
    async fn execute_chat_completion_stream(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        // Transform request with streaming enabled
        let mut openai_request = self.transform_chat_request(request)?;
        openai_request["stream"] = Value::Bool(true);

        // Get API key
        let api_key =
            self.config
                .base
                .api_key
                .as_ref()
                .ok_or_else(|| ProviderError::Authentication {
                    provider: "openai",
                    message: "API key is required".to_string(),
                })?;

        // Execute streaming request using the global connection pool
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let client = streaming_client();
        let mut req = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&openai_request);

        // Add organization header if present
        if let Some(org) = &self.config.organization {
            req = req.header("OpenAI-Organization", org);
        }

        // Add project header if present
        if let Some(project) = &self.config.project {
            req = req.header("OpenAI-Project", project);
        }

        let response = req.send().await.map_err(|e| ProviderError::Network {
            provider: "openai",
            message: e.to_string(),
        })?;

        // Create OpenAI-specific stream handler using unified SSE parser
        let stream = response.bytes_stream();
        Ok(Box::pin(super::streaming::create_openai_stream(stream)))
    }

    /// Transform ChatRequest to OpenAI API format
    pub(crate) fn transform_chat_request(
        &self,
        request: ChatRequest,
    ) -> Result<Value, ProviderError> {
        let mut openai_request = serde_json::json!({
            "model": self.config.get_model_mapping(&request.model),
            "messages": request.messages
        });

        // Add optional parameters
        if let Some(temp) = request.temperature {
            let temp_f64 = temp as f64;
            openai_request["temperature"] = Value::Number(
                serde_json::Number::from_f64(temp_f64).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "openai",
                        format!("invalid temperature value: {temp_f64} (NaN and Infinity are not allowed)"),
                    )
                })?,
            );
        }

        if let Some(max_tokens) = request.max_tokens {
            openai_request["max_tokens"] = Value::Number(serde_json::Number::from(max_tokens));
        }

        if let Some(max_completion_tokens) = request.max_completion_tokens {
            openai_request["max_completion_tokens"] =
                Value::Number(serde_json::Number::from(max_completion_tokens));
        }

        if let Some(top_p) = request.top_p {
            let top_p_f64 = top_p as f64;
            openai_request["top_p"] =
                Value::Number(serde_json::Number::from_f64(top_p_f64).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "openai",
                        format!(
                            "invalid top_p value: {top_p_f64} (NaN and Infinity are not allowed)"
                        ),
                    )
                })?);
        }

        if let Some(tools) = request.tools {
            openai_request["tools"] = serde_json::to_value(tools)?;
        }

        if let Some(tool_choice) = request.tool_choice {
            openai_request["tool_choice"] = serde_json::to_value(tool_choice)?;
        }

        if let Some(response_format) = request.response_format {
            openai_request["response_format"] = serde_json::to_value(response_format)?;
        }

        if let Some(stop) = request.stop {
            openai_request["stop"] = serde_json::to_value(stop)?;
        }

        if let Some(user) = request.user {
            openai_request["user"] = Value::String(user);
        }

        if let Some(seed) = request.seed {
            openai_request["seed"] = Value::Number(serde_json::Number::from(seed));
        }

        if let Some(n) = request.n {
            openai_request["n"] = Value::Number(serde_json::Number::from(n));
        }

        if let Some(stream_options) = request.stream_options {
            openai_request["stream_options"] = serde_json::to_value(stream_options)?;
        }

        // Add extra parameters from config
        // Skip extra_params as BaseConfig doesn't have it

        Ok(openai_request)
    }

    /// Transform OpenAI response to standard format
    fn transform_chat_response(&self, response: Value) -> Result<ChatResponse, ProviderError> {
        let response: crate::core::providers::openai::models::OpenAIChatResponse =
            serde_json::from_value(response)?;

        // Use existing transformer logic
        use crate::core::providers::openai::transformer::OpenAIResponseTransformer;
        OpenAIResponseTransformer::transform(response).map_err(|e| ProviderError::Other {
            provider: "openai",
            message: e.to_string(),
        })
    }

    /// Get model information with validation
    pub fn get_model_info(
        &self,
        model_id: &str,
    ) -> Result<crate::core::types::model::ModelInfo, ProviderError> {
        // Return a default ModelInfo for any model
        // Like Python LiteLLM, we don't validate models locally
        use crate::core::types::model::ModelInfo;
        Ok(ModelInfo {
            id: model_id.to_string(),
            name: model_id.to_string(),
            provider: "openai".to_string(),
            max_context_length: 128000, // Default context
            max_output_length: Some(4096),
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: false,
            capabilities: vec![], // Empty capabilities, API will handle validation
            input_cost_per_1k_tokens: None,
            output_cost_per_1k_tokens: None,
            currency: "USD".to_string(),
            created_at: None,
            updated_at: None,
            metadata: std::collections::HashMap::new(),
        })
    }

    /// Check if model supports a specific capability
    pub fn model_supports_capability(
        &self,
        model_id: &str,
        capability: &ProviderCapability,
    ) -> bool {
        if let Some(model_spec) = self.model_registry.get_model_spec(model_id) {
            model_spec.model_info.capabilities.contains(capability)
        } else {
            false
        }
    }

    /// Get model configuration
    pub fn get_model_config(&self, model_id: &str) -> Option<&super::models::OpenAIModelConfig> {
        self.model_registry
            .get_model_spec(model_id)
            .map(|spec| &spec.config)
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    type Config = OpenAIConfig;
    type Error = ProviderError;
    type ErrorMapper = crate::core::traits::error_mapper::implementations::OpenAIErrorMapper;

    fn name(&self) -> &'static str {
        "openai"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        static CAPABILITIES: &[ProviderCapability] = &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::Embeddings,
            ProviderCapability::ImageGeneration,
            ProviderCapability::AudioTranscription,
            ProviderCapability::ToolCalling,
            ProviderCapability::FunctionCalling,
            // New capabilities
            ProviderCapability::FineTuning,
            ProviderCapability::ImageEdit,
            ProviderCapability::ImageVariation,
            ProviderCapability::RealtimeApi,
        ];
        CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        static MODELS: std::sync::LazyLock<Vec<ModelInfo>> =
            std::sync::LazyLock::new(|| get_openai_registry().get_all_models());
        &MODELS
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        // Like Python LiteLLM, we don't validate models locally
        // OpenAI API will handle invalid models

        // Execute request
        self.execute_chat_completion(request).await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        // Like Python LiteLLM, we don't validate models locally
        // OpenAI API will handle invalid models

        self.execute_chat_completion_stream(request).await
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        // Calls the inherent `embeddings` method from api_methods.rs
        // (2-arg inherent method takes priority over 3-arg trait method)
        self.embeddings(request).await
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        let response = self
            .generate_images(
                request.prompt,
                Some(request.model.unwrap_or_else(|| "dall-e-3".to_string())),
                request.n,
                request.size,
                request.quality,
                request.style,
            )
            .await?;

        serde_json::from_value(response).map_err(|e| ProviderError::ResponseParsing {
            provider: "openai",
            message: e.to_string(),
        })
    }

    async fn health_check(&self) -> HealthStatus {
        let url = format!("{}/models?limit=1", self.config.get_api_base());
        let client = reqwest::Client::new();
        let mut req = client.get(&url);

        if let Some(api_key) = &self.config.base.api_key {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        match req.send().await {
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
        let model_info = self.get_model_info(model)?;

        let input_cost = model_info
            .input_cost_per_1k_tokens
            .map(|cost| (input_tokens as f64 / 1000.0) * cost)
            .unwrap_or(0.0);

        let output_cost = model_info
            .output_cost_per_1k_tokens
            .map(|cost| (output_tokens as f64 / 1000.0) * cost)
            .unwrap_or(0.0);

        Ok(input_cost + output_cost)
    }

    // ==================== Python LiteLLM Compatible Interface ====================

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        // Return parameters based on model capabilities
        if let Some(model_spec) = self.model_registry.get_model_spec(model) {
            match model_spec.family {
                super::models::OpenAIModelFamily::GPT4
                | super::models::OpenAIModelFamily::GPT4Turbo
                | super::models::OpenAIModelFamily::GPT4O => &[
                    "messages",
                    "model",
                    "temperature",
                    "max_tokens",
                    "max_completion_tokens",
                    "top_p",
                    "frequency_penalty",
                    "presence_penalty",
                    "stop",
                    "stream",
                    "tools",
                    "tool_choice",
                    "parallel_tool_calls",
                    "response_format",
                    "user",
                    "seed",
                    "n",
                    "logit_bias",
                    "logprobs",
                    "top_logprobs",
                ],
                super::models::OpenAIModelFamily::GPT35 => &[
                    "messages",
                    "model",
                    "temperature",
                    "max_tokens",
                    "top_p",
                    "frequency_penalty",
                    "presence_penalty",
                    "stop",
                    "stream",
                    "tools",
                    "tool_choice",
                    "response_format",
                    "user",
                    "n",
                    "logit_bias",
                ],
                super::models::OpenAIModelFamily::O1 => &[
                    "messages",
                    "model",
                    "max_completion_tokens",
                    "stream",
                    "user",
                ],
                _ => &[
                    "messages",
                    "model",
                    "temperature",
                    "max_tokens",
                    "top_p",
                    "stream",
                    "user",
                ],
            }
        } else {
            &[
                "messages",
                "model",
                "temperature",
                "max_tokens",
                "top_p",
                "stream",
                "user",
            ]
        }
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // OpenAI provider uses standard OpenAI parameters, no mapping needed
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        self.transform_chat_request(request)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response_value: Value = serde_json::from_slice(raw_response)?;
        self.transform_chat_response(response_value)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        crate::core::traits::error_mapper::implementations::OpenAIErrorMapper
    }
}

// Re-export error mapper from dedicated module
pub use super::error_mapper::OpenAIErrorMapper;

// ==================== Convenience Methods ====================

impl OpenAIProvider {
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

    /// List available models from static registry
    pub fn list_available_models(&self) -> Vec<String> {
        self.models().iter().map(|m| m.id.clone()).collect()
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
