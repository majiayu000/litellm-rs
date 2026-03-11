//! Shared HTTP client and canonical provider error mapping utilities for providers.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::Client;
use serde_json::Value;

use crate::core::providers::base::BaseConfig;
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::net::http::get_client_with_timeout_fallible;

/// Create a provider-scoped HTTP client with a configurable timeout.
pub fn create_http_client(
    provider: &'static str,
    timeout: Duration,
) -> Result<Client, ProviderError> {
    get_client_with_timeout_fallible(timeout)
        .map(|shared_client| (*shared_client).clone())
        .map_err(|e| {
            ProviderError::initialization(provider, format!("Failed to create HTTP client: {}", e))
        })
}

/// Base HTTP client wrapper used by provider implementations.
#[derive(Debug, Clone)]
pub struct BaseHttpClient {
    client: Client,
    config: BaseConfig,
}

impl BaseHttpClient {
    /// Create a new HTTP client with common configuration.
    pub fn new(config: BaseConfig) -> Result<Self, ProviderError> {
        let timeout = Duration::from_secs(config.timeout);
        let client = create_http_client("provider", timeout)?;
        Ok(Self { client, config })
    }

    /// Get the underlying reqwest client.
    pub fn inner(&self) -> &Client {
        &self.client
    }

    /// Get configuration.
    pub fn config(&self) -> &BaseConfig {
        &self.config
    }
}

/// Canonical HTTP status/body -> ProviderError mapper.
pub struct HttpErrorMapper;

impl HttpErrorMapper {
    /// Map HTTP status code to provider error.
    pub fn map_status_code(provider: &'static str, status: u16, body: &str) -> ProviderError {
        match status {
            400 => ProviderError::invalid_request(provider, body.to_string()),
            401 => ProviderError::authentication(
                provider,
                "Invalid API key or authentication failed".to_string(),
            ),
            403 => ProviderError::authentication(
                provider,
                "Forbidden: insufficient permissions".to_string(),
            ),
            404 => ProviderError::model_not_found(provider, body.to_string()),
            429 => ProviderError::rate_limit(provider, None),
            402 => ProviderError::quota_exceeded(provider, "Quota exceeded".to_string()),
            500..=599 => {
                ProviderError::api_error(provider, status, format!("Server error: {}", body))
            }
            _ => ProviderError::api_error(provider, status, body.to_string()),
        }
    }

    /// Parse JSON error response.
    pub fn parse_json_error(provider: &'static str, json: &Value) -> ProviderError {
        let message = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .or_else(|| json.get("message").and_then(|m| m.as_str()))
            .or_else(|| json.get("error").and_then(|e| e.as_str()))
            .unwrap_or("Unknown error");

        let error_type = json
            .get("error")
            .and_then(|e| e.get("type"))
            .and_then(|t| t.as_str())
            .or_else(|| json.get("type").and_then(|t| t.as_str()));

        match error_type {
            Some("invalid_request_error") => {
                ProviderError::invalid_request(provider, message.to_string())
            }
            Some("authentication_error") => {
                ProviderError::authentication(provider, message.to_string())
            }
            Some("rate_limit_error") => ProviderError::rate_limit(provider, None),
            Some("quota_exceeded") => ProviderError::quota_exceeded(provider, message.to_string()),
            _ => ProviderError::api_error(provider, 500, message.to_string()),
        }
    }
}

/// Common URL builder.
pub struct UrlBuilder {
    base: String,
    path: String,
    query_params: HashMap<String, String>,
}

impl UrlBuilder {
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            path: String::new(),
            query_params: HashMap::new(),
        }
    }

    pub fn with_path(mut self, path: &str) -> Self {
        self.path = path.trim_start_matches('/').to_string();
        self
    }

    pub fn with_query(mut self, key: &str, value: &str) -> Self {
        self.query_params.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_optional_query(mut self, key: &str, value: Option<&str>) -> Self {
        if let Some(v) = value {
            self.query_params.insert(key.to_string(), v.to_string());
        }
        self
    }

    pub fn build(self) -> String {
        let mut url = format!("{}/{}", self.base, self.path);

        if !self.query_params.is_empty() {
            let query_string: Vec<String> = self
                .query_params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v.replace(" ", "%20")))
                .collect();

            url.push('?');
            url.push_str(&query_string.join("&"));
        }

        url
    }
}

/// Common request transformer for OpenAI-compatible APIs.
pub struct OpenAIRequestTransformer;

impl OpenAIRequestTransformer {
    pub fn transform_chat_request(request: &crate::core::types::chat::ChatRequest) -> Value {
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
        });

        if let Some(temperature) = request.temperature {
            body["temperature"] = serde_json::json!(temperature);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(frequency_penalty) = request.frequency_penalty {
            body["frequency_penalty"] = serde_json::json!(frequency_penalty);
        }
        if let Some(presence_penalty) = request.presence_penalty {
            body["presence_penalty"] = serde_json::json!(presence_penalty);
        }
        if let Some(stop) = &request.stop {
            body["stop"] = serde_json::json!(stop);
        }

        body["stream"] = serde_json::json!(request.stream);

        if let Some(user) = &request.user {
            body["user"] = serde_json::json!(user);
        }
        if let Some(tools) = &request.tools {
            body["tools"] = serde_json::json!(tools);
        }
        if let Some(tool_choice) = &request.tool_choice {
            body["tool_choice"] = serde_json::json!(tool_choice);
        }
        if let Some(response_format) = &request.response_format {
            body["response_format"] = serde_json::json!(response_format);
        }
        if let Some(seed) = request.seed {
            body["seed"] = serde_json::json!(seed);
        }

        body
    }
}

/// Common chat request validation helper.
pub fn validate_chat_request_common(
    provider: &'static str,
    request: &crate::core::types::chat::ChatRequest,
    max_output_tokens: u32,
) -> Result<(), ProviderError> {
    if request.messages.is_empty() {
        return Err(ProviderError::invalid_request(
            provider,
            "Messages cannot be empty",
        ));
    }

    if let Some(max_tokens) = request.max_tokens
        && max_tokens > max_output_tokens
    {
        return Err(ProviderError::invalid_request(
            provider,
            format!(
                "max_tokens {} exceeds model limit of {}",
                max_tokens, max_output_tokens
            ),
        ));
    }

    Ok(())
}
