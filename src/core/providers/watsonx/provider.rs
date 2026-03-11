//! Main Watsonx Provider Implementation
//!
//! Implements the LLMProvider trait for IBM Watsonx.ai platform.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use super::config::WatsonxConfig;
use super::error::{WatsonxError, WatsonxErrorMapper};
use super::model_info::{get_available_models, get_model_info, supports_tools};
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header_owned};
use crate::core::providers::unified_provider::ProviderError;
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

/// API endpoints for Watsonx
mod endpoints {
    /// Chat endpoint (non-streaming)
    pub const CHAT: &str = "/ml/v1/text/chat";
    /// Chat stream endpoint
    pub const CHAT_STREAM: &str = "/ml/v1/text/chat_stream";
    /// Deployment chat endpoint
    pub const DEPLOYMENT_CHAT: &str = "/ml/v1/deployments/{deployment_id}/text/chat";
    /// Deployment chat stream endpoint
    pub const DEPLOYMENT_CHAT_STREAM: &str = "/ml/v1/deployments/{deployment_id}/text/chat_stream";
}

/// Static capabilities for Watsonx provider
const WATSONX_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

/// Cached IAM token with expiry
#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    expires_at: std::time::Instant,
}

/// Watsonx provider implementation
#[derive(Debug)]
pub struct WatsonxProvider {
    config: WatsonxConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
    /// Cached IAM token
    token_cache: Arc<RwLock<Option<CachedToken>>>,
}

impl Clone for WatsonxProvider {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            pool_manager: self.pool_manager.clone(),
            models: self.models.clone(),
            token_cache: Arc::new(RwLock::new(None)), // Don't share token cache
        }
    }
}

