//! OpenAI-Like Provider Implementation
//!
//! Main provider implementation for any OpenAI-compatible API endpoint

use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpMethod, apply_headers, header, header_owned,
};
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse},
};

use super::{
    config::OpenAILikeConfig,
    error::{OpenAILikeError, PROVIDER_NAME},
    models::{OpenAILikeModelRegistry, get_openai_like_registry},
};
use crate::core::providers::unified_provider::ProviderError;

/// OpenAI-Like Provider implementation
///
/// Connects to any OpenAI-compatible API endpoint.
#[derive(Debug, Clone)]
pub struct OpenAILikeProvider {
    /// Connection pool manager
    pool_manager: Arc<GlobalPoolManager>,
    /// Provider configuration
    config: OpenAILikeConfig,
    /// Model registry
    model_registry: &'static OpenAILikeModelRegistry,
    /// Interned provider name for `&'static str` return in `name()`
    provider_name: &'static str,
}

/// Intern a provider name as `&'static str`.
///
/// Returns the pre-existing constant for the default name to avoid allocation,
/// and leaks the string for custom names. Providers are long-lived singletons,
/// so the small allocation is acceptable.
fn intern_provider_name(name: &str) -> &'static str {
    if name == PROVIDER_NAME {
        return PROVIDER_NAME;
    }
    Box::leak(name.to_string().into_boxed_str())
}

