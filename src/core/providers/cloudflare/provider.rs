//! Main Cloudflare Workers AI Provider Implementation
//!
//! Implements the LLMProvider trait for Cloudflare's Workers AI models.

use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::CloudflareConfig;
use super::model_info::{calculate_cost, get_available_models, get_model_info};
use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEStream};
use crate::core::providers::base::{
    GlobalPoolManager, HttpMethod, header, read_streaming_error_body,
};
use crate::core::providers::openai::{
    OpenAIRequestTransformer, OpenAIResponseTransformer, models::OpenAIChatResponse,
};
use crate::core::providers::unified_provider::{ProviderError, default_http_error_mapper};
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

/// Static capabilities for Cloudflare provider
const CLOUDFLARE_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

/// Cloudflare Workers AI provider implementation
#[derive(Debug, Clone)]
pub struct CloudflareProvider {
    config: CloudflareConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl CloudflareProvider {
    /// Create a new Cloudflare provider instance
    pub async fn new(config: CloudflareConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("cloudflare", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                "cloudflare",
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        // Build model list from static configuration
        let models = get_available_models()
            .iter()
            .filter_map(|id| get_model_info(id))
            .map(|info| {
                let mut capabilities = vec![ProviderCapability::ChatCompletion];
                if info.supports_tools {
                    capabilities.push(ProviderCapability::ToolCalling);
                }
                if info.supports_streaming {
                    capabilities.push(ProviderCapability::ChatCompletionStream);
                }

                ModelInfo {
                    id: format!("cloudflare/{}", info.model_id),
                    name: info.display_name.to_string(),
                    provider: "cloudflare".to_string(),
                    max_context_length: info.max_context_length,
                    max_output_length: info.max_output_length,
                    supports_streaming: info.supports_streaming,
                    supports_tools: info.supports_tools,
                    supports_multimodal: info.supports_multimodal,
                    input_cost_per_1k_tokens: Some(info.input_cost_per_million / 1000.0),
                    output_cost_per_1k_tokens: Some(info.output_cost_per_million / 1000.0),
                    currency: "USD".to_string(),
                    capabilities,
                    created_at: None,
                    updated_at: None,
                    metadata: HashMap::new(),
                }
            })
            .collect();

        Ok(Self {
            config,
            pool_manager,
            models,
        })
    }

    /// Create provider with account ID and token
    pub async fn with_credentials(
        account_id: impl Into<String>,
        api_token: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let config = CloudflareConfig {
            account_id: Some(account_id.into()),
            api_token: Some(api_token.into()),
            ..Default::default()
        };
        Self::new(config).await
    }

    fn chat_url(&self) -> Result<String, ProviderError> {
        let account_id = self
            .config
            .get_account_id()
            .ok_or_else(|| ProviderError::configuration("cloudflare", "Account ID is required"))?;
        Ok(format!(
            "{}/accounts/{}/ai/v1/chat/completions",
            self.config.get_api_base().trim_end_matches('/'),
            account_id
        ))
    }

    /// Execute a non-streaming OpenAI-compatible chat request.
    async fn execute_request(
        &self,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let url = self.chat_url()?;

        let mut headers = Vec::with_capacity(2);
        if let Some(api_token) = self.config.get_api_token() {
            headers.push(header("Authorization", format!("Bearer {}", api_token)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| ProviderError::network("cloudflare", e.to_string()))?;

        let status = response.status();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        let response_bytes = response.bytes().await.map_err(|e| {
            if status.is_success() {
                ProviderError::network("cloudflare", e.to_string())
            } else {
                Self::http_error(status.as_u16(), &e.to_string(), retry_after)
            }
        })?;

        if !status.is_success() {
            return Err(Self::http_error(
                status.as_u16(),
                &String::from_utf8_lossy(&response_bytes),
                retry_after,
            ));
        }

        serde_json::from_slice(&response_bytes)
            .map_err(|e| ProviderError::response_parsing("cloudflare", e.to_string()))
    }

    fn http_error(status: u16, body: &str, retry_after: Option<u64>) -> ProviderError {
        if status == 429 && retry_after.is_some() {
            return ProviderError::rate_limit("cloudflare", retry_after);
        }
        default_http_error_mapper("cloudflare", status, body)
    }

    fn decode_response(mut response: serde_json::Value) -> Result<ChatResponse, ProviderError> {
        if response.get("success") == Some(&serde_json::Value::Bool(false)) {
            let message = response["errors"]
                .as_array()
                .and_then(|errors| errors.first())
                .and_then(|error| error["message"].as_str())
                .unwrap_or("Cloudflare reported an unsuccessful response");
            return Err(ProviderError::api_error("cloudflare", 502, message));
        }
        crate::core::providers::shared::normalize_openai_chat_response_usage(&mut response);
        let response: OpenAIChatResponse = serde_json::from_value(response)
            .map_err(|e| ProviderError::response_parsing("cloudflare", e.to_string()))?;
        if response.choices.is_empty() {
            return Err(ProviderError::response_parsing(
                "cloudflare",
                "Missing completion choices",
            ));
        }
        OpenAIResponseTransformer::transform(response)
            .map_err(|e| ProviderError::response_parsing("cloudflare", e.to_string()))
    }

    fn transform_to_cloudflare_format(
        &self,
        mut request: ChatRequest,
    ) -> Result<serde_json::Value, ProviderError> {
        if request.thinking.is_some() {
            return Err(ProviderError::not_supported(
                "cloudflare",
                "Use reasoning_effort or provider-native parameters for reasoning configuration",
            ));
        }
        request.model = request
            .model
            .strip_prefix("cloudflare/")
            .unwrap_or(&request.model)
            .to_string();
        let stream = request.stream;
        let mut wire = OpenAIRequestTransformer::transform(request)
            .map_err(|e| ProviderError::invalid_request("cloudflare", e.to_string()))?;
        wire.stream = Some(stream);
        serde_json::to_value(wire)
            .map_err(|e| ProviderError::serialization("cloudflare", e.to_string()))
    }
}

impl LLMProvider for CloudflareProvider {
    fn name(&self) -> &'static str {
        "cloudflare"
    }

    fn error_provider_name(&self) -> &'static str {
        "cloudflare"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        CLOUDFLARE_CAPABILITIES
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
            "frequency_penalty",
            "presence_penalty",
            "n",
            "seed",
            "tools",
            "tool_choice",
            "response_format",
            "reasoning_effort",
            "stream_options",
            "max_completion_tokens",
            "parallel_tool_calls",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, ProviderError> {
        // Cloudflare supports a subset of OpenAI parameters directly
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, ProviderError> {
        self.transform_to_cloudflare_format(request)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let response = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("cloudflare", e.to_string()))?;
        Self::decode_response(response)
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(crate::core::traits::error_mapper::DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("Cloudflare chat request: model={}", request.model);

        let mut request = request;
        request.stream = false;
        let body = self.transform_to_cloudflare_format(request)?;
        let response = self.execute_request(body).await?;
        Self::decode_response(response)
    }

    async fn chat_completion_stream(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        request.stream = true;
        let body = self.transform_to_cloudflare_format(request)?;
        let mut headers = vec![header("Content-Type", "application/json".to_string())];
        if let Some(token) = self.config.get_api_token() {
            headers.push(header("Authorization", format!("Bearer {}", token)));
        }
        let response = self
            .pool_manager
            .execute_streaming_request(&self.chat_url()?, headers, body, "cloudflare")
            .await?;
        let status = response.status();
        if !status.is_success() {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let body = read_streaming_error_body(response)
                .await
                .map_err(|e| Self::http_error(status.as_u16(), &e.to_string(), retry_after))?;
            return Err(Self::http_error(status.as_u16(), &body, retry_after));
        }
        Ok(Box::pin(UnifiedSSEStream::new(
            response.bytes_stream(),
            OpenAICompatibleTransformer::new("cloudflare"),
        )))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        // Cloudflare also supports embeddings models, but we'll implement text generation first
        Err(ProviderError::not_supported(
            "cloudflare",
            "Embeddings are not yet implemented for Cloudflare provider",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Simple health check - try to list models
        let account_id = match self.config.get_account_id() {
            Some(id) => id,
            None => return HealthStatus::Unhealthy,
        };

        let url = format!(
            "{}/accounts/{}/ai/models/search",
            self.config.get_api_base(),
            account_id
        );

        let mut headers = Vec::with_capacity(1);
        if let Some(api_token) = self.config.get_api_token() {
            headers.push(header("Authorization", format!("Bearer {}", api_token)));
        }

        match self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None::<serde_json::Value>)
            .await
        {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::Unhealthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        calculate_cost(model, input_tokens, output_tokens)
            .ok_or_else(|| ProviderError::model_not_found("cloudflare", model))
    }
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
