//! Main GitHub Models Provider Implementation
//!
//! Implements the LLMProvider trait for GitHub Models API.
//! The API is OpenAI-compatible, making the implementation straightforward.

use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::GitHubConfig;
use super::model_info::{get_available_models, get_model_info};
use crate::core::providers::base::{GlobalPoolManager, HttpErrorMapper, HttpMethod, header};
use crate::core::providers::unified_provider::ProviderError;
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

/// Static capabilities for GitHub Models provider
const GITHUB_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

/// GitHub Models provider implementation
#[derive(Debug, Clone)]
pub struct GitHubProvider {
    config: GitHubConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl GitHubProvider {
    /// Create a new GitHub provider instance
    pub async fn new(config: GitHubConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("github", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration("github", format!("Failed to create pool manager: {}", e))
        })?);

        // Build model list from static configuration
        let models = get_available_models()
            .iter()
            .filter_map(|id| get_model_info(id))
            .map(|info| {
                let mut capabilities = vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ];
                if info.supports_tools {
                    capabilities.push(ProviderCapability::ToolCalling);
                }

                ModelInfo {
                    id: info.model_id.to_string(),
                    name: info.display_name.to_string(),
                    provider: "github".to_string(),
                    max_context_length: info.max_context_length,
                    max_output_length: Some(info.max_output_length),
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

    /// Create provider with API key only
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = GitHubConfig {
            api_key: Some(api_key.into()),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
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
            .map_err(|e| ProviderError::network("github", e.to_string()))?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("github", e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&response_bytes);
            let status_code = status.as_u16();
            return Err(match status_code {
                401 => ProviderError::authentication("github", "Invalid API key"),
                404 => ProviderError::model_not_found("github", body_str.to_string()),
                429 => ProviderError::rate_limit("github", None),
                400 => ProviderError::invalid_request("github", body_str.to_string()),
                500..=599 => ProviderError::provider_unavailable("github", body_str.to_string()),
                _ => HttpErrorMapper::map_status_code("github", status_code, &body_str),
            });
        }

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::api_error("github", 500, format!("Failed to parse response: {}", e))
        })
    }
}

impl LLMProvider for GitHubProvider {
    fn name(&self) -> &'static str {
        "github"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        GITHUB_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "temperature",
            "top_p",
            "max_tokens",
            "max_completion_tokens",
            "stream",
            "stop",
            "frequency_penalty",
            "presence_penalty",
            "n",
            "response_format",
            "seed",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "user",
            "logprobs",
            "top_logprobs",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, ProviderError> {
        // GitHub Models uses the same parameters as OpenAI
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, ProviderError> {
        // Convert to JSON value - GitHub Models is OpenAI-compatible
        serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("github", e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        // Parse response - GitHub Models uses OpenAI format
        let chat_response: ChatResponse = serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::api_error("github", 500, format!("Failed to parse response: {}", e))
        })?;

        Ok(chat_response)
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(crate::core::traits::error_mapper::DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("GitHub Models chat request: model={}", request.model);

        // Transform and execute
        let request_json = serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("github", e.to_string()))?;

        let response = self
            .execute_request("/chat/completions", request_json)
            .await?;

        serde_json::from_value(response).map_err(|e| {
            ProviderError::api_error(
                "github",
                500,
                format!("Failed to parse chat response: {}", e),
            )
        })
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        debug!("GitHub Models streaming request: model={}", request.model);

        // Get API configuration
        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| ProviderError::authentication("github", "API key is required"))?;

        // Create streaming request
        let mut stream_request = request.clone();
        stream_request.stream = true;

        // Execute streaming request using reqwest directly for SSE
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let client = crate::core::http::outbound::streaming_outbound_client().clone();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&stream_request)
            .send()
            .await
            .map_err(|e| ProviderError::network("github", e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            let body_str = body.unwrap_or_else(|| "Unknown error".to_string());
            return Err(match status {
                401 => ProviderError::authentication("github", "Invalid API key"),
                404 => ProviderError::model_not_found("github", body_str.clone()),
                429 => ProviderError::rate_limit("github", None),
                400 => ProviderError::invalid_request("github", body_str.clone()),
                500..=599 => ProviderError::provider_unavailable("github", body_str.clone()),
                _ => HttpErrorMapper::map_status_code("github", status, &body_str),
            });
        }

        // Create SSE stream
        let stream = GitHubStream::new(response.bytes_stream());
        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::not_supported(
            "github",
            "GitHub Models does not support embeddings endpoint directly. Use a specific embedding model.",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Simple health check - try to get models list
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
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        let model_info = get_model_info(model).ok_or_else(|| {
            ProviderError::model_not_found("github", format!("Unknown model: {}", model))
        })?;

        let input_cost = (input_tokens as f64) * (model_info.input_cost_per_million / 1_000_000.0);
        let output_cost =
            (output_tokens as f64) * (model_info.output_cost_per_million / 1_000_000.0);
        Ok(input_cost + output_cost)
    }
}