impl OpenAILikeProvider {
    /// Create a new OpenAI-like provider
    pub async fn new(config: OpenAILikeConfig) -> Result<Self, OpenAILikeError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| OpenAILikeError::configuration(PROVIDER_NAME, e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| OpenAILikeError::network(PROVIDER_NAME, e.to_string()))?,
        );
        let model_registry = get_openai_like_registry();
        let provider_name = intern_provider_name(&config.provider_name);

        Ok(Self {
            pool_manager,
            config,
            model_registry,
            provider_name,
        })
    }

    /// Create provider with just an API base URL (no API key required)
    pub async fn with_api_base(api_base: impl Into<String>) -> Result<Self, OpenAILikeError> {
        let config = OpenAILikeConfig::new(api_base).with_skip_api_key(true);
        Self::new(config).await
    }

    /// Create provider with API base and key
    pub async fn with_api_key(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, OpenAILikeError> {
        let config = OpenAILikeConfig::with_api_key(api_base, api_key);
        Self::new(config).await
    }

    /// Generate headers for API requests
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(4 + self.config.custom_headers.len());

        // Add Authorization header if API key is provided
        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        // Add organization header if present
        if let Some(org) = &self.config.base.organization {
            headers.push(header("OpenAI-Organization", org.clone()));
        }

        // Add base headers
        for (key, value) in &self.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        // Add custom headers
        for (key, value) in &self.config.custom_headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        // Add OpenRouter-specific headers when OR_SITE_URL / OR_APP_NAME are set
        if self.config.provider_name == "openrouter" {
            if let Ok(site_url) = std::env::var("OR_SITE_URL") {
                headers.push(header_owned("HTTP-Referer".to_string(), site_url));
            }
            if let Ok(app_name) = std::env::var("OR_APP_NAME") {
                headers.push(header_owned("X-Title".to_string(), app_name));
            }
        }

        headers
    }

    /// Execute chat completion request
    async fn execute_chat_completion(
        &self,
        request: ChatRequest,
    ) -> Result<ChatResponse, OpenAILikeError> {
        // Transform request to OpenAI format
        let openai_request = self.transform_chat_request(request)?;

        // Execute HTTP request
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let headers = self.get_request_headers();
        let body = Some(openai_request);

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body)
            .await
            .map_err(|e| OpenAILikeError::network(PROVIDER_NAME, e.to_string()))?;

        // Check for error status codes
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(self.map_error_response(status.as_u16(), &body));
        }

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| OpenAILikeError::network(PROVIDER_NAME, e.to_string()))?;

        let response_json: Value = serde_json::from_slice(&response_bytes)
            .map_err(|e| OpenAILikeError::response_parsing(PROVIDER_NAME, e.to_string()))?;

        // Transform response back to standard format
        self.transform_chat_response(response_json)
    }

    /// Execute streaming chat completion
    async fn execute_chat_completion_stream(
        &self,
        request: ChatRequest,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<ChatChunk, OpenAILikeError>> + Send>>,
        OpenAILikeError,
    > {
        // Transform request with streaming enabled
        let mut openai_request = self.transform_chat_request(request)?;
        openai_request["stream"] = Value::Bool(true);

        // Execute streaming request via pool_manager's client
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let client = self.pool_manager.client();
        let headers = self.get_request_headers();
        let req = apply_headers(client.post(&url).json(&openai_request), headers);

        let response = req
            .send()
            .await
            .map_err(|e| OpenAILikeError::network(PROVIDER_NAME, e.to_string()))?;

        // Check for error status codes
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(self.map_error_response(status.as_u16(), &body));
        }

        // Create stream handler using unified SSE parser
        let stream = response.bytes_stream();
        Ok(Box::pin(super::streaming::create_openai_like_stream(
            stream,
        )))
    }

    /// Transform ChatRequest to OpenAI API format
    fn transform_chat_request(&self, request: ChatRequest) -> Result<Value, OpenAILikeError> {
        // Get effective model name (strip prefix if configured)
        let model = self.config.get_effective_model(&request.model);

        let mut openai_request = serde_json::json!({
            "model": model,
            "messages": request.messages
        });

        // Add optional parameters
        if let Some(temp) = request.temperature {
            openai_request["temperature"] = serde_json::json!(temp);
        }

        if let Some(max_tokens) = request.max_tokens {
            openai_request["max_tokens"] = Value::Number(serde_json::Number::from(max_tokens));
        }

        if let Some(max_completion_tokens) = request.max_completion_tokens {
            openai_request["max_completion_tokens"] =
                Value::Number(serde_json::Number::from(max_completion_tokens));
        }

        if let Some(top_p) = request.top_p {
            openai_request["top_p"] = serde_json::json!(top_p);
        }

        if let Some(tools) = request.tools {
            openai_request["tools"] = serde_json::to_value(tools)
                .map_err(|e| OpenAILikeError::serialization(PROVIDER_NAME, e.to_string()))?;
        }

        if let Some(tool_choice) = request.tool_choice {
            openai_request["tool_choice"] = serde_json::to_value(tool_choice)
                .map_err(|e| OpenAILikeError::serialization(PROVIDER_NAME, e.to_string()))?;
        }

        if let Some(response_format) = request.response_format {
            openai_request["response_format"] = serde_json::to_value(response_format)
                .map_err(|e| OpenAILikeError::serialization(PROVIDER_NAME, e.to_string()))?;
        }

        if let Some(stop) = request.stop {
            openai_request["stop"] = serde_json::to_value(stop)
                .map_err(|e| OpenAILikeError::serialization(PROVIDER_NAME, e.to_string()))?;
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
            openai_request["stream_options"] = serde_json::to_value(stream_options)
                .map_err(|e| OpenAILikeError::serialization(PROVIDER_NAME, e.to_string()))?;
        }

        // Wire OpenRouter reasoning parameter from ThinkingConfig before extra_params consumes request
        let openrouter_thinking_params = if self.config.provider_name == "openrouter" {
            if let Some(thinking_config) = &request.thinking {
                let params =
                    crate::core::providers::thinking::openrouter_thinking::transform_config(
                        thinking_config,
                        &model,
                    )
                    .map_err(|e| OpenAILikeError::serialization(PROVIDER_NAME, e.to_string()))?;
                Some(params)
            } else {
                None
            }
        } else {
            None
        };

        // Forward extra_params (e.g. OpenRouter-specific params, frequency_penalty, etc.)
        if let Some(obj) = openai_request.as_object_mut() {
            for (key, value) in request.extra_params {
                obj.insert(key, value);
            }
            // Merge OpenRouter reasoning params (reasoning.effort etc.)
            if let Some(Value::Object(params)) = openrouter_thinking_params {
                for (key, value) in params {
                    obj.insert(key, value);
                }
            }
        }

        Ok(openai_request)
    }

    /// Transform OpenAI response to standard format
    fn transform_chat_response(&self, response: Value) -> Result<ChatResponse, OpenAILikeError> {
        // Directly deserialize to ChatResponse since it's OpenAI-compatible
        serde_json::from_value(response)
            .map_err(|e| OpenAILikeError::response_parsing(PROVIDER_NAME, e.to_string()))
    }

    /// Map HTTP error response to OpenAILikeError
    fn map_error_response(&self, status: u16, body: &str) -> OpenAILikeError {
        // Try to parse error JSON
        if let Ok(error_json) = serde_json::from_str::<Value>(body)
            && let Some(error) = error_json.get("error")
        {
            let error_type = error.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let error_code = error.get("code").and_then(|c| c.as_str()).unwrap_or("");
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");

            return match (status, error_type, error_code) {
                (401, _, _) | (_, "authentication_error", _) => {
                    OpenAILikeError::openai_like_authentication(message)
                }
                (429, _, _) | (_, "rate_limit_error", _) => {
                    let retry_after = error.get("retry_after").and_then(|r| r.as_u64());
                    OpenAILikeError::openai_like_rate_limit(retry_after)
                }
                (404, _, "model_not_found") => {
                    OpenAILikeError::openai_like_model_not_found(message)
                }
                (400, "invalid_request_error", _) => {
                    OpenAILikeError::openai_like_invalid_request(message)
                }
                (503, _, _) | (_, "overloaded_error", _) => {
                    OpenAILikeError::openai_like_unavailable(message)
                }
                _ => OpenAILikeError::openai_like_api_error(status, message),
            };
        }

        // Fallback to status-based error
        match status {
            401 => OpenAILikeError::openai_like_authentication("Authentication failed"),
            429 => OpenAILikeError::openai_like_rate_limit(None),
            404 => OpenAILikeError::openai_like_model_not_found("Resource not found"),
            500..=599 => {
                OpenAILikeError::openai_like_unavailable(format!("Server error: {}", status))
            }
            _ => OpenAILikeError::openai_like_api_error(status, body.to_string()),
        }
    }

    /// Get model information
    pub fn get_model_info(&self, model_id: &str) -> ModelInfo {
        self.model_registry.get_model_info(model_id)
    }

    /// Get the provider configuration
    pub fn config(&self) -> &OpenAILikeConfig {
        &self.config
    }
}

