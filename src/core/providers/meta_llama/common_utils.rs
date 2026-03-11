//! Common utilities for Meta Llama provider
//!
//! This module contains shared utilities, configuration, and client implementation
//! for the Meta Llama provider.

use futures::Stream;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, warn};

use super::LlamaProviderConfig;
use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::health::HealthStatus;
use crate::utils::net::http::create_custom_client;

/// Provider name constant
const PROVIDER_NAME: &str = "meta";

/// Llama error type alias (uses unified ProviderError)
pub type LlamaError = ProviderError;

/// Extension methods for creating Llama-specific errors
impl ProviderError {
    /// Create a not_supported error for Llama
    pub fn llama_not_supported(feature: &str) -> Self {
        ProviderError::invalid_request(
            PROVIDER_NAME,
            format!("Feature '{}' is not supported", feature),
        )
    }
}

/// Llama API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaConfig {
    /// API key for authentication
    pub api_key: String,
    /// API base URL
    pub api_base: String,
    /// Organization ID (optional)
    pub organization_id: Option<String>,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries
    pub max_retries: u32,
    /// Custom headers
    pub custom_headers: HashMap<String, String>,
    /// Enable debug logging
    pub debug: bool,
}

impl Default for LlamaConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.llama.com/compat/v1".to_string(),
            organization_id: None,
            timeout_seconds: 30,
            max_retries: 3,
            custom_headers: HashMap::new(),
            debug: false,
        }
    }
}

impl LlamaConfig {
    /// Create from provider config
    pub fn from_provider_config(config: &LlamaProviderConfig) -> Result<Self, ProviderError> {
        Ok(Self {
            api_key: config.api_key.clone(),
            api_base: config
                .api_base
                .clone()
                .unwrap_or_else(|| "https://api.llama.com/compat/v1".to_string()),
            organization_id: config.organization_id.clone(),
            timeout_seconds: config.timeout.unwrap_or(30),
            max_retries: config.max_retries.unwrap_or(3),
            custom_headers: config.headers.clone().unwrap_or_default(),
            debug: false,
        })
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.api_key.is_empty() {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "API key is required",
            ));
        }

        if self.api_base.is_empty() {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "API base URL is required",
            ));
        }

        if self.timeout_seconds == 0 {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "Timeout must be greater than 0",
            ));
        }

        Ok(())
    }
}

/// Llama API client
#[derive(Debug, Clone)]
pub struct LlamaClient {
    config: LlamaConfig,
    http_client: Client,
}

impl LlamaClient {
    /// Create a new Llama client
    pub fn new(config: LlamaConfig) -> Result<Self, ProviderError> {
        config.validate()?;

        let http_client = create_custom_client(Duration::from_secs(config.timeout_seconds))
            .map_err(|e| {
                ProviderError::configuration(
                    PROVIDER_NAME,
                    format!("Failed to create HTTP client: {}", e),
                )
            })?;

        Ok(Self {
            config,
            http_client,
        })
    }

    /// Build request headers
    fn build_headers(&self, api_key: Option<&str>) -> HashMap<String, String> {
        let mut headers = HashMap::new();

        // Add authorization header
        let key = api_key.unwrap_or(&self.config.api_key);
        headers.insert("Authorization".to_string(), format!("Bearer {}", key));

        // Add content type
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        // Add organization ID if present
        if let Some(org_id) = &self.config.organization_id {
            headers.insert("X-Organization-ID".to_string(), org_id.clone());
        }

        // Add custom headers
        headers.extend(self.config.custom_headers.clone());

        headers
    }

    /// Make a chat completion request
    pub async fn chat_completion(
        &self,
        request: Value,
        api_key: Option<&str>,
        api_base: Option<&str>,
        additional_headers: Option<HashMap<String, String>>,
    ) -> Result<Value, ProviderError> {
        let url = format!(
            "{}/chat/completions",
            api_base.unwrap_or(&self.config.api_base)
        );

        let mut headers = self.build_headers(api_key);
        if let Some(additional) = additional_headers {
            headers.extend(additional);
        }

        debug!("Making Llama API request to: {}", url);

        let mut request_builder = self.http_client.post(&url);
        for (key, value) in headers {
            request_builder = request_builder.header(key, value);
        }

        let response = request_builder.json(&request).send().await?;

        let status = response.status();
        let response_text = response.text().await?;

        if status.is_success() {
            serde_json::from_str(&response_text).map_err(|e| {
                ProviderError::serialization(
                    PROVIDER_NAME,
                    format!("Failed to parse response: {}", e),
                )
            })
        } else {
            self.handle_error_response(status, &response_text)
        }
    }

    /// Make a streaming chat completion request
    pub async fn chat_completion_stream(
        &self,
        request: Value,
        api_key: Option<&str>,
        api_base: Option<&str>,
        additional_headers: Option<HashMap<String, String>>,
    ) -> Result<impl Stream<Item = Result<Value, ProviderError>> + Send + 'static, ProviderError>
    {
        // For streaming, we'll return a simple implementation
        // In production, this would use SSE (Server-Sent Events) parsing
        use futures::stream;

        // Placeholder implementation
        let response = self
            .chat_completion(request, api_key, api_base, additional_headers)
            .await?;
        Ok(stream::once(async move { Ok(response) }))
    }

