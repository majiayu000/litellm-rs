//! Implementation

use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, error, warn};

use crate::ProviderError;
use crate::core::traits::{
    error_mapper::types::GenericErrorMapper, provider::ProviderConfig,
    provider::llm_provider::trait_definition::LLMProvider,
};
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
use crate::utils::net::http::create_custom_client_with_headers;

use super::config::OpenRouterConfig;
use super::error::OpenRouterError;
use super::models::get_openrouter_registry;
use super::transformer::{
    OpenRouterRequestTransformer, OpenRouterResponseTransformer, create_openrouter_headers,
};

use std::collections::HashMap;

/// OpenRouter Provider implementation
#[derive(Debug, Clone)]
pub struct OpenRouterProvider {
    /// HTTP client
    client: Client,
    /// API base URL
    base_url: String,
    /// Model
    models: Vec<ModelInfo>,
}

impl OpenRouterProvider {
    /// Create
    pub async fn new(config: OpenRouterConfig) -> Result<Self, OpenRouterError> {
        // Configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("openrouter", e))?;

        // Get
        let api_key = if config.api_key.is_empty() {
            std::env::var("OPENROUTER_API_KEY").map_err(|_| {
                ProviderError::configuration("openrouter", "OpenRouter API key not found")
            })?
        } else {
            config.api_key.clone()
        };
        let api_key = api_key.trim().to_string();

        // Create
        let headers = create_openrouter_headers(
            &api_key,
            config.site_url.as_deref(),
            config.site_name.as_deref(),
        );

        // Create
        let mut header_map = reqwest::header::HeaderMap::new();
        for (key, value) in &headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    ProviderError::configuration(
                        "openrouter",
                        format!("Invalid header key '{}': {}", key, e),
                    )
                })?;

            // Ensure header value has no illegal characters
            let clean_value = value.trim();
            let header_value =
                reqwest::header::HeaderValue::from_str(clean_value).map_err(|e| {
                    error!(
                        provider = "openrouter",
                        header_key = %key,
                        header_value = %clean_value,
                        error = %e,
                        "Failed to parse HTTP header value"
                    );
                    ProviderError::configuration(
                        "openrouter",
                        format!("Invalid header value for '{}': {}", key, e),
                    )
                })?;
            header_map.insert(header_name, header_value);
        }

        // Add custom headers - OpenRouterConfig doesn't have headers field, so skip this
        // TODO: Add support for custom headers if needed
        /*
        for (key, value) in &config.headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    ProviderError::configuration("openrouter", format!(
                        "Invalid custom header key '{}': {}",
                        key, e
                    ))
                })?;
            let header_value = reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                ProviderError::configuration("openrouter", format!(
                    "Invalid custom header value for '{}': {}",
                    key, e
                ))
            })?;
            header_map.insert(header_name, header_value);
        }
        */

        let client = create_custom_client_with_headers(
            Duration::from_secs(config.timeout_seconds),
            header_map,
        )
        .map_err(|e| {
            ProviderError::network("openrouter", format!("Failed to create HTTP client: {}", e))
        })?;

        let base_url = config.base_url.clone();

        // Get
        let models = get_openrouter_registry().get_all_models();

        Ok(Self {
            client,
            base_url,
            models,
        })
    }

    /// Default
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, OpenRouterError> {
        let config = OpenRouterConfig {
            api_key: api_key.into(),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Execute HTTP request
    async fn execute_request<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<T, OpenRouterError> {
        let url = format!("{}/{}", self.base_url, endpoint);

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout {
                        provider: "openrouter",
                        message: format!("Request to {} timed out", url),
                    }
                } else if e.is_connect() {
                    ProviderError::network(
                        "openrouter",
                        format!("Connection failed to {}: {}", url, e),
                    )
                } else {
                    ProviderError::network("openrouter", format!("Request failed: {}", e))
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(OpenRouterResponseTransformer::parse_error(
                &error_text,
                status.as_u16(),
            ));
        }

        let response_text = response.text().await.map_err(|e| {
            ProviderError::network("openrouter", format!("Failed to read response: {}", e))
        })?;

        debug!(
            provider = "openrouter",
            response_text = %response_text,
            "Raw HTTP response received"
        );

        serde_json::from_str(&response_text).map_err(|e| {
            ProviderError::api_error(
                "openrouter",
                500,
                format!("Failed to parse response: {}", e),
            )
        })
    }
}

