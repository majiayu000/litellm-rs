//! Main Clarifai Provider Implementation
//!
//! Implements the LLMProvider trait for Clarifai's AI platform.

use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::ClarifaiConfig;
use super::error::ClarifaiError;
use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEStream};
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::{
    provider::ProviderConfig as _, provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse},
};

/// Static capabilities for Clarifai provider
const CLARIFAI_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

/// Supported OpenAI parameters for Clarifai
const SUPPORTED_OPENAI_PARAMS: &[&str] = &[
    "max_tokens",
    "max_completion_tokens",
    "response_format",
    "stream",
    "temperature",
    "top_p",
    "tool_choice",
    "tools",
    "presence_penalty",
    "frequency_penalty",
    "stream_options",
];

/// Clarifai provider implementation
#[derive(Debug, Clone)]
pub struct ClarifaiProvider {
    config: ClarifaiConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl ClarifaiProvider {
    /// Create a new Clarifai provider instance
    pub async fn new(config: ClarifaiConfig) -> Result<Self, ClarifaiError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ClarifaiError::configuration("clarifai", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ClarifaiError::configuration(
                "clarifai",
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        // Build default model list - Clarifai hosts various models
        let models = vec![ModelInfo {
            id: "clarifai-custom".to_string(),
            name: "Clarifai Custom Model".to_string(),
            provider: "clarifai".to_string(),
            max_context_length: 128000,
            max_output_length: Some(4096),
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: false,
            input_cost_per_1k_tokens: None,
            output_cost_per_1k_tokens: None,
            currency: "USD".to_string(),
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
                ProviderCapability::ToolCalling,
            ],
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        }];

        Ok(Self {
            config,
            pool_manager,
            models,
        })
    }

    /// Create provider with API key only
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ClarifaiError> {
        let config = ClarifaiConfig {
            api_key: Some(api_key.into()),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Transform model name to Clarifai URL format if needed
    fn transform_model(&self, model: &str) -> String {
        // If model is in user.app.model format, convert to URL
        if let Some(url) = ClarifaiConfig::get_model_url(model) {
            url
        } else {
            // Otherwise use as-is
            model.to_string()
        }
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ClarifaiError> {
        let url = format!("{}{}", self.config.get_api_base(), endpoint);

        let mut headers = Vec::with_capacity(2);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| ClarifaiError::network("clarifai", e.to_string()))?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ClarifaiError::network("clarifai", e.to_string()))?;

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ClarifaiError::api_error("clarifai", 500, format!("Failed to parse response: {}", e))
        })
    }
}

impl LLMProvider for ClarifaiProvider {
    fn name(&self) -> &'static str {
        "clarifai"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        CLARIFAI_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        SUPPORTED_OPENAI_PARAMS
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, ProviderError> {
        // Clarifai uses OpenAI-compatible parameters
        Ok(params)
    }

    async fn transform_request(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, ProviderError> {
        // Transform model name to Clarifai URL format
        request.model = self.transform_model(&request.model);

        serde_json::to_value(&request)
            .map_err(|e| ClarifaiError::invalid_request("clarifai", e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let mut response: ChatResponse = serde_json::from_slice(raw_response).map_err(|e| {
            ClarifaiError::api_error("clarifai", 500, format!("Failed to parse response: {}", e))
        })?;

        // Prefix the model with clarifai/ in the response
        if !response.model.starts_with("clarifai/") {
            response.model = format!("clarifai/{}", model);
        }

        Ok(response)
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(crate::core::traits::error_mapper::DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("Clarifai chat request: model={}", request.model);

        let original_model = request.model.clone();

        // Transform model name
        request.model = self.transform_model(&request.model);

        let request_json = serde_json::to_value(&request)
            .map_err(|e| ClarifaiError::invalid_request("clarifai", e.to_string()))?;

        let response = self
            .execute_request("/chat/completions", request_json)
            .await?;

        let mut chat_response: ChatResponse = serde_json::from_value(response).map_err(|e| {
            ClarifaiError::api_error(
                "clarifai",
                500,
                format!("Failed to parse chat response: {}", e),
            )
        })?;

        // Prefix model name with clarifai/
        if !chat_response.model.starts_with("clarifai/") {
            chat_response.model = format!("clarifai/{}", original_model);
        }

        Ok(chat_response)
    }

    async fn chat_completion_stream(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        debug!("Clarifai streaming request: model={}", request.model);

        request.stream = true;

        // Transform model name
        request.model = self.transform_model(&request.model);

        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| ClarifaiError::authentication("clarifai", "API key is required"))?;

        let url = format!("{}/chat/completions", self.config.get_api_base());

        let client = crate::core::http::outbound::streaming_outbound_client().clone();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ClarifaiError::network("clarifai", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            return Err(match status {
                400 => ClarifaiError::invalid_request(
                    "clarifai",
                    body.unwrap_or_else(|| "Bad request".to_string()),
                ),
                401 => ClarifaiError::authentication("clarifai", "Invalid API key"),
                429 => ClarifaiError::rate_limit("clarifai", None),
                _ => ClarifaiError::api_error(
                    "clarifai",
                    status,
                    format!("Stream request failed: {}", status),
                ),
            });
        }

        // Create SSE stream using unified SSE parser
        let transformer = OpenAICompatibleTransformer::new("clarifai");
        let inner_stream = UnifiedSSEStream::new(Box::pin(response.bytes_stream()), transformer);

        // Wrap to convert ClarifaiError to ClarifaiError
        let mapped_stream = futures::stream::unfold(inner_stream, |mut stream| async move {
            use futures::StreamExt;
            match stream.next().await {
                Some(Ok(chunk)) => Some((Ok(chunk), stream)),
                Some(Err(e)) => Some((Err(e), stream)),
                None => None,
            }
        });

        Ok(Box::pin(mapped_stream))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        Err(ClarifaiError::not_supported(
            "clarifai",
            "Clarifai embeddings require a specific embedding model. \
             Please specify the model in user.app.model format.",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Simple health check - try to validate API key
        let url = format!("{}/models", self.config.get_api_base());
        let mut headers = Vec::with_capacity(1);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        match self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None::<serde_json::Value>)
            .await
        {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        // Clarifai pricing depends on the specific model and deployment
        // Return 0 as a placeholder - actual costs should be tracked via Clarifai dashboard
        Ok(0.0)
    }
}
