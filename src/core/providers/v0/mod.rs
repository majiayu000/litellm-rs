//! V0 AI Provider Module
//!
//! V0 is an OpenAI-compatible AI platform for developers
//! <https://v0.dev/>

pub mod chat;

use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::{
    providers::base::HttpErrorMapper,
    providers::unified_provider::ProviderError,
    traits::{
        error_mapper::types::GenericErrorMapper,
        provider::{LLMProvider, ProviderConfig},
    },
    types::{
        chat::ChatRequest, context::RequestContext, health::HealthStatus, model::ModelInfo,
        model::ProviderCapability, responses::ChatResponse,
    },
};
use crate::utils::net::http::create_custom_client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Provider name constant for error messages
const PROVIDER_NAME: &str = "v0";

/// V0 Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V0Config {
    /// API base URL for V0
    pub api_base: String,
    /// API key for authentication
    pub api_key: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
}

impl Default for V0Config {
    fn default() -> Self {
        Self {
            api_base: "https://api.v0.dev/v1".to_string(),
            api_key: String::new(),
            timeout_seconds: 60,
            max_retries: 3,
        }
    }
}

impl V0Config {
    /// Configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.api_key.is_empty() {
            return Err("V0 API key is required".to_string());
        }
        if self.api_base.is_empty() {
            return Err("V0 API base URL is required".to_string());
        }
        Ok(())
    }
}

/// implementation ProviderConfig trait
impl ProviderConfig for V0Config {
    /// Configuration
    fn validate(&self) -> Result<(), String> {
        self.validate()
    }

    /// Get
    fn api_key(&self) -> Option<&str> {
        if self.api_key.is_empty() {
            None
        } else {
            Some(&self.api_key)
        }
    }

    /// Get
    fn api_base(&self) -> Option<&str> {
        Some(&self.api_base)
    }

    /// Get
    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_seconds)
    }

    /// Get
    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// V0 supported models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum V0Model {
    /// V0 Default Model
    V0Default,
    /// Custom model
    Custom(String),
}

impl V0Model {
    /// Get model identifier for API calls
    pub fn model_id(&self) -> String {
        match self {
            Self::V0Default => "v0-default".to_string(),
            Self::Custom(id) => id.clone(),
        }
    }

    /// Check if model supports function calling
    pub fn supports_function_calling(&self) -> bool {
        matches!(self, Self::V0Default | Self::Custom(_))
    }

    /// Check if model supports streaming
    pub fn supports_streaming(&self) -> bool {
        true
    }

    /// Get maximum context window size
    pub fn max_context_tokens(&self) -> usize {
        match self {
            Self::V0Default => 32768,
            Self::Custom(_) => 32768, // Default assumption
        }
    }
}

/// Parse model string to V0Model enum
pub fn parse_v0_model(model: &str) -> V0Model {
    match model {
        "v0" | "v0-default" => V0Model::V0Default,
        _ => V0Model::Custom(model.to_string()),
    }
}

/// V0 Provider implementation
#[derive(Debug, Clone)]
pub struct V0Provider {
    config: V0Config,
    client: reqwest::Client,
}

impl V0Provider {
    /// Create a new V0 provider
    ///
    /// # Errors
    /// Returns error if HTTP client cannot be created
    pub fn new(
        config: V0Config,
    ) -> Result<Self, crate::core::providers::unified_provider::ProviderError> {
        let client = create_custom_client(std::time::Duration::from_secs(config.timeout_seconds))
            .map_err(|e| {
            crate::core::providers::unified_provider::ProviderError::Configuration {
                provider: "v0",
                message: format!("Failed to create HTTP client: {}", e),
            }
        })?;

        Ok(Self { config, client })
    }

    /// Create a new V0 provider with default client on error
    pub fn new_or_default(config: V0Config) -> Self {
        Self::new(config.clone()).unwrap_or_else(|e| {
            tracing::error!("Failed to create V0 provider: {}, using default client", e);
            Self {
                config,
                client: crate::core::http::outbound::default_outbound_client().clone(),
            }
        })
    }

    /// Get API endpoint URL
    fn get_endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.config.api_base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Create request headers
    fn create_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(auth_value) = format!("Bearer {}", self.config.api_key).parse() {
            headers.insert(reqwest::header::AUTHORIZATION, auth_value);
        }
        if let Ok(content_type) = "application/json".parse() {
            headers.insert(reqwest::header::CONTENT_TYPE, content_type);
        }
        headers
    }

    /// Internal health check method
    async fn check_health(&self) -> Result<(), ProviderError> {
        let url = self.get_endpoint("models");
        let response = self
            .client
            .get(&url)
            .headers(self.create_headers())
            .send()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(HttpErrorMapper::map_status_code(
                PROVIDER_NAME,
                response.status().as_u16(),
                &format!("Health check failed with status: {}", response.status()),
            ))
        }
    }
}

