//! OpenAI-Like Provider Implementation
//!
//! Main provider implementation for any OpenAI-compatible API endpoint

use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpMethod, header, header_owned, read_streaming_error_body,
};
use crate::core::providers::openai::{OpenAIResponseTransformer, models::OpenAIChatResponse};
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    model::{ModelInfo, ProviderCapability},
    responses::{ChatChunk, ChatResponse},
};

use super::{
    config::OpenAILikeConfig,
    error::{OpenAILikeError, PROVIDER_NAME},
    models::{OpenAILikeModelRegistry, get_openai_like_registry},
};
use crate::core::providers::unified_provider::ProviderError;

pub(crate) static OPENAI_LIKE_CATALOG_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    ProviderCapability::FunctionCalling,
];

pub(crate) static OPENAI_COMPATIBLE_PROXY_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ImageEdit,
    ProviderCapability::ImageVariation,
    ProviderCapability::Moderation,
    ProviderCapability::ToolCalling,
    ProviderCapability::FunctionCalling,
];

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
    /// Provider name returned by the LLM provider trait.
    provider_name: String,
    /// Catalog or instance capability profile, validated against executable methods.
    capabilities: &'static [ProviderCapability],
}

impl OpenAILikeProvider {
    /// Create a new OpenAI-like provider
    pub async fn new(config: OpenAILikeConfig) -> Result<Self, OpenAILikeError> {
        Self::new_with_profile(
            config,
            OPENAI_LIKE_CATALOG_CAPABILITIES,
            OPENAI_LIKE_CATALOG_CAPABILITIES,
        )
        .await
    }

    pub(crate) async fn new_for_catalog(
        config: OpenAILikeConfig,
        capabilities: &'static [ProviderCapability],
    ) -> Result<Self, OpenAILikeError> {
        Self::new_with_profile(config, capabilities, OPENAI_LIKE_CATALOG_CAPABILITIES).await
    }

    pub(crate) async fn new_openai_compatible(
        config: OpenAILikeConfig,
    ) -> Result<Self, OpenAILikeError> {
        Self::new_with_profile(
            config,
            OPENAI_COMPATIBLE_PROXY_CAPABILITIES,
            OPENAI_COMPATIBLE_PROXY_CAPABILITIES,
        )
        .await
    }

    async fn new_with_profile(
        config: OpenAILikeConfig,
        capabilities: &'static [ProviderCapability],
        allowed_capabilities: &'static [ProviderCapability],
    ) -> Result<Self, OpenAILikeError> {
        config
            .validate()
            .map_err(|e| OpenAILikeError::configuration(PROVIDER_NAME, e))?;
        Self::validate_capability_profile(capabilities, allowed_capabilities)?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new_for_provider(PROVIDER_NAME, config.base.clone())
                .map_err(|e| OpenAILikeError::configuration(PROVIDER_NAME, e.to_string()))?,
        );
        let model_registry = get_openai_like_registry();
        let provider_name = config.provider_name.clone();

