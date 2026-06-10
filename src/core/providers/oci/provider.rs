//! Main OCI Generative AI Provider Implementation
//!
//! Implements the LLMProvider trait for Oracle Cloud Infrastructure Generative AI.

use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::OciConfig;
use super::error::OciErrorMapper;
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

/// Static capabilities for OCI provider
const OCI_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

/// OCI Generative AI provider implementation
#[derive(Debug)]
pub struct OciProvider {
    config: OciConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl Clone for OciProvider {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            pool_manager: self.pool_manager.clone(),
            models: self.models.clone(),
        }
    }
}

impl OciProvider {
    /// Create a new OCI provider instance
    pub async fn new(config: OciConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("oci", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration("oci", format!("Failed to create pool manager: {}", e))
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
                    provider: "oci".to_string(),
                    max_context_length: info.max_context_length,
                    max_output_length: Some(info.max_output_length),
                    supports_streaming: true,
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

    /// Create provider with auth token and compartment ID
    pub async fn with_credentials(
        auth_token: impl Into<String>,
        compartment_id: impl Into<String>,
        region: Option<String>,
    ) -> Result<Self, ProviderError> {
        let config = OciConfig {
            auth_token: Some(auth_token.into()),
            compartment_id: Some(compartment_id.into()),
            region,
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Build authorization headers
    fn build_headers(&self) -> Result<Vec<(String, String)>, ProviderError> {
        let auth_token = self.config.get_auth_token().ok_or_else(|| {
            ProviderError::authentication(
                "oci",
                "Auth token not configured. Set OCI_AUTH_TOKEN environment variable.",
            )
        })?;

        Ok(vec![
            (
                "Authorization".to_string(),
                format!("Bearer {}", auth_token),
            ),
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Accept".to_string(), "application/json".to_string()),
        ])
    }

    /// Prepare the request payload
    fn prepare_payload(&self, request: &ChatRequest) -> Result<serde_json::Value, ProviderError> {
        let compartment_id = self.config.get_compartment_id().ok_or_else(|| {
            ProviderError::configuration(
                "oci",
                "Compartment ID not configured. Set OCI_COMPARTMENT_ID environment variable.",
            )
        })?;

        let mut payload = serde_json::json!({
            "compartmentId": compartment_id,
            "servingMode": {
                "servingType": "ON_DEMAND",
                "modelId": request.model
            },
            "chatRequest": {
                "apiFormat": "COHERE",
                "messages": request.messages
            }
        });

        // Add optional parameters
        let chat_request = payload.get_mut("chatRequest").ok_or_else(|| {
            ProviderError::serialization(
                "oci",
                "Failed to build OCI payload: missing chatRequest field",
            )
        })?;

        if let Some(temp) = request.temperature {
            let temp_f64 = temp as f64;
            chat_request["temperature"] = serde_json::Value::Number(
                serde_json::Number::from_f64(temp_f64).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "oci",
                        format!("invalid temperature value: {temp_f64} (NaN and Infinity are not allowed)"),
                    )
                })?,
            );
        }

        if let Some(max_tokens) = request.max_tokens {
            chat_request["maxTokens"] = serde_json::Value::Number(max_tokens.into());
        }

        if let Some(top_p) = request.top_p {
            let top_p_f64 = top_p as f64;
            chat_request["topP"] = serde_json::Value::Number(
                serde_json::Number::from_f64(top_p_f64).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "oci",
                        format!(
                            "invalid top_p value: {top_p_f64} (NaN and Infinity are not allowed)"
                        ),
                    )
                })?,
            );
        }

        if let Some(ref stop) = request.stop {
            chat_request["stopSequences"] = serde_json::to_value(stop).unwrap_or_default();
        }

        if let Some(freq_penalty) = request.frequency_penalty {
            let freq_f64 = freq_penalty as f64;
            chat_request["frequencyPenalty"] = serde_json::Value::Number(
                serde_json::Number::from_f64(freq_f64).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "oci",
                        format!("invalid frequency_penalty value: {freq_f64} (NaN and Infinity are not allowed)"),
                    )
                })?,
            );
        }

        if let Some(presence_penalty) = request.presence_penalty {
            let pres_f64 = presence_penalty as f64;
            chat_request["presencePenalty"] = serde_json::Value::Number(
                serde_json::Number::from_f64(pres_f64).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "oci",
                        format!("invalid presence_penalty value: {pres_f64} (NaN and Infinity are not allowed)"),
                    )
                })?,
            );
        }

        // Handle tools
        if let Some(ref tools) = request.tools {
            chat_request["tools"] = serde_json::to_value(tools).unwrap_or_default();
        }

        Ok(payload)
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let headers = self.build_headers()?;
        let header_tuples: Vec<_> = headers
            .iter()
            .map(|(k, v)| header_owned(k.clone(), v.clone()))
            .collect();

        let response = self
            .pool_manager
            .execute_request(url, HttpMethod::POST, header_tuples, Some(body))
            .await
            .map_err(|e| ProviderError::network("oci", e.to_string()))?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("oci", e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&response_bytes);
            let mapper = OciErrorMapper;
            return Err(mapper.map_http_error(status.as_u16(), &body_str));
        }

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::response_parsing("oci", format!("Failed to parse response: {}", e))
        })
    }
}

