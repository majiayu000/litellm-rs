//! Base Provider Module
//!
//! Provides common functionality and patterns for all AI providers
//! to reduce code duplication and ensure consistency.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Duration;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::ProviderConfig;

/// Common provider configuration fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseProviderConfig {
    /// API key for authentication
    pub api_key: Option<String>,

    /// API base URL
    pub api_base: Option<String>,

    /// Request timeout in seconds
    pub timeout: Option<u64>,

    /// Maximum retry attempts
    pub max_retries: Option<u32>,

    /// Custom HTTP headers
    pub headers: Option<HashMap<String, String>>,

    /// Organization ID (if applicable)
    pub organization: Option<String>,

    /// API version
    pub api_version: Option<String>,
}

impl Default for BaseProviderConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base: None,
            timeout: Some(60),
            max_retries: Some(3),
            headers: None,
            organization: None,
            api_version: None,
        }
    }
}

impl BaseProviderConfig {}

/// Trait for unified provider configuration
pub trait UnifiedProviderConfig: ProviderConfig + Clone + Send + Sync {
    /// Get default API base URL
    fn default_api_base(&self) -> &'static str;

    /// Get default timeout in seconds
    fn default_timeout(&self) -> u64 {
        60
    }

    /// Get default max retries
    fn default_max_retries(&self) -> u32 {
        3
    }

    /// Get effective API key (from config)
    fn get_effective_api_key(&self) -> Option<String> {
        self.api_key().map(|s| s.to_string())
    }

    /// Get effective API base URL
    fn get_effective_api_base(&self) -> String {
        self.api_base()
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.default_api_base().to_string())
    }

    /// Common validation logic
    fn validate_common(&self) -> Result<(), String> {
        // Check API key
        if self.get_effective_api_key().is_none() {
            return Err("API key is required".to_string());
        }

        // Check timeout
        let timeout = self.timeout();
        if timeout.as_secs() == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }

        // Check max retries
        if self.max_retries() > 10 {
            return Err("Max retries should not exceed 10".to_string());
        }

        Ok(())
    }
}

/// Base HTTP client wrapper
#[derive(Debug, Clone)]
pub struct BaseHttpClient {
    client: Client,
    config: BaseProviderConfig,
}

impl BaseHttpClient {
    /// Create a new HTTP client with common configuration
    pub fn new(config: BaseProviderConfig) -> Result<Self, ProviderError> {
        let timeout = Duration::from_secs(config.timeout.unwrap_or(60));

        let client = crate::utils::net::http::get_client_with_timeout_fallible(timeout)
            .map(|shared_client| (*shared_client).clone())
            .map_err(|e| {
                ProviderError::invalid_request(
                    "http_client",
                    format!("Failed to create HTTP client: {}", e),
                )
            })?;

        Ok(Self { client, config })
    }

    /// Get the underlying reqwest client
    pub fn inner(&self) -> &Client {
        &self.client
    }

    /// Get configuration
    pub fn config(&self) -> &BaseProviderConfig {
        &self.config
    }
}

/// Common header builder
///
/// Uses `Cow<'static, str>` for header keys to avoid allocation for static header names
/// while still supporting dynamic header names when needed.
pub struct HeaderBuilder {
    headers: HashMap<Cow<'static, str>, String>,
}

impl Default for HeaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HeaderBuilder {
    /// Create new header builder
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
        }
    }

    /// Add authorization header (Bearer token)
    pub fn with_bearer_token(mut self, token: &str) -> Self {
        self.headers
            .insert(Cow::Borrowed("Authorization"), format!("Bearer {}", token));
        self
    }

    /// Add API key header with custom header name
    /// Note: header_name is owned because it may be dynamic
    pub fn with_api_key(mut self, key: &str, header_name: &str) -> Self {
        self.headers
            .insert(Cow::Owned(header_name.to_string()), key.to_string());
        self
    }

    /// Add content type
    pub fn with_content_type(mut self, content_type: &str) -> Self {
        self.headers
            .insert(Cow::Borrowed("Content-Type"), content_type.to_string());
        self
    }

    /// Add user agent
    pub fn with_user_agent(mut self, agent: &str) -> Self {
        self.headers
            .insert(Cow::Borrowed("User-Agent"), agent.to_string());
        self
    }

    /// Add custom headers
    pub fn with_custom_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers
            .extend(headers.into_iter().map(|(k, v)| (Cow::Owned(k), v)));
        self
    }

    /// Add organization ID (for providers that support it)
    pub fn with_organization(mut self, org_id: &str) -> Self {
        self.headers
            .insert(Cow::Borrowed("OpenAI-Organization"), org_id.to_string());
        self
    }

    /// Add a custom header with name and value
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers
            .insert(Cow::Owned(name.to_string()), value.to_string());
        self
    }

    /// Build the headers as HashMap<String, String> for compatibility
    pub fn build(self) -> HashMap<String, String> {
        self.headers
            .into_iter()
            .map(|(k, v)| (k.into_owned(), v))
            .collect()
    }

    /// Build as reqwest HeaderMap
    pub fn build_reqwest(self) -> Result<reqwest::header::HeaderMap, ProviderError> {
        let mut header_map = reqwest::header::HeaderMap::new();

        for (key, value) in self.headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    ProviderError::invalid_request(
                        "headers",
                        format!("Invalid header name '{}': {}", key, e),
                    )
                })?;

            let header_value = reqwest::header::HeaderValue::from_str(&value).map_err(|e| {
                ProviderError::invalid_request(
                    "headers",
                    format!("Invalid header value for '{}': {}", key, e),
                )
            })?;

            header_map.insert(header_name, header_value);
        }

        Ok(header_map)
    }
}