use crate::core::traits::error_mapper::trait_def::ErrorMapper;
/// SSE stream implementation for GitHub Models
use bytes::Bytes;

pub struct GitHubStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    buffer: String,
}

impl GitHubStream {
    pub fn new(stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static) -> Self {
        Self {
            inner: Box::pin(stream),
            buffer: String::new(),
        }
    }

    fn parse_sse_line(&self, line: &str) -> Option<Result<ChatChunk, ProviderError>> {
        if line.is_empty() || line.starts_with(':') {
            return None;
        }

        if let Some(data) = line.strip_prefix("data: ") {
            let data = data.trim();

            if data == "[DONE]" {
                return None;
            }

            match serde_json::from_str::<ChatChunk>(data) {
                Ok(chunk) => Some(Ok(chunk)),
                Err(e) => Some(Err(ProviderError::api_error(
                    "github",
                    500,
                    format!("Failed to parse chunk: {}", e),
                ))),
            }
        } else {
            None
        }
    }
}

impl Stream for GitHubStream {
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            // Check if we have complete lines in the buffer
            if let Some(newline_pos) = self.buffer.find('\n') {
                let line = self.buffer[..newline_pos].to_string();
                self.buffer = self.buffer[newline_pos + 1..].to_string();

                if let Some(result) = self.parse_sse_line(&line) {
                    return std::task::Poll::Ready(Some(result));
                }
                continue;
            }

            // Need more data
            match self.inner.as_mut().poll_next(cx) {
                std::task::Poll::Ready(Some(Ok(bytes))) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&bytes));
                }
                std::task::Poll::Ready(Some(Err(e))) => {
                    return std::task::Poll::Ready(Some(Err(ProviderError::network(
                        "github",
                        e.to_string(),
                    ))));
                }
                std::task::Poll::Ready(None) => {
                    // Stream ended, check remaining buffer
                    if !self.buffer.is_empty() {
                        let line = std::mem::take(&mut self.buffer);
                        if let Some(result) = self.parse_sse_line(&line) {
                            return std::task::Poll::Ready(Some(result));
                        }
                    }
                    return std::task::Poll::Ready(None);
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_github_provider_creation() {
        let config = GitHubConfig {
            api_key: Some("ghp_test123".to_string()),
            ..Default::default()
        };

        let provider = GitHubProvider::new(config).await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "github");
    }

    #[tokio::test]
    async fn test_github_provider_with_api_key() {
        let provider = GitHubProvider::with_api_key("ghp_test123").await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_github_provider_capabilities() {
        let provider = GitHubProvider::with_api_key("ghp_test123").await.unwrap();
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ToolCalling));
    }

    #[tokio::test]
    async fn test_github_provider_models() {
        let provider = GitHubProvider::with_api_key("ghp_test123").await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());

        // Check that we have expected models
        let model_ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert!(model_ids.contains(&"gpt-4o"));
        assert!(model_ids.contains(&"gpt-4o-mini"));
    }

    #[tokio::test]
    async fn test_github_provider_supported_params() {
        let provider = GitHubProvider::with_api_key("ghp_test123").await.unwrap();
        let params = provider.get_supported_openai_params("gpt-4o");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"stream"));
    }

    #[tokio::test]
    async fn test_github_provider_cost_calculation() {
        let provider = GitHubProvider::with_api_key("ghp_test123").await.unwrap();

        // Test with GPT-4o
        let cost = provider.calculate_cost("gpt-4o", 1000, 500).await;
        assert!(cost.is_ok());
        let cost = cost.unwrap();
        // 1000 input * $2.5/1M + 500 output * $10/1M = $0.0025 + $0.005 = $0.0075
        assert!((cost - 0.0075).abs() < 0.0001);

        // Test with free model (Meta Llama)
        let cost = provider
            .calculate_cost("meta-llama-3.1-70b-instruct", 1000, 500)
            .await;
        assert!(cost.is_ok());
        assert_eq!(cost.unwrap(), 0.0);
    }

    #[tokio::test]
    async fn test_github_provider_cost_unknown_model() {
        let provider = GitHubProvider::with_api_key("ghp_test123").await.unwrap();
        let cost = provider.calculate_cost("unknown-model", 1000, 500).await;
        assert!(cost.is_err());
    }
}
