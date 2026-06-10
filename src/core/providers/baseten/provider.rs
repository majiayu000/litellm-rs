//! Main Baseten Provider Implementation
//!
//! Implements the LLMProvider trait for Baseten's serverless ML inference.

use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::BasetenConfig;
use super::error::BasetenError;
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

/// Static capabilities for Baseten provider
const BASETEN_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

/// Supported OpenAI parameters for Baseten
const SUPPORTED_OPENAI_PARAMS: &[&str] = &[
    "max_tokens",
    "max_completion_tokens",
    "response_format",
    "seed",
    "stop",
    "stream",
    "temperature",
    "top_p",
    "tool_choice",
    "tools",
    "user",
    "presence_penalty",
    "frequency_penalty",
    "stream_options",
];

/// Baseten provider implementation
#[derive(Debug, Clone)]
pub struct BasetenProvider {
    config: BasetenConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl BasetenProvider {
    /// Create a new Baseten provider instance
    pub async fn new(config: BasetenConfig) -> Result<Self, BasetenError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| BasetenError::configuration("baseten", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            BasetenError::configuration("baseten", format!("Failed to create pool manager: {}", e))
        })?);

        // Build default model list - Baseten supports custom deployments
        // so we provide a minimal default list
        let models = vec![ModelInfo {
            id: "baseten-custom".to_string(),
            name: "Baseten Custom Model".to_string(),
            provider: "baseten".to_string(),
            max_context_length: 128000,
            max_output_length: Some(4096),
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: false,
            input_cost_per_1k_tokens: None,  // Depends on deployment
            output_cost_per_1k_tokens: None, // Depends on deployment
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
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, BasetenError> {
        let config = BasetenConfig {
            api_key: Some(api_key.into()),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Get the appropriate API base for a model
    fn get_api_base_for_request(&self, model: &str) -> String {
        // Check if custom api_base is set
        if let Some(custom_base) = &self.config.api_base {
            return custom_base.clone();
        }

        // Otherwise use model-based API base selection
        BasetenConfig::get_api_base_for_model(model)
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, BasetenError> {
        let mut headers = Vec::with_capacity(2);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));

        let response = self
            .pool_manager
            .execute_request(url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| BasetenError::network("baseten", e.to_string()))?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| BasetenError::network("baseten", e.to_string()))?;

        serde_json::from_slice(&response_bytes).map_err(|e| {
            BasetenError::api_error("baseten", 500, format!("Failed to parse response: {}", e))
        })
    }
}

impl LLMProvider for BasetenProvider {
    fn name(&self) -> &'static str {
        "baseten"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        BASETEN_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        SUPPORTED_OPENAI_PARAMS
    }

    async fn map_openai_params(
        &self,
        mut params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, ProviderError> {
        // Map max_completion_tokens to max_tokens if present
        if let Some(max_completion_tokens) = params.remove("max_completion_tokens") {
            params.insert("max_tokens".to_string(), max_completion_tokens);
        }
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, ProviderError> {
        serde_json::to_value(&request)
            .map_err(|e| BasetenError::invalid_request("baseten", e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        serde_json::from_slice(raw_response).map_err(|e| {
            BasetenError::api_error("baseten", 500, format!("Failed to parse response: {}", e))
        })
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(crate::core::traits::error_mapper::DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("Baseten chat request: model={}", request.model);

        let api_base = self.get_api_base_for_request(&request.model);
        let url = format!("{}/chat/completions", api_base);

        let request_json = serde_json::to_value(&request)
            .map_err(|e| BasetenError::invalid_request("baseten", e.to_string()))?;

        let response = self.execute_request(&url, request_json).await?;

        serde_json::from_value(response).map_err(|e| {
            BasetenError::api_error(
                "baseten",
                500,
                format!("Failed to parse chat response: {}", e),
            )
        })
    }

    async fn chat_completion_stream(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        debug!("Baseten streaming request: model={}", request.model);

        request.stream = true;

        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| BasetenError::authentication("baseten", "API key is required"))?;

        let api_base = self.get_api_base_for_request(&request.model);
        let url = format!("{}/chat/completions", api_base);

        let client = crate::core::http::outbound::streaming_outbound_client().clone();
        let response = crate::core::providers::base::connection_pool::send_streaming_request(
            client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request),
            "baseten",
        )
        .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            return Err(match status {
                400 => BasetenError::invalid_request(
                    "baseten",
                    body.unwrap_or_else(|| "Bad request".to_string()),
                ),
                401 => BasetenError::authentication("baseten", "Invalid API key"),
                429 => BasetenError::rate_limit("baseten", None),
                _ => BasetenError::api_error(
                    "baseten",
                    status,
                    format!("Stream request failed: {}", status),
                ),
            });
        }

        // Create SSE stream using unified SSE parser
        let transformer = OpenAICompatibleTransformer::new("baseten");
        let inner_stream = UnifiedSSEStream::new(Box::pin(response.bytes_stream()), transformer);

        // Wrap to convert BasetenError to BasetenError
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
        Err(BasetenError::not_supported(
            "baseten",
            "Baseten embeddings require a specific embedding model deployment. \
             Please specify the model ID of your embedding deployment.",
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
        // Baseten pricing depends on the specific deployment
        // Return 0 as a placeholder - actual costs should be tracked via Baseten dashboard
        Ok(0.0)
    }
}
