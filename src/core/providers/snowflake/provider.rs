//! Snowflake Cortex AI Provider Implementation
//!
//! Implements the LLMProvider trait for Snowflake Cortex AI.

use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::SnowflakeConfig;
use super::error::SnowflakeError;
use super::model_info::get_available_models;
use crate::core::providers::base::GlobalPoolManager;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::ProviderConfig as _;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::health::HealthStatus;
use crate::core::types::responses::{ChatChunk, ChatResponse, EmbeddingResponse};
use crate::core::types::{chat::ChatRequest, embedding::EmbeddingRequest};
use crate::core::types::{context::RequestContext, model::ModelInfo, model::ProviderCapability};

/// Static capabilities for Snowflake provider
const SNOWFLAKE_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

/// Snowflake Cortex AI provider implementation
#[derive(Debug, Clone)]
pub struct SnowflakeProvider {
    config: SnowflakeConfig,
    _pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl SnowflakeProvider {
    /// Create a new Snowflake provider instance
    pub async fn new(config: SnowflakeConfig) -> Result<Self, SnowflakeError> {
        config
            .validate()
            .map_err(|e| SnowflakeError::configuration("snowflake", e))?;

        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            SnowflakeError::configuration(
                "snowflake",
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        // Build model list from static configuration
        let models = get_available_models()
            .iter()
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
                    provider: "snowflake".to_string(),
                    max_context_length: info.max_context_length as u32,
                    max_output_length: Some(info.max_output_length as u32),
                    supports_streaming: true,
                    supports_tools: info.supports_tools,
                    supports_multimodal: false,
                    input_cost_per_1k_tokens: None, // Snowflake pricing varies by region
                    output_cost_per_1k_tokens: None,
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
            _pool_manager: pool_manager,
            models,
        })
    }

    /// Create provider with API key and account ID
    pub async fn with_api_key(
        api_key: impl Into<String>,
        account_id: impl Into<String>,
    ) -> Result<Self, SnowflakeError> {
        let config = SnowflakeConfig {
            api_key: Some(api_key.into()),
            account_id: Some(account_id.into()),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Get the API base URL
    fn get_api_base(&self) -> String {
        if let Some(base) = &self.config.api_base {
            base.clone()
        } else if let Some(account_id) = &self.config.account_id {
            format!("https://{}.snowflakecomputing.com/api/v2", account_id)
        } else {
            std::env::var("SNOWFLAKE_ACCOUNT_ID")
                .map(|id| format!("https://{}.snowflakecomputing.com/api/v2", id))
                .unwrap_or_else(|_| "https://snowflakecomputing.com/api/v2".to_string())
        }
    }

    /// Get the API key (JWT or PAT)
    fn get_api_key(&self) -> Option<String> {
        self.config
            .api_key
            .clone()
            .or_else(|| std::env::var("SNOWFLAKE_JWT").ok())
    }
}

impl LLMProvider for SnowflakeProvider {
    fn name(&self) -> &'static str {
        "snowflake"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        SNOWFLAKE_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        static PARAMS: &[&str] = &[
            "stream",
            "max_tokens",
            "temperature",
            "top_p",
            "stop",
            "tools",
            "tool_choice",
        ];
        PARAMS
    }

    async fn map_openai_params(
        &self,
        mut params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, ProviderError> {
        // Map max_completion_tokens to max_tokens
        if let Some(max_completion_tokens) = params.remove("max_completion_tokens") {
            params.insert("max_tokens".to_string(), max_completion_tokens);
        }

        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, ProviderError> {
        serde_json::to_value(&request)
            .map_err(|e| SnowflakeError::invalid_request("snowflake", e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        serde_json::from_slice(raw_response).map_err(|e| {
            SnowflakeError::api_error("snowflake", 500, format!("Failed to parse response: {}", e))
        })
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(crate::core::traits::error_mapper::DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("Snowflake chat request: model={}", request.model);

        let api_key = self
            .get_api_key()
            .ok_or_else(|| SnowflakeError::authentication("snowflake", "API key is required"))?;

        // Build the URL for Cortex LLM REST API
        let url = format!("{}/cortex/inference:complete", self.get_api_base());

        // Build request body
        let body = serde_json::json!({
            "model": request.model.strip_prefix("snowflake/").unwrap_or(&request.model),
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(1024),
            "top_p": request.top_p.unwrap_or(1.0),
        });

        // Execute request
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Snowflake Token=\"{}\"", api_key))
            .header("Content-Type", "application/json")
            .header("X-Snowflake-Authorization-Token-Type", "KEYPAIR_JWT")
            .json(&body)
            .send()
            .await
            .map_err(|e| SnowflakeError::network("snowflake", e.to_string()))?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| SnowflakeError::network("snowflake", e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&response_bytes);
            return Err(SnowflakeError::api_error(
                "snowflake",
                status.as_u16(),
                body_str.to_string(),
            ));
        }

        // Parse response
        let json: serde_json::Value = serde_json::from_slice(&response_bytes).map_err(|e| {
            SnowflakeError::api_error("snowflake", 500, format!("Failed to parse response: {}", e))
        })?;

        // Transform Snowflake response to OpenAI format
        let content = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("messages"))
            .and_then(|m| m.as_str())
            .unwrap_or("");

        Ok(ChatResponse {
            id: format!("snowflake-{}", uuid::Uuid::new_v4().simple()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: format!("snowflake/{}", request.model),
            choices: vec![crate::core::types::responses::ChatChoice {
                index: 0,
                message: crate::core::types::chat::ChatMessage {
                    role: crate::core::types::message::MessageRole::Assistant,
                    content: Some(crate::core::types::message::MessageContent::Text(
                        content.to_string(),
                    )),
                    thinking: None,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                },
                finish_reason: Some(crate::core::types::responses::FinishReason::Stop),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        })
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        Err(SnowflakeError::not_supported(
            "snowflake",
            "Streaming not yet implemented for Snowflake",
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        Err(SnowflakeError::not_supported(
            "snowflake",
            "Embeddings not supported by Snowflake Cortex provider",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.validate().is_ok() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        // Snowflake Cortex pricing varies by region and is consumption-based
        // Return 0.0 as we don't have accurate cost data
        Ok(0.0)
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_snowflake_provider_name() {
        // Test would require async runtime
    }
}