    /// Handle error responses
    fn handle_error_response(
        &self,
        status: StatusCode,
        response_text: &str,
    ) -> Result<Value, ProviderError> {
        let error_message = if let Ok(error_json) = serde_json::from_str::<Value>(response_text) {
            error_json
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or(response_text)
                .to_string()
        } else {
            response_text.to_string()
        };

        match status {
            StatusCode::UNAUTHORIZED => {
                Err(ProviderError::authentication(PROVIDER_NAME, error_message))
            }
            StatusCode::TOO_MANY_REQUESTS => {
                Err(ProviderError::rate_limit(PROVIDER_NAME, Some(60)))
            }
            StatusCode::BAD_REQUEST => {
                Err(ProviderError::invalid_request(PROVIDER_NAME, error_message))
            }
            StatusCode::NOT_FOUND => {
                Err(ProviderError::model_not_found(PROVIDER_NAME, error_message))
            }
            _ => Err(HttpErrorMapper::map_status_code(
                PROVIDER_NAME,
                status.as_u16(),
                &error_message,
            )),
        }
    }

    /// Check API health
    pub async fn check_health(&self) -> Result<HealthStatus, ProviderError> {
        // Try to list models as a health check
        let url = format!("{}/models", self.config.api_base);

        let headers = self.build_headers(None);

        let mut request_builder = self.http_client.get(&url);
        for (key, value) in headers {
            request_builder = request_builder.header(key, value);
        }

        match request_builder.send().await {
            Ok(response) if response.status().is_success() => Ok(HealthStatus::Healthy),
            Ok(response) => {
                warn!("Health check failed with status: {}", response.status());
                Ok(HealthStatus::Degraded)
            }
            Err(e) => {
                error!("Health check failed: {}", e);
                Ok(HealthStatus::Unhealthy)
            }
        }
    }
}

/// Utility functions for Llama provider
pub struct LlamaUtils;

impl LlamaUtils {
    /// Extract model name from request
    pub fn extract_model(request: &Value) -> Option<String> {
        request
            .get("model")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string())
    }

    /// Check if model supports vision
    pub fn is_vision_model(model: &str) -> bool {
        model.contains("vision")
    }

    /// Check if model supports function calling
    pub fn supports_function_calling(_model: &str) -> bool {
        // All Llama models support function calling through the OpenAI-compatible API
        true
    }

    /// Get default parameters for a model
    pub fn get_default_params(model: &str) -> HashMap<String, Value> {
        let mut params = HashMap::new();

        // Set reasonable defaults based on model
        if model.contains("405b") {
            params.insert("temperature".to_string(), serde_json::json!(0.7));
            params.insert("top_p".to_string(), serde_json::json!(0.9));
        } else if model.contains("70b") || model.contains("90b") {
            params.insert("temperature".to_string(), serde_json::json!(0.8));
            params.insert("top_p".to_string(), serde_json::json!(0.95));
        } else {
            params.insert("temperature".to_string(), serde_json::json!(0.9));
            params.insert("top_p".to_string(), serde_json::json!(1.0));
        }

        params
    }

    /// Validate request parameters
    pub fn validate_params(params: &Value) -> Result<(), ProviderError> {
        // Check temperature range
        if let Some(temp) = params.get("temperature").and_then(|t| t.as_f64())
            && !(0.0..=2.0).contains(&temp)
        {
            return Err(ProviderError::invalid_request(
                PROVIDER_NAME,
                format!("Temperature must be between 0 and 2, got {}", temp),
            ));
        }

        // Check top_p range
        if let Some(top_p) = params.get("top_p").and_then(|t| t.as_f64())
            && !(0.0..=1.0).contains(&top_p)
        {
            return Err(ProviderError::invalid_request(
                PROVIDER_NAME,
                format!("top_p must be between 0 and 1, got {}", top_p),
            ));
        }

        // Check max_tokens
        if let Some(max_tokens) = params.get("max_tokens").and_then(|t| t.as_i64())
            && max_tokens < 1
        {
            return Err(ProviderError::invalid_request(
                PROVIDER_NAME,
                format!("max_tokens must be positive, got {}", max_tokens),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = LlamaConfig::default();
        assert!(config.validate().is_err()); // No API key

        config.api_key = "test-key".to_string();
        assert!(config.validate().is_ok());

        config.timeout_seconds = 0;
        assert!(config.validate().is_err()); // Invalid timeout
    }

    #[test]
    fn test_model_detection() {
        assert!(LlamaUtils::is_vision_model("llama3.2-11b-vision"));
        assert!(!LlamaUtils::is_vision_model("llama3.1-8b"));

        assert!(LlamaUtils::supports_function_calling("llama3.1-70b"));
    }

    #[test]
    fn test_parameter_validation() {
        let valid_params = serde_json::json!({
            "temperature": 0.8,
            "top_p": 0.95,
            "max_tokens": 100
        });
        assert!(LlamaUtils::validate_params(&valid_params).is_ok());

        let invalid_temp = serde_json::json!({
            "temperature": 3.0
        });
        assert!(LlamaUtils::validate_params(&invalid_temp).is_err());

        let invalid_top_p = serde_json::json!({
            "top_p": 1.5
        });
        assert!(LlamaUtils::validate_params(&invalid_top_p).is_err());
    }

    #[test]
    fn test_error_types() {
        let err = ProviderError::authentication(PROVIDER_NAME, "bad key");
        assert!(matches!(err, ProviderError::Authentication { .. }));

        let err = ProviderError::rate_limit(PROVIDER_NAME, Some(60));
        assert!(matches!(err, ProviderError::RateLimit { .. }));

        let err = ProviderError::model_not_found(PROVIDER_NAME, "unknown");
        assert!(matches!(err, ProviderError::ModelNotFound { .. }));
    }
}