impl LLMProvider for OciProvider {
    fn name(&self) -> &'static str {
        "oci"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        OCI_CAPABILITIES
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
                "presence_penalty",
                "stop",
                "stream",
                "tools",
                "tool_choice",
            ]
        } else {
            &[
                "temperature",
                "max_tokens",
                "top_p",
                "frequency_penalty",
                "presence_penalty",
                "stop",
                "stream",
            ]
        }
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, ProviderError> {
        // OCI uses camelCase parameters
        let mut mapped = HashMap::new();

        for (key, value) in params {
            let mapped_key = match key.as_str() {
                "max_tokens" => "maxTokens".to_string(),
                "top_p" => "topP".to_string(),
                "frequency_penalty" => "frequencyPenalty".to_string(),
                "presence_penalty" => "presencePenalty".to_string(),
                "stop" => "stopSequences".to_string(),
                "tool_choice" => "toolChoice".to_string(),
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
    ) -> Result<serde_json::Value, ProviderError> {
        self.prepare_payload(&request)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::response_parsing("oci", format!("Failed to parse response: {}", e))
        })
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(OciErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("OCI chat request: model={}", request.model);

        let url = self.config.build_chat_url();
        let payload = self.prepare_payload(&request)?;

        let response = self.execute_request(&url, payload).await?;

        // Transform OCI response to OpenAI format
        transform_oci_response(response)
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        debug!("OCI streaming request: model={}", request.model);

        let url = self.config.build_chat_url();
        let headers = self.build_headers()?;
        let mut payload = self.prepare_payload(&request)?;

        // Enable streaming
        if let Some(chat_req) = payload.get_mut("chatRequest") {
            chat_req["isStream"] = serde_json::Value::Bool(true);
        }

        // Execute streaming request
        let client = crate::core::http::outbound::streaming_outbound_client().clone();
        let mut req_builder = client.post(&url);

        for (key, value) in headers {
            req_builder = req_builder.header(key, value);
        }

        let response = crate::core::providers::base::connection_pool::send_streaming_request(
            req_builder.json(&payload),
            "oci",
        )
        .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body =
                crate::core::providers::base::connection_pool::read_streaming_error_body(response)
                    .await
                    .map_err(|err| err.into_provider_error("oci"))?;
            let mapper = OciErrorMapper;
            return Err(mapper.map_http_error(status, &body));
        }

        let stream = super::streaming::create_oci_stream(response.bytes_stream());
        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::not_supported(
            "oci",
            "Embeddings are available through OCI Generative AI Embeddings API. Use the oci_embed provider for embeddings.",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Check if we have valid credentials
        if self.config.get_auth_token().is_some() && self.config.get_compartment_id().is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        let model_info = get_model_info(model).ok_or_else(|| {
            ProviderError::model_not_found("oci", format!("Unknown model: {}", model))
        })?;

        let input_cost = (input_tokens as f64) * (model_info.input_cost_per_million / 1_000_000.0);
        let output_cost =
            (output_tokens as f64) * (model_info.output_cost_per_million / 1_000_000.0);
        Ok(input_cost + output_cost)
    }
}

/// Transform OCI response to OpenAI-compatible format
fn transform_oci_response(response: serde_json::Value) -> Result<ChatResponse, ProviderError> {
    // OCI response format may differ from OpenAI
    // Try direct parsing first, then transform if needed
    if let Ok(chat_response) = serde_json::from_value::<ChatResponse>(response.clone()) {
        return Ok(chat_response);
    }

    // Transform OCI-specific response format
    let chat_result = response.get("chatResponse").unwrap_or(&response);

    let id = response
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("oci-response")
        .to_string();

    let model = response
        .get("modelId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let text = chat_result
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let _finish_reason = chat_result
        .get("finishReason")
        .and_then(|v| v.as_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "stop".to_string());

    // Build OpenAI-compatible response
    Ok(ChatResponse {
        id,
        object: "chat.completion".to_string(),
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64),
        model,
        choices: vec![crate::core::types::responses::ChatChoice {
            index: 0,
            message: crate::core::types::chat::ChatMessage {
                role: crate::core::types::message::MessageRole::Assistant,
                content: Some(crate::core::types::message::MessageContent::Text(text)),
                thinking: None,
                audio: None,
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