impl WatsonxProvider {
    /// Create a new Watsonx provider instance
    pub async fn new(config: WatsonxConfig) -> Result<Self, WatsonxError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("watsonx", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration("watsonx", format!("Failed to create pool manager: {}", e))
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
                    provider: "watsonx".to_string(),
                    max_context_length: info.max_context_length as u32,
                    max_output_length: Some(info.max_output_length as u32),
                    supports_streaming: true,
                    supports_tools: info.supports_tools,
                    supports_multimodal: info.model_id.contains("vision"),
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
            token_cache: Arc::new(RwLock::new(None)),
        })
    }

    /// Create provider with API key and project ID
    pub async fn with_credentials(
        api_key: impl Into<String>,
        project_id: impl Into<String>,
        api_base: Option<String>,
    ) -> Result<Self, WatsonxError> {
        let config = WatsonxConfig {
            api_key: Some(api_key.into()),
            project_id: Some(project_id.into()),
            api_base,
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Generate or retrieve cached IAM token
    async fn get_token(&self) -> Result<String, WatsonxError> {
        // Check for pre-configured token
        if let Some(token) = self.config.get_token() {
            return Ok(token);
        }

        // Check for Zen API key
        if let Some(zen_key) = self.config.get_zen_api_key() {
            return Ok(format!("ZenApiKey {}", zen_key));
        }

        // Check cached token
        {
            let cache = self.token_cache.read().await;
            if let Some(cached) = cache.as_ref()
                && cached.expires_at > std::time::Instant::now()
            {
                return Ok(cached.token.clone());
            }
        }

        // Generate new IAM token
        let api_key = self.config.get_api_key().ok_or_else(|| {
            ProviderError::authentication(
                "watsonx",
                "API key not configured. Set WATSONX_API_KEY environment variable.",
            )
        })?;

        let iam_url = self.config.get_iam_url();
        debug!("Generating IAM token from {}", iam_url);

        let client = reqwest::Client::new();
        let response = client
            .post(&iam_url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .form(&[
                ("grant_type", "urn:ibm:params:oauth:grant-type:apikey"),
                ("apikey", &api_key),
            ])
            .send()
            .await
            .map_err(|e| {
                ProviderError::authentication("watsonx", format!("Failed to request token: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::authentication(
                "watsonx",
                format!("Token request failed with status {}: {}", status, body),
            ));
        }

        let token_response: serde_json::Value = response.json().await.map_err(|e| {
            ProviderError::authentication(
                "watsonx",
                format!("Failed to parse token response: {}", e),
            )
        })?;

        let access_token = token_response
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ProviderError::authentication("watsonx", "Missing access_token in response")
            })?
            .to_string();

        let expires_in = token_response
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        // Cache the token with some buffer before expiry
        let expires_at = std::time::Instant::now()
            + std::time::Duration::from_secs(expires_in.saturating_sub(60));

        {
            let mut cache = self.token_cache.write().await;
            *cache = Some(CachedToken {
                token: access_token.clone(),
                expires_at,
            });
        }

        Ok(access_token)
    }

    /// Build authorization headers
    async fn build_headers(&self) -> Result<Vec<(String, String)>, WatsonxError> {
        let token = self.get_token().await?;

        let auth_header = if token.starts_with("ZenApiKey ") {
            token
        } else {
            format!("Bearer {}", token)
        };

        Ok(vec![
            ("Authorization".to_string(), auth_header),
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Accept".to_string(), "application/json".to_string()),
        ])
    }

    /// Get the appropriate endpoint URL for the request
    fn get_endpoint_url(&self, model: &str, stream: bool) -> Result<String, WatsonxError> {
        let endpoint = if model.starts_with("deployment/") {
            // Extract deployment ID
            let deployment_id = model.trim_start_matches("deployment/");
            if stream {
                endpoints::DEPLOYMENT_CHAT_STREAM.replace("{deployment_id}", deployment_id)
            } else {
                endpoints::DEPLOYMENT_CHAT.replace("{deployment_id}", deployment_id)
            }
        } else if stream {
            endpoints::CHAT_STREAM.to_string()
        } else {
            endpoints::CHAT.to_string()
        };

        self.config
            .build_url(&endpoint, stream)
            .map_err(|e| ProviderError::configuration("watsonx", e))
    }

    /// Prepare the request payload
    fn prepare_payload(
        &self,
        model: &str,
        request: &ChatRequest,
    ) -> Result<serde_json::Value, WatsonxError> {
        let is_deployment = model.starts_with("deployment/");

        let mut payload = serde_json::json!({
            "messages": request.messages,
        });

        // Add model_id and project_id only for non-deployment models
        if !is_deployment {
            payload["model_id"] = serde_json::Value::String(model.to_string());

            if let Some(project_id) = self.config.get_project_id() {
                payload["project_id"] = serde_json::Value::String(project_id);
            } else if let Some(space_id) = self.config.get_space_id() {
                payload["space_id"] = serde_json::Value::String(space_id);
            } else {
                return Err(ProviderError::configuration(
                    "watsonx",
                    "Either project_id or space_id must be configured",
                ));
            }
        }

        // Add optional parameters
        if let Some(temp) = request.temperature {
            payload["temperature"] = serde_json::Value::Number(
                serde_json::Number::from_f64(temp as f64).unwrap_or_else(|| 0.into()),
            );
        }

        if let Some(max_tokens) = request.max_tokens {
            payload["max_tokens"] = serde_json::Value::Number(max_tokens.into());
        }

        if let Some(top_p) = request.top_p {
            payload["top_p"] = serde_json::Value::Number(
                serde_json::Number::from_f64(top_p as f64).unwrap_or_else(|| 1.into()),
            );
        }

        if let Some(ref stop) = request.stop {
            payload["stop_sequences"] = serde_json::to_value(stop).unwrap_or_default();
        }

        if let Some(freq_penalty) = request.frequency_penalty {
            payload["repetition_penalty"] = serde_json::Value::Number(
                serde_json::Number::from_f64(freq_penalty as f64).unwrap_or_else(|| 1.into()),
            );
        }

        if let Some(seed) = request.seed {
            payload["random_seed"] = serde_json::Value::Number(seed.into());
        }

        // Handle tools
        if let Some(ref tools) = request.tools {
            payload["tools"] = serde_json::to_value(tools).unwrap_or_default();
        }

        if let Some(ref tool_choice) = request.tool_choice {
            // Serialize tool_choice to JSON Value for Watsonx
            let tool_choice_value = serde_json::to_value(tool_choice).unwrap_or_default();
            // Check if it's a string option like "auto", "none", "required"
            if let serde_json::Value::String(ref choice) = tool_choice_value {
                if choice == "auto" || choice == "none" || choice == "required" {
                    payload["tool_choice_option"] = tool_choice_value.clone();
                } else {
                    payload["tool_choice"] = tool_choice_value.clone();
                }
            } else {
                payload["tool_choice"] = tool_choice_value;
            }
        }

        // Handle response format
        if let Some(ref response_format) = request.response_format {
            payload["response_format"] = serde_json::to_value(response_format).unwrap_or_default();
        }

        Ok(payload)
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, WatsonxError> {
        let headers = self.build_headers().await?;
        let header_tuples: Vec<_> = headers
            .iter()
            .map(|(k, v)| header_owned(k.clone(), v.clone()))
            .collect();

        let response = self
            .pool_manager
            .execute_request(url, HttpMethod::POST, header_tuples, Some(body))
            .await
            .map_err(|e| ProviderError::network("watsonx", e.to_string()))?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("watsonx", e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&response_bytes);
            let mapper = WatsonxErrorMapper;
            return Err(mapper.map_http_error(status.as_u16(), &body_str));
        }

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::response_parsing("watsonx", format!("Failed to parse response: {}", e))
        })
    }
}

