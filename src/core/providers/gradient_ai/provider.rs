//! Main Gradient AI Provider Implementation
//!
//! Implements the LLMProvider trait for Gradient AI's agent and model platform.

use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::GradientAIConfig;
use super::error::GradientAIErrorMapper;
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

/// Static capabilities for Gradient AI provider
const GRADIENT_AI_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
];

/// Supported OpenAI parameters for Gradient AI
const SUPPORTED_OPENAI_PARAMS: &[&str] = &[
    "frequency_penalty",
    "max_tokens",
    "max_completion_tokens",
    "presence_penalty",
    "stop",
    "stream",
    "stream_options",
    "temperature",
    "top_p",
    // Gradient AI specific parameters
    "k",
    "kb_filters",
    "filter_kb_content_by_query_metadata",
    "instruction_override",
    "include_functions_info",
    "include_retrieval_info",
    "include_guardrails_info",
    "provide_citations",
    "retrieval_method",
];

/// Gradient AI provider implementation
#[derive(Debug, Clone)]
pub struct GradientAIProvider {
    config: GradientAIConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl GradientAIProvider {
    /// Create a new Gradient AI provider instance
    pub async fn new(config: GradientAIConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("gradient_ai", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                "gradient_ai",
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        // Build default model list
        let models = vec![ModelInfo {
            id: "gradient-ai-agent".to_string(),
            name: "Gradient AI Agent".to_string(),
            provider: "gradient_ai".to_string(),
            max_context_length: 128000,
            max_output_length: Some(4096),
            supports_streaming: true,
            supports_tools: false, // Gradient AI uses KB-based retrieval instead of tool calling
            supports_multimodal: false,
            input_cost_per_1k_tokens: None,
            output_cost_per_1k_tokens: None,
            currency: "USD".to_string(),
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
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
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = GradientAIConfig {
            api_key: Some(api_key.into()),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Build request body with Gradient AI specific parameters
    fn build_request_body(&self, request: &ChatRequest) -> serde_json::Value {
        let mut body = serde_json::to_value(request).unwrap_or_default();

        // Add Gradient AI specific parameters from config
        if let Some(k) = self.config.k {
            body["k"] = serde_json::json!(k);
        }
        if let Some(ref kb_filters) = self.config.kb_filters {
            body["kb_filters"] = serde_json::json!(kb_filters);
        }
        if let Some(filter) = self.config.filter_kb_content_by_query_metadata {
            body["filter_kb_content_by_query_metadata"] = serde_json::json!(filter);
        }
        if let Some(ref instruction) = self.config.instruction_override {
            body["instruction_override"] = serde_json::json!(instruction);
        }
        if let Some(include) = self.config.include_functions_info {
            body["include_functions_info"] = serde_json::json!(include);
        }
        if let Some(include) = self.config.include_retrieval_info {
            body["include_retrieval_info"] = serde_json::json!(include);
        }
        if let Some(include) = self.config.include_guardrails_info {
            body["include_guardrails_info"] = serde_json::json!(include);
        }
        if let Some(provide) = self.config.provide_citations {
            body["provide_citations"] = serde_json::json!(provide);
        }
        if let Some(ref method) = self.config.retrieval_method {
            body["retrieval_method"] = serde_json::json!(method);
        }

        body
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let mut headers = Vec::with_capacity(2);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));

        let response = self
            .pool_manager
            .execute_request(url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| ProviderError::network("gradient_ai", e.to_string()))?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("gradient_ai", e.to_string()))?;

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::api_error(
                "gradient_ai",
                500,
                format!("Failed to parse response: {}", e),
            )
        })
    }
}

impl LLMProvider for GradientAIProvider {
    fn name(&self) -> &'static str {
        "gradient_ai"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        GRADIENT_AI_CAPABILITIES
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
        // Gradient AI uses compatible parameters
        // Check for unsupported params could be added here if drop_params is not set
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, ProviderError> {
        Ok(self.build_request_body(&request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::api_error(
                "gradient_ai",
                500,
                format!("Failed to parse response: {}", e),
            )
        })
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(GradientAIErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("Gradient AI chat request: model={}", request.model);

        let url = self.config.get_complete_url();
        let request_body = self.build_request_body(&request);

        let response = self.execute_request(&url, request_body).await?;

        serde_json::from_value(response).map_err(|e| {
            ProviderError::api_error(
                "gradient_ai",
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
        debug!("Gradient AI streaming request: model={}", request.model);

        request.stream = true;

        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| ProviderError::authentication("gradient_ai", "API key is required"))?;

        let url = self.config.get_complete_url();
        let request_body = self.build_request_body(&request);

        let client = crate::core::http::outbound::streaming_outbound_client().clone();
        let response = crate::core::providers::base::connection_pool::send_streaming_request(
            client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request_body),
            "gradient_ai",
        )
        .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            return Err(match status {
                400 => ProviderError::invalid_request(
                    "gradient_ai",
                    body.unwrap_or_else(|| "Bad request".to_string()),
                ),
                401 => ProviderError::authentication("gradient_ai", "Invalid API key"),
                429 => ProviderError::rate_limit("gradient_ai", None),
                _ => ProviderError::streaming_error(
                    "gradient_ai",
                    "chat",
                    None,
                    None,
                    format!("Stream request failed: {}", status),
                ),
            });
        }

        // Create SSE stream using unified SSE parser
        let transformer = OpenAICompatibleTransformer::new("gradient_ai");
        let inner_stream = UnifiedSSEStream::new(Box::pin(response.bytes_stream()), transformer);

        // Wrap to convert ProviderError to ProviderError (they're the same type now)
        let mapped_stream = futures::stream::unfold(inner_stream, |mut stream| async move {
            use futures::StreamExt;
            match stream.next().await {
                Some(Ok(chunk)) => Some((Ok(chunk), stream)),
                Some(Err(e)) => Some((
                    Err(ProviderError::streaming_error(
                        "gradient_ai",
                        "chat",
                        None,
                        None,
                        e.to_string(),
                    )),
                    stream,
                )),
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
        Err(ProviderError::not_supported(
            "gradient_ai",
            "Gradient AI does not support embeddings through this endpoint. \
             Use the Gradient AI embeddings API directly.",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Simple health check - try to validate API connectivity
        let url = format!(
            "{}/health",
            self.config
                .get_api_base()
                .trim_end_matches("/v1/chat/completions")
        );
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
        // Gradient AI pricing depends on the specific agent configuration
        // Return 0 as a placeholder - actual costs should be tracked via Gradient AI dashboard
        Ok(0.0)
    }
}