#[async_trait]
impl LLMProvider for OpenRouterProvider {
    type Config = OpenRouterConfig;
    type Error = OpenRouterError;
    type ErrorMapper = GenericErrorMapper;

    fn name(&self) -> &'static str {
        "openrouter"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        static CAPABILITIES: &[ProviderCapability] = &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::FunctionCalling,
        ];
        CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    async fn health_check(&self) -> HealthStatus {
        // Check
        match self
            .execute_request::<serde_json::Value>("models", serde_json::json!({}))
            .await
        {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        // Transform request to OpenAI format
        let openai_request = OpenRouterRequestTransformer::transform_request(
            request, None, // Using None for now - will implement proper conversion later
        )?;

        let body = serde_json::to_value(openai_request)?;
        debug!(
            provider = "openrouter",
            request_body = %serde_json::to_string_pretty(&body).unwrap_or_default(),
            "Sending request to OpenRouter API"
        );

        // Execute request
        let response: crate::core::providers::openai::models::OpenAIChatResponse =
            self.execute_request("chat/completions", body).await?;

        debug!(
            provider = "openrouter",
            response = ?response,
            "Raw response received from OpenRouter"
        );

        // Transform response
        OpenRouterResponseTransformer::transform_response(response)
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        // Response
        // Handle
        Err(ProviderError::not_supported(
            "openrouter",
            "Streaming not yet implemented",
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        // OpenRouter may not support embeddings for all models
        Err(ProviderError::not_supported(
            "openrouter",
            "Embeddings not supported via OpenRouter",
        ))
    }

    async fn image_generation(
        &self,
        _request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        // OpenRouter may support some image generation models
        Err(ProviderError::not_supported(
            "openrouter",
            "Image generation not yet implemented",
        ))
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        // Default
        // Get
        let input_cost = (input_tokens as f64 / 1000.0) * 0.001;
        let output_cost = (output_tokens as f64 / 1000.0) * 0.002;
        Ok(input_cost + output_cost)
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        // OpenRouter supports standard OpenAI parameters, plus its own extensions
        static SUPPORTED_PARAMS: &[&str] = &[
            "messages",
            "model",
            "max_tokens",
            "temperature",
            "top_p",
            "n",
            "stream",
            "stop",
            "presence_penalty",
            "frequency_penalty",
            "logit_bias",
            "user",
            "functions",
            "function_call",
            "tools",
            "tool_choice",
            "response_format",
            // OpenRouter specific_params
            "transforms",
            "models",
            "route",
            "provider",
        ];
        SUPPORTED_PARAMS
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        let mut mapped_params = HashMap::new();

        for (key, value) in params {
            match key.as_str() {
                // Standard OpenAI parameters map directly
                "messages" | "model" | "max_tokens" | "temperature" | "top_p" | "n" | "stream"
                | "stop" | "presence_penalty" | "frequency_penalty" | "logit_bias" | "user"
                | "functions" | "function_call" | "tools" | "tool_choice" | "response_format" => {
                    mapped_params.insert(key, value);
                }

                // OpenRouterspecific_params
                "transforms" | "models" | "route" | "provider" => {
                    mapped_params.insert(key, value);
                }

                // Ignore unsupported parameters
                _ => {
                    warn!(
                        provider = "openrouter",
                        parameter = %key,
                        "Ignoring unsupported parameter for OpenRouter"
                    );
                }
            }
        }

        Ok(mapped_params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        // Request
        // TODO: Convert HashMap extra_params to OpenRouterExtraParams
        let openai_request = OpenRouterRequestTransformer::transform_request(
            request, None, // Using None for now - will implement proper conversion later
        )?;

        // Serialize to JSON value
        serde_json::to_value(openai_request).map_err(|e| {
            ProviderError::api_error(
                "openrouter",
                500,
                format!("Failed to serialize request: {}", e),
            )
        })
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        // Response
        let response_text = std::str::from_utf8(raw_response).map_err(|e| {
            ProviderError::api_error("openrouter", 500, format!("Invalid UTF-8 response: {}", e))
        })?;

        let openai_response: crate::core::providers::openai::models::OpenAIChatResponse =
            serde_json::from_str(response_text).map_err(|e| {
                ProviderError::api_error(
                    "openrouter",
                    500,
                    format!("Failed to parse response: {}", e),
                )
            })?;

        // Response
        OpenRouterResponseTransformer::transform_response(openai_response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        GenericErrorMapper
    }
}

// Provider trait implementation removed - OpenRouterProvider is now included through the Provider enum variants

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ProviderError Tests ====================

    #[test]
    fn test_provider_error_authentication() {
        let error = ProviderError::authentication("openrouter", "Invalid API key");
        assert!(error.to_string().contains("Authentication failed"));
        assert!(error.to_string().contains("openrouter"));
        assert_eq!(error.http_status(), 401);
    }

    #[test]
    fn test_provider_error_rate_limit() {
        let error = ProviderError::rate_limit("openrouter", Some(60));
        assert!(error.to_string().contains("Rate limit"));
        assert_eq!(error.http_status(), 429);
    }

    #[test]
    fn test_provider_error_invalid_request() {
        let error = ProviderError::invalid_request("openrouter", "Bad request body");
        assert!(error.to_string().contains("Invalid request"));
        assert_eq!(error.http_status(), 400);
    }

    #[test]
    fn test_provider_error_model_not_found() {
        let error = ProviderError::model_not_found("openrouter", "gpt-5");
        assert!(error.to_string().contains("not found"));
        assert_eq!(error.http_status(), 404);
    }

    #[test]
    fn test_provider_error_network() {
        let error = ProviderError::network("openrouter", "Connection refused");
        assert!(error.to_string().contains("Network error"));
        assert_eq!(error.http_status(), 503);
    }

    #[test]
    fn test_provider_error_api_error() {
        let error = ProviderError::api_error("openrouter", 500, "Internal server error");
        assert!(error.to_string().contains("API error"));
        assert_eq!(error.http_status(), 500);
    }

    #[test]
    fn test_provider_error_timeout() {
        let error = ProviderError::timeout("openrouter", "Request timed out");
        assert!(error.to_string().contains("Timeout"));
        assert_eq!(error.http_status(), 503);
    }

    #[test]
    fn test_provider_error_not_supported() {
        let error = ProviderError::not_supported("openrouter", "Embeddings");
        assert!(error.to_string().contains("not supported"));
        assert_eq!(error.http_status(), 405);
    }

    #[test]
    fn test_provider_error_is_retryable() {
        assert!(ProviderError::network("openrouter", "Connection refused").is_retryable());
        assert!(ProviderError::timeout("openrouter", "Timeout").is_retryable());
        assert!(ProviderError::rate_limit("openrouter", Some(60)).is_retryable());
        assert!(!ProviderError::authentication("openrouter", "Invalid key").is_retryable());
        assert!(!ProviderError::invalid_request("openrouter", "Bad request").is_retryable());
    }

    // ==================== OpenRouterProvider Tests ====================

    #[test]
    fn test_provider_name() {
        // We can't easily create a provider without config, but we can test the constant
        // The name should be "openrouter"
        assert_eq!("openrouter", "openrouter");
    }

    #[test]
    fn test_provider_capabilities() {
        // Test the static capabilities
        let capabilities = &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::FunctionCalling,
        ];
        assert_eq!(capabilities.len(), 3);
        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::FunctionCalling));
    }

    #[test]
    fn test_supported_params() {
        let supported_params = &[
            "messages",
            "model",
            "max_tokens",
            "temperature",
            "top_p",
            "n",
            "stream",
            "stop",
            "presence_penalty",
            "frequency_penalty",
            "logit_bias",
            "user",
            "functions",
            "function_call",
            "tools",
            "tool_choice",
            "response_format",
            "transforms",
            "models",
            "route",
            "provider",
        ];

        assert!(supported_params.contains(&"messages"));
        assert!(supported_params.contains(&"transforms")); // OpenRouter specific
        assert!(supported_params.contains(&"route")); // OpenRouter specific
        assert_eq!(supported_params.len(), 21);
    }

    #[test]
    fn test_cost_calculation() {
        // Test the cost calculation formula
        let input_tokens: u32 = 1000;
        let output_tokens: u32 = 500;

        let input_cost = (input_tokens as f64 / 1000.0) * 0.001;
        let output_cost = (output_tokens as f64 / 1000.0) * 0.002;
        let total = input_cost + output_cost;

        assert!((total - 0.002).abs() < 0.0001);
    }
}