#[async_trait]
impl LLMProvider for WatsonxProvider {
    type Config = WatsonxConfig;
    type Error = WatsonxError;
    type ErrorMapper = WatsonxErrorMapper;

    fn name(&self) -> &'static str {
        "watsonx"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        WATSONX_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        if supports_tools(model) {
            &[
                "temperature",
                "max_tokens",
                "top_p",
                "frequency_penalty",
                "stop",
                "seed",
                "stream",
                "tools",
                "tool_choice",
                "logprobs",
                "top_logprobs",
                "n",
                "presence_penalty",
                "response_format",
                "reasoning_effort",
            ]
        } else {
            &[
                "temperature",
                "max_tokens",
                "top_p",
                "frequency_penalty",
                "stop",
                "seed",
                "stream",
                "logprobs",
                "top_logprobs",
                "n",
                "presence_penalty",
                "response_format",
            ]
        }
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        // Watsonx uses similar parameters to OpenAI with some mapping
        let mut mapped = HashMap::new();

        for (key, value) in params {
            let mapped_key = match key.as_str() {
                "max_tokens" => "max_new_tokens".to_string(),
                "frequency_penalty" => "repetition_penalty".to_string(),
                "stop" => "stop_sequences".to_string(),
                "seed" => "random_seed".to_string(),
                _ => key,
            };
            mapped.insert(mapped_key, value);
        }

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        self.prepare_payload(&request.model, &request)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::response_parsing("watsonx", format!("Failed to parse response: {}", e))
        })
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        WatsonxErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Watsonx chat request: model={}", request.model);

        let url = self.get_endpoint_url(&request.model, false)?;
        let payload = self.prepare_payload(&request.model, &request)?;

        let response = self.execute_request(&url, payload).await?;

        serde_json::from_value(response).map_err(|e| {
            ProviderError::response_parsing(
                "watsonx",
                format!("Failed to parse chat response: {}", e),
            )
        })
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Watsonx streaming request: model={}", request.model);

        let url = self.get_endpoint_url(&request.model, true)?;
        let headers = self.build_headers().await?;
        let payload = self.prepare_payload(&request.model, &request)?;

        // Execute streaming request using reqwest directly for SSE
        let client = reqwest::Client::new();
        let mut req_builder = client.post(&url);

        for (key, value) in headers {
            req_builder = req_builder.header(key, value);
        }

        let response = req_builder
            .json(&payload)
            .send()
            .await
            .map_err(|e| ProviderError::network("watsonx", e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            let mapper = WatsonxErrorMapper;
            return Err(mapper.map_http_error(status, &body.unwrap_or_default()));
        }

        // Create SSE stream
        let stream = super::streaming::create_watsonx_stream(response.bytes_stream());
        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(ProviderError::not_supported(
            "watsonx",
            "Embeddings are available through a separate Watsonx embeddings API. \
            Use the watsonx_embed provider for embeddings.",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Try to get a token as a health check
        match self.get_token().await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        let model_info = get_model_info(model).ok_or_else(|| {
            ProviderError::model_not_found("watsonx", format!("Unknown model: {}", model))
        })?;

        let input_cost = (input_tokens as f64) * (model_info.input_cost_per_million / 1_000_000.0);
        let output_cost =
            (output_tokens as f64) * (model_info.output_cost_per_million / 1_000_000.0);
        Ok(input_cost + output_cost)
    }
}