/// Implementation of unified LLMProvider trait
///
/// V0 is an OpenAI-compatible AI platform
impl LLMProvider for V0Provider {
    /// Get
    fn name(&self) -> &'static str {
        "v0"
    }

    /// Get
    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ToolCalling,
            ProviderCapability::FunctionCalling,
        ]
    }

    /// Model
    fn models(&self) -> &[ModelInfo] {
        // Use LazyLock for lazy initialization of static data
        static MODELS: LazyLock<Vec<ModelInfo>> = LazyLock::new(|| {
            vec![ModelInfo {
                id: "v0-default".to_string(),
                name: "V0 Default Model".to_string(),
                provider: "v0".to_string(),
                max_context_length: 32768,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.1),
                output_cost_per_1k_tokens: Some(0.2),
                currency: "USD".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                    ProviderCapability::ToolCalling,
                ],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            }]
        });
        &MODELS
    }

    // ==================== Python LiteLLM compatible interface ====================

    /// Get
    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "messages",
            "model",
            "temperature",
            "max_tokens",
            "top_p",
            "stream",
            "tools",
            "tool_choice",
            "user",
            "seed",
        ]
    }

    /// Map OpenAI parameters to V0 parameters
    async fn map_openai_params(
        &self,
        mut params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        // V0 uses OpenAI-compatible parameters, so most parameters are passed through directly

        // Can add specific parameter mapping logic here
        // For example: rename certain parameters or convert formats

        // Ensure stream parameter is boolean value, not Option<bool>
        if let Some(stream_val) = params.get("stream")
            && let Some(stream_bool) = stream_val.as_bool()
        {
            params.insert("stream".to_string(), Value::Bool(stream_bool));
        }

        Ok(params)
    }

    /// Request
    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        // Request
        if request.messages.is_empty() {
            return Err(ProviderError::invalid_request(
                PROVIDER_NAME,
                "Messages cannot be empty",
            ));
        }

        if request.model.is_empty() {
            return Err(ProviderError::invalid_request(
                PROVIDER_NAME,
                "Model cannot be empty",
            ));
        }

        // Convert to V0 API format (OpenAI compatible)
        let v0_request = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
            "top_p": request.top_p,
            "stream": request.stream,
            "tools": request.tools,
            "tool_choice": request.tool_choice,
        });

        Ok(v0_request)
    }

    /// Response
    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        // Response
        let response_json: Value = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::serialization(PROVIDER_NAME, e.to_string()))?;

        // Convert to standard ChatResponse format
        // Response
        // Create

        let choices = response_json
            .get("choices")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                ProviderError::response_parsing(PROVIDER_NAME, "Invalid response format")
            })?;

        let usage = response_json
            .get("usage")
            .map(|u| serde_json::from_value(u.clone()))
            .transpose()
            .map_err(|e| ProviderError::serialization(PROVIDER_NAME, e.to_string()))?;

        let chat_response = ChatResponse {
            id: request_id.to_string(),
            object: "chat.completion".to_string(),
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            model: model.to_string(),
            choices: serde_json::from_value(serde_json::Value::Array(choices.clone()))
                .map_err(|e| ProviderError::serialization(PROVIDER_NAME, e.to_string()))?,
            usage,
            system_fingerprint: None,
        };

        Ok(chat_response)
    }

    /// Error
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(GenericErrorMapper)
    }

    // ==================== Core functionality: chat completion ====================

    /// Request
    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        // Use new transformation flow
        let _transformed_request = self
            .transform_request(request.clone(), context.clone())
            .await?;

        // Should call actual API here, using original handler for demonstration
        chat::V0ChatHandler::handle_chat_completion(self, request).await
    }

    /// Check
    async fn health_check(&self) -> HealthStatus {
        match self.check_health().await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    /// Request
    async fn calculate_cost(
        &self,
        _model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        // V0 pricing: input $0.1/1K tokens, output $0.2/1K tokens
        let input_cost = (input_tokens as f64 / 1000.0) * 0.1;
        let output_cost = (output_tokens as f64 / 1000.0) * 0.2;
        Ok(input_cost + output_cost)
    }
}

// Provider trait implementation removed - V0Provider is now included through the Provider enum variants

// ==================== Unit Tests ====================

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