/// Common URL builder
pub struct UrlBuilder {
    base: String,
    path: String,
    query_params: HashMap<String, String>,
}

impl UrlBuilder {
    /// Create new URL builder
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            path: String::new(),
            query_params: HashMap::new(),
        }
    }

    /// Add path segment
    pub fn with_path(mut self, path: &str) -> Self {
        self.path = path.trim_start_matches('/').to_string();
        self
    }

    /// Add query parameter
    pub fn with_query(mut self, key: &str, value: &str) -> Self {
        self.query_params.insert(key.to_string(), value.to_string());
        self
    }

    /// Add optional query parameter
    pub fn with_optional_query(mut self, key: &str, value: Option<&str>) -> Self {
        if let Some(v) = value {
            self.query_params.insert(key.to_string(), v.to_string());
        }
        self
    }

    /// Build the URL
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

/// Common request transformer for OpenAI-compatible APIs
pub struct OpenAIRequestTransformer;

impl OpenAIRequestTransformer {
    /// Transform standard chat request to OpenAI format
    pub fn transform_chat_request(request: &crate::core::types::chat::ChatRequest) -> Value {
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
        });

        // Add optional parameters
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

    /// Transform standard parameters map to provider format
    pub fn transform_parameters(params: &HashMap<String, Value>) -> HashMap<String, Value> {
        let mut transformed = HashMap::new();

        // Direct pass-through for most parameters
        for (key, value) in params {
            match key.as_str() {
                // Rename parameters if needed
                "max_length" => transformed.insert("max_tokens".to_string(), value.clone()),
                "stop_sequences" => transformed.insert("stop".to_string(), value.clone()),
                // Pass through standard parameters
                _ => transformed.insert(key.clone(), value.clone()),
            };
        }

        transformed
    }
}

/// Common HTTP error mapper
pub struct HttpErrorMapper;

impl HttpErrorMapper {
    /// Map HTTP status code to provider error
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

    /// Parse JSON error response
    pub fn parse_json_error(provider: &'static str, json: &Value) -> ProviderError {
        // Try common error formats
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

/// Common chat request validation helper.
///
/// Validates the common parts of a chat request that most providers share:
/// - Messages must not be empty
/// - max_tokens must not exceed the model's output token limit
///
/// Providers should call this after their own registry lookup, passing the
/// resolved `max_output_tokens` from their model spec.
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

    if let Some(max_tokens) = request.max_tokens {
        if max_tokens > max_output_tokens {
            return Err(ProviderError::invalid_request(
                provider,
                format!(
                    "max_tokens {} exceeds model limit of {}",
                    max_tokens, max_output_tokens
                ),
            ));
        }
    }

    Ok(())
}

/// Cost calculation utilities
pub struct CostCalculator;

impl CostCalculator {
    /// Calculate cost based on token usage
    pub fn calculate(
        input_tokens: u32,
        output_tokens: u32,
        input_cost_per_1k: f64,
        output_cost_per_1k: f64,
    ) -> f64 {
        let input_cost = (input_tokens as f64 / 1000.0) * input_cost_per_1k;
        let output_cost = (output_tokens as f64 / 1000.0) * output_cost_per_1k;
        input_cost + output_cost
    }

    /// Get model pricing (per 1K tokens)
    pub fn get_model_pricing(provider: &str, model: &str) -> Option<(f64, f64)> {
        match (provider, model) {
            // OpenAI models
            ("openai", "gpt-4") => Some((0.03, 0.06)),
            ("openai", "gpt-4-turbo") => Some((0.01, 0.03)),
            ("openai", "gpt-3.5-turbo") => Some((0.0005, 0.0015)),

            // Anthropic models
            ("anthropic", "claude-3-opus") => Some((0.015, 0.075)),
            ("anthropic", "claude-3-sonnet") => Some((0.003, 0.015)),
            ("anthropic", "claude-3-haiku") => Some((0.00025, 0.00125)),

            // Add more models as needed...
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_builder() {
        let headers = HeaderBuilder::new()
            .with_bearer_token("test-token")
            .with_content_type("application/json")
            .with_user_agent("test-agent")
            .build();

        assert_eq!(headers.get("Authorization").unwrap(), "Bearer test-token");
        assert_eq!(headers.get("Content-Type").unwrap(), "application/json");
        assert_eq!(headers.get("User-Agent").unwrap(), "test-agent");
    }

    #[test]
    fn test_url_builder() {
        let url = UrlBuilder::new("https://api.example.com")
            .with_path("v1/chat/completions")
            .with_query("api-version", "2024-01-01")
            .with_optional_query("deployment", Some("gpt-4"))
            .build();

        // Parse the URL to check components since query parameter order may vary
        let parsed_url = url::Url::parse(&url).expect("Invalid URL");
        assert_eq!(parsed_url.scheme(), "https");
        assert_eq!(parsed_url.host_str(), Some("api.example.com"));
        assert_eq!(parsed_url.path(), "/v1/chat/completions");

        // Check query parameters exist (order doesn't matter)
        let query_pairs: std::collections::HashMap<_, _> = parsed_url.query_pairs().collect();
        assert_eq!(
            query_pairs.get("api-version"),
            Some(&std::borrow::Cow::Borrowed("2024-01-01"))
        );
        assert_eq!(
            query_pairs.get("deployment"),
            Some(&std::borrow::Cow::Borrowed("gpt-4"))
        );
    }

    #[test]
    fn test_cost_calculator() {
        let cost = CostCalculator::calculate(1000, 500, 0.01, 0.02);
        assert_eq!(cost, 0.02); // 1000/1000 * 0.01 + 500/1000 * 0.02
    }
}