/// Error mapper for OpenAI-like provider
pub struct OpenAILikeErrorMapper;

impl<E> crate::core::traits::error_mapper::trait_def::ErrorMapper<E> for OpenAILikeErrorMapper
where
    E: crate::core::types::errors::ProviderErrorTrait,
{
    fn map_http_error(&self, status_code: u16, response_body: &str) -> E {
        // Try to parse JSON response first
        if let Ok(error_json) = serde_json::from_str::<Value>(response_body) {
            return self.map_json_error(&error_json);
        }

        // Fallback to status-based mapping
        match status_code {
            401 => E::authentication_failed("Authentication failed"),
            429 => E::rate_limited(None),
            404 => E::not_supported("Resource not found"),
            _ => E::network_error(&format!("HTTP error {}: {}", status_code, response_body)),
        }
    }

    fn map_json_error(&self, error_response: &Value) -> E {
        if let Some(error) = error_response.get("error") {
            let error_type = error.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");

            match error_type {
                "authentication_error" => E::authentication_failed(message),
                "rate_limit_error" => {
                    let retry_after = error.get("retry_after").and_then(|r| r.as_u64());
                    E::rate_limited(retry_after)
                }
                "invalid_request_error" => E::network_error(message),
                _ => E::network_error(&format!("API Error: {}", message)),
            }
        } else {
            E::network_error("Invalid error response format")
        }
    }
}