        Ok(Self {
            pool_manager,
            config,
            model_registry,
            provider_name,
            capabilities,
        })
    }

    fn validate_capability_profile(
        capabilities: &'static [ProviderCapability],
        allowed_capabilities: &'static [ProviderCapability],
    ) -> Result<(), OpenAILikeError> {
        if capabilities.is_empty() {
            return Err(OpenAILikeError::configuration(
                PROVIDER_NAME,
                "capability profile cannot be empty",
            ));
        }

        for (index, capability) in capabilities.iter().enumerate() {
            if capabilities[..index].contains(capability) {
                return Err(OpenAILikeError::configuration(
                    PROVIDER_NAME,
                    format!("capability profile contains duplicate {capability:?}"),
                ));
            }
            if !allowed_capabilities.contains(capability) {
                return Err(OpenAILikeError::configuration(
                    PROVIDER_NAME,
                    format!(
                        "capability {capability:?} is not executable for this OpenAI-like profile"
                    ),
                ));
            }
        }

        Ok(())
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

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        if let Some(org) = &self.config.base.organization {
            headers.push(header("OpenAI-Organization", org.clone()));
        }

        for (key, value) in &self.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        for (key, value) in &self.config.custom_headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

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
        let openai_request = self.transform_chat_request(request)?;

        let url = format!("{}/chat/completions", self.config.get_api_base());
        let headers = self.get_request_headers();
        let body = Some(openai_request);

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body)
            .await
            .map_err(|e| OpenAILikeError::network(PROVIDER_NAME, e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .map_err(|error| OpenAILikeError::network(PROVIDER_NAME, error.to_string()))?;
            return Err(self.map_error_response(status.as_u16(), &body));
        }

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| OpenAILikeError::network(PROVIDER_NAME, e.to_string()))?;

        let response_json: Value = serde_json::from_slice(&response_bytes)
            .map_err(|e| OpenAILikeError::response_parsing(PROVIDER_NAME, e.to_string()))?;

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
        let mut openai_request = self.transform_chat_request(request)?;
        openai_request["stream"] = Value::Bool(true);

        let url = format!("{}/chat/completions", self.config.get_api_base());
        let headers = self.get_request_headers();
        let response = self
            .pool_manager
            .execute_streaming_request(&url, headers, openai_request, PROVIDER_NAME)
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = read_streaming_error_body(response)
                .await
                .map_err(|e| e.into_provider_error(PROVIDER_NAME))?;
            return Err(self.map_error_response(status.as_u16(), &body));
        }

        let stream = response.bytes_stream();
        Ok(Box::pin(super::streaming::create_openai_like_stream(
            stream,
        )))
    }

    /// Transform ChatRequest to OpenAI API format
    fn transform_chat_request(&self, request: ChatRequest) -> Result<Value, OpenAILikeError> {
        let model = self.config.get_effective_model(&request.model);

        let mut openai_request = serde_json::json!({
            "model": model,
            "messages": request.messages
        });

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

        let reasoning_effort = request.reasoning_effort;

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

        macro_rules! insert_optional_param {
            ($field:ident) => {
                if let Some(value) = request.$field {
                    openai_request[stringify!($field)] =
                        serde_json::to_value(value).map_err(|e| {
                            OpenAILikeError::serialization(PROVIDER_NAME, e.to_string())
                        })?;
                }
            };
        }
        insert_optional_param!(frequency_penalty);
        insert_optional_param!(presence_penalty);
        insert_optional_param!(logit_bias);
        insert_optional_param!(logprobs);
        insert_optional_param!(top_logprobs);
        insert_optional_param!(store);
        insert_optional_param!(metadata);
        insert_optional_param!(service_tier);
        insert_optional_param!(parallel_tool_calls);
        insert_optional_param!(functions);
        insert_optional_param!(function_call);

        if let Some(effort) = reasoning_effort {
            self.insert_reasoning_effort(&mut openai_request, &model, effort)?;
        }

        if let Some(obj) = openai_request.as_object_mut() {
            for (key, value) in request.extra_params {
                obj.entry(key).or_insert(value);
            }

            if let Some(Value::Object(params)) = openrouter_thinking_params {
                for (key, value) in params {
                    match obj.get_mut(&key) {
                        Some(Value::Object(existing)) if value.is_object() => {
                            if let Value::Object(incoming) = value {
                                for (k, v) in incoming {
                                    existing.entry(k).or_insert(v);
                                }
                            }
                        }
                        Some(_) => {}
                        None => {
                            obj.insert(key, value);
                        }
                    }
                }
            }
        }

        Ok(openai_request)
    }

    fn insert_reasoning_effort(
        &self,
        request: &mut Value,
        model: &str,
        effort: String,
    ) -> Result<(), OpenAILikeError> {
        if self.config.provider_name != "xai" {
            request["reasoning_effort"] = Value::String(effort);
            return Ok(());
        }

        Self::reject_xai_reasoning_incompatible_params(request)?;

        match super::models::xai_reasoning_param_for_model(model) {
            Some(super::models::XaiReasoningParam::TopLevelReasoningEffort) => {
                request["reasoning_effort"] = Value::String(effort);
                Ok(())
            }
            Some(super::models::XaiReasoningParam::NestedReasoningEffort) => {
                request["reasoning"] = serde_json::json!({ "effort": effort });
                Ok(())
            }
            None => Err(OpenAILikeError::configuration(
                PROVIDER_NAME,
                format!("xAI model {model} does not support reasoning_effort"),
            )),
        }
    }

    fn reject_xai_reasoning_incompatible_params(request: &Value) -> Result<(), OpenAILikeError> {
        let incompatible_params = ["stop", "presence_penalty", "frequency_penalty"]
            .into_iter()
            .filter(|field| request.get(*field).is_some())
            .collect::<Vec<_>>();

        if incompatible_params.is_empty() {
            return Ok(());
        }

        Err(OpenAILikeError::configuration(
            PROVIDER_NAME,
            format!(
                "xAI reasoning_effort is incompatible with {}",
                incompatible_params.join(", ")
            ),
        ))
    }

    fn transform_chat_response(&self, response: Value) -> Result<ChatResponse, OpenAILikeError> {
        let resp: OpenAIChatResponse = serde_json::from_value(response)
            .map_err(|e| OpenAILikeError::response_parsing(PROVIDER_NAME, e.to_string()))?;
        OpenAIResponseTransformer::transform(resp)
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
            _ => OpenAILikeError::openai_like_api_error(status, sanitized_upstream_error(status)),
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

fn sanitized_upstream_error(status: u16) -> String {
    format!(
        "Upstream OpenAI-compatible provider returned HTTP {}",
        status
    )
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
            _ => E::network_error(&sanitized_upstream_error(status_code)),
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
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn error_provider_name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        self.capabilities
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
        let url = format!("{}/models", self.config.get_api_base());
        match self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, self.get_request_headers(), None)
            .await
        {
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
        if self.config.provider_name != "xai" && super::models::is_xai_priced_model(model) {
            return Ok(0.0);
        }

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
            "reasoning_effort",
            "store",
            "metadata",
            "service_tier",
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
mod tests;