impl LLMProvider for OpenAILikeProvider {
    fn name(&self) -> &'static str {
        self.provider_name
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        static CAPABILITIES: &[ProviderCapability] = &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ToolCalling,
            ProviderCapability::FunctionCalling,
        ];
        CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        // Return empty slice - any model is supported dynamically
        static MODELS: &[ModelInfo] = &[];
        MODELS
    }

    fn supports_model(&self, _model: &str) -> bool {
        // Accept any model name
        true
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        self.execute_chat_completion(request).await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        self.execute_chat_completion_stream(request).await
    }

    async fn health_check(&self) -> HealthStatus {
        // Try to connect to the API base via pool_manager's client
        let url = format!("{}/models", self.config.get_api_base());
        let client = self.pool_manager.client();
        let headers = self.get_request_headers();
        let req = apply_headers(client.get(&url), headers);

        match req.send().await {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::Degraded,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        let model_info = self.get_model_info(model);

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

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        // Support all common OpenAI parameters
        &[
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
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        // Pass through all params without modification
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        self.transform_chat_request(request)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let response_value: Value = serde_json::from_slice(raw_response)
            .map_err(|e| OpenAILikeError::response_parsing(PROVIDER_NAME, e.to_string()))?;
        self.transform_chat_response(response_value)
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(OpenAILikeErrorMapper)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_creation_with_api_base() {
        let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1").await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "openai_like");
    }

    #[tokio::test]
    async fn test_provider_creation_with_api_key() {
        let provider =
            OpenAILikeProvider::with_api_key("http://localhost:8000/v1", "sk-test123").await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_provider_supports_any_model() {
        let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1")
            .await
            .unwrap();

        assert!(provider.supports_model("gpt-4"));
        assert!(provider.supports_model("llama-2-70b"));
        assert!(provider.supports_model("any-custom-model"));
        assert!(provider.supports_model("custom/my-model"));
    }

    #[tokio::test]
    async fn test_model_info_for_any_model() {
        let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1")
            .await
            .unwrap();

        let info = provider.get_model_info("my-custom-model");
        assert_eq!(info.id, "my-custom-model");
        assert_eq!(info.provider, "openai_like");
        assert!(info.supports_streaming);
    }

    #[tokio::test]
    async fn test_request_transformation() {
        let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1")
            .await
            .unwrap();

        let request = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![],
            temperature: Some(0.7),
            max_tokens: Some(100),
            ..Default::default()
        };

        let transformed = provider.transform_chat_request(request);
        assert!(transformed.is_ok());

        let json = transformed.unwrap();
        assert_eq!(json["model"], "test-model");
        assert!((json["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
        assert_eq!(json["max_tokens"], 100);
    }

    #[tokio::test]
    async fn test_model_prefix_stripping() {
        let config = OpenAILikeConfig::new("http://localhost:8000/v1")
            .with_model_prefix("custom/")
            .with_skip_api_key(true);

        let provider = OpenAILikeProvider::new(config).await.unwrap();

        let request = ChatRequest {
            model: "custom/gpt-4".to_string(),
            messages: vec![],
            ..Default::default()
        };

        let transformed = provider.transform_chat_request(request).unwrap();
        assert_eq!(transformed["model"], "gpt-4");
    }

    #[test]
    fn test_error_mapping() {
        let provider_name = PROVIDER_NAME;

        let err = OpenAILikeError::openai_like_authentication("Invalid API key");
        assert_eq!(err.provider(), provider_name);

        let err = OpenAILikeError::openai_like_rate_limit(Some(60));
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(60));
    }

    #[tokio::test]
    async fn test_name_returns_default_for_default_config() {
        let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1")
            .await
            .unwrap();
        assert_eq!(provider.name(), "openai_like");
    }

    #[tokio::test]
    async fn test_name_returns_actual_provider_name() {
        let config = OpenAILikeConfig::new("https://api.groq.com/openai/v1")
            .with_provider_name("groq")
            .with_skip_api_key(true);
        let provider = OpenAILikeProvider::new(config).await.unwrap();
        assert_eq!(provider.name(), "groq");
    }

    #[tokio::test]
    async fn test_name_returns_deepseek_name() {
        let config = OpenAILikeConfig::new("https://api.deepseek.com/v1")
            .with_provider_name("deepseek")
            .with_skip_api_key(true);
        let provider = OpenAILikeProvider::new(config).await.unwrap();
        assert_eq!(provider.name(), "deepseek");
    }

    #[test]
    fn test_intern_provider_name_default() {
        let name = intern_provider_name("openai_like");
        assert_eq!(name, PROVIDER_NAME);
    }

    #[test]
    fn test_intern_provider_name_custom() {
        let name = intern_provider_name("xai");
        assert_eq!(name, "xai");
    }

    /// OR_SITE_URL → HTTP-Referer and OR_APP_NAME → X-Title are injected via
    /// env vars at request time in get_request_headers(). Integration tests cover
    /// the live path; here we verify the branch is only active for "openrouter".
    #[tokio::test]
    async fn test_non_openrouter_provider_no_or_headers() {
        // When provider_name is NOT "openrouter", OR_* env vars must be ignored
        // even if they happen to be set in the environment.
        let config = OpenAILikeConfig::new("https://api.openai.com/v1")
            .with_provider_name("openai")
            .with_skip_api_key(true);
        let Ok(provider) = OpenAILikeProvider::new(config).await else {
            panic!("provider creation must succeed");
        };
        let headers = provider.get_request_headers();

        let has_referer = headers.iter().any(|h| h.0 == "HTTP-Referer");
        let has_title = headers.iter().any(|h| h.0 == "X-Title");

        assert!(
            !has_referer,
            "HTTP-Referer must not be set for non-openrouter providers"
        );
        assert!(
            !has_title,
            "X-Title must not be set for non-openrouter providers"
        );
    }

    #[tokio::test]
    async fn test_openrouter_thinking_wired_in_transform() {
        use crate::core::types::thinking::{ThinkingConfig, ThinkingEffort};

        let config = OpenAILikeConfig::new("https://openrouter.ai/api/v1")
            .with_provider_name("openrouter")
            .with_skip_api_key(true);
        let Ok(provider) = OpenAILikeProvider::new(config).await else {
            panic!("provider creation must succeed");
        };

        let request = ChatRequest {
            model: "unknown-model".to_string(),
            messages: vec![],
            thinking: Some(
                ThinkingConfig::new()
                    .enabled()
                    .with_effort(ThinkingEffort::High)
                    .with_budget(5000),
            ),
            ..Default::default()
        };

        let Ok(json) = provider.transform_chat_request(request) else {
            panic!("transform_chat_request must succeed");
        };

        assert_eq!(
            json.get("reasoning")
                .and_then(|r| r.get("effort"))
                .and_then(|v| v.as_str()),
            Some("high"),
            "reasoning.effort must be forwarded"
        );
    }
}
