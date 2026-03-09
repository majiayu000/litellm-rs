//! Main Fireworks AI Provider Implementation
//!
//! Implements the LLMProvider trait for Fireworks AI's fast inference platform.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::FireworksConfig;
use super::model_info::{
    format_model_id, get_available_models, get_model_info, is_reasoning_model,
    supports_function_calling, supports_tool_choice,
};
use crate::core::providers::base::{GlobalPoolManager, HttpErrorMapper, HttpMethod, header};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::streaming::utils::is_done_marker;
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

/// Static capabilities for Fireworks AI provider
const FIREWORKS_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

fn parse_fireworks_sse_line(line: &str) -> Option<Result<ChatChunk, ProviderError>> {
    let data = line.strip_prefix("data: ")?;
    if is_done_marker(data) {
        return None;
    }

    Some(serde_json::from_str::<ChatChunk>(data).map_err(|e| {
        ProviderError::api_error("fireworks", 500, format!("Failed to parse chunk: {}", e))
    }))
}

/// Fireworks AI provider implementation
#[derive(Debug, Clone)]
pub struct FireworksProvider {
    config: FireworksConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl FireworksProvider {
    /// Create a new Fireworks AI provider instance
    pub async fn new(config: FireworksConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("fireworks", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                "fireworks",
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        // Build model list from static configuration
        let models = get_available_models()
            .iter()
            .filter_map(|id| get_model_info(id))
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
                    provider: "fireworks".to_string(),
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

    /// Create provider with API key only
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = FireworksConfig {
            api_key: Some(api_key.into()),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Transform messages for Fireworks AI API
    fn transform_messages(&self, _request: &mut ChatRequest) {
        // Fireworks AI uses standard OpenAI-compatible messages
        // No special transformation needed
    }

    /// Transform tools to remove unsupported fields
    fn transform_tools(&self, request: &mut ChatRequest) {
        if let Some(ref mut tools) = request.tools {
            for tool in tools.iter_mut() {
                // Remove 'strict' field from function parameters if present
                if let Some(ref mut params) = tool.function.parameters {
                    if let Some(obj) = params.as_object_mut() {
                        obj.remove("strict");
                    }
                }
            }
        }
    }

    /// Handle response_format with tool calling
    fn handle_response_format(&self, request: &mut ChatRequest) {
        // Fireworks AI doesn't support tools and response_format together
        // If both are set, convert response_format to a tool
        if request.tools.is_some() && request.response_format.is_some() {
            // For now, prioritize tools over response_format
            // In a full implementation, we'd convert response_format to a tool
            debug!("Fireworks AI: tools and response_format both set, using tools");
        }

        // Transform json_schema format to json_object
        if let Some(ref mut format) = request.response_format {
            if format.format_type == "json_schema" && format.json_schema.is_some() {
                // Fireworks uses json_object with a schema field
                format.format_type = "json_object".to_string();
                // Keep the schema in json_schema field
            }
        }
    }

    /// Handle tool_choice mapping
    fn map_tool_choice(&self, request: &mut ChatRequest) {
        if let Some(ref mut tool_choice) = request.tool_choice {
            // Fireworks AI uses "any" instead of "required"
            match tool_choice {
                crate::core::types::tools::ToolChoice::String(s) if s == "required" => {
                    *s = "any".to_string();
                }
                _ => {}
            }
        }
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let url = format!("{}{}", self.config.get_api_base(), endpoint);

        let mut headers = Vec::with_capacity(2);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| ProviderError::network("fireworks", e.to_string()))?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("fireworks", e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&response_bytes);
            let status_code = status.as_u16();
            return Err(match status_code {
                401 => ProviderError::authentication("fireworks", "Invalid API key"),
                404 => ProviderError::model_not_found("fireworks", body_str.to_string()),
                429 => ProviderError::rate_limit("fireworks", None),
                400 => ProviderError::invalid_request("fireworks", body_str.to_string()),
                500..=599 => ProviderError::provider_unavailable("fireworks", body_str.to_string()),
                _ => HttpErrorMapper::map_status_code("fireworks", status_code, &body_str),
            });
        }

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::api_error("fireworks", 500, format!("Failed to parse response: {}", e))
        })
    }
}

#[async_trait]
impl LLMProvider for FireworksProvider {
    type Config = FireworksConfig;
    type Error = ProviderError;
    type ErrorMapper = crate::core::traits::error_mapper::DefaultErrorMapper;

    fn name(&self) -> &'static str {
        "fireworks"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        FIREWORKS_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        let has_tools = supports_function_calling(model);
        let has_tool_choice = supports_tool_choice(model);
        let has_reasoning = is_reasoning_model(model);

        // Base parameters supported by all models
        static BASE_PARAMS: &[&str] = &[
            "stream",
            "max_completion_tokens",
            "max_tokens",
            "temperature",
            "top_p",
            "top_k",
            "frequency_penalty",
            "presence_penalty",
            "n",
            "stop",
            "response_format",
            "user",
            "logprobs",
            "prompt_truncate_length",
            "context_length_exceeded_behavior",
        ];

        static WITH_TOOLS: &[&str] = &[
            "stream",
            "max_completion_tokens",
            "max_tokens",
            "temperature",
            "top_p",
            "top_k",
            "frequency_penalty",
            "presence_penalty",
            "n",
            "stop",
            "response_format",
            "user",
            "logprobs",
            "prompt_truncate_length",
            "context_length_exceeded_behavior",
            "tools",
        ];

        static WITH_TOOLS_AND_CHOICE: &[&str] = &[
            "stream",
            "max_completion_tokens",
            "max_tokens",
            "temperature",
            "top_p",
            "top_k",
            "frequency_penalty",
            "presence_penalty",
            "n",
            "stop",
            "response_format",
            "user",
            "logprobs",
            "prompt_truncate_length",
            "context_length_exceeded_behavior",
            "tools",
            "tool_choice",
        ];

        static WITH_REASONING: &[&str] = &[
            "stream",
            "max_completion_tokens",
            "max_tokens",
            "temperature",
            "top_p",
            "top_k",
            "frequency_penalty",
            "presence_penalty",
            "n",
            "stop",
            "response_format",
            "user",
            "logprobs",
            "prompt_truncate_length",
            "context_length_exceeded_behavior",
            "tools",
            "tool_choice",
            "reasoning_effort",
        ];

        if has_reasoning {
            WITH_REASONING
        } else if has_tool_choice {
            WITH_TOOLS_AND_CHOICE
        } else if has_tools {
            WITH_TOOLS
        } else {
            BASE_PARAMS
        }
    }

    async fn map_openai_params(
        &self,
        mut params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        // Map max_completion_tokens to max_tokens
        if let Some(max_completion_tokens) = params.remove("max_completion_tokens") {
            params.insert("max_tokens".to_string(), max_completion_tokens);
        }

        // Map tool_choice "required" to "any"
        if let Some(tool_choice) = params.get_mut("tool_choice") {
            if tool_choice.as_str() == Some("required") {
                *tool_choice = serde_json::json!("any");
            }
        }

        Ok(params)
    }

    async fn transform_request(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        // Format model ID
        request.model = format_model_id(&request.model);

        // Transform messages
        self.transform_messages(&mut request);

        // Transform tools
        self.transform_tools(&mut request);

        // Handle response_format
        self.handle_response_format(&mut request);

        // Map tool_choice
        self.map_tool_choice(&mut request);

        // Convert to JSON value
        serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("fireworks", e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let mut chat_response: ChatResponse =
            serde_json::from_slice(raw_response).map_err(|e| {
                ProviderError::api_error(
                    "fireworks",
                    500,
                    format!("Failed to parse response: {}", e),
                )
            })?;

        // Prefix model with provider name
        chat_response.model = format!("fireworks_ai/{}", chat_response.model);

        Ok(chat_response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        crate::core::traits::error_mapper::DefaultErrorMapper
    }

    async fn chat_completion(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Fireworks AI chat request: model={}", request.model);

        // Format model ID
        request.model = format_model_id(&request.model);

        // Transform request
        self.transform_messages(&mut request);
        self.transform_tools(&mut request);
        self.handle_response_format(&mut request);
        self.map_tool_choice(&mut request);

        // Execute request
        let request_json = serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("fireworks", e.to_string()))?;

        let response = self
            .execute_request("/chat/completions", request_json)
            .await?;

        let mut chat_response: ChatResponse = serde_json::from_value(response).map_err(|e| {
            ProviderError::api_error("fireworks", 500, format!("Failed to parse response: {}", e))
        })?;

        // Prefix model with provider name
        chat_response.model = format!("fireworks_ai/{}", chat_response.model);

        Ok(chat_response)
    }

    async fn chat_completion_stream(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Fireworks AI streaming request: model={}", request.model);

        // Format model ID
        request.model = format_model_id(&request.model);

        // Transform request
        self.transform_messages(&mut request);
        self.transform_tools(&mut request);
        self.handle_response_format(&mut request);
        self.map_tool_choice(&mut request);
        request.stream = true;

        // Get API configuration
        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| ProviderError::authentication("fireworks", "API key is required"))?;

        // Execute streaming request
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::network("fireworks", e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            let body_str = body.unwrap_or_default();
            return Err(match status {
                401 => ProviderError::authentication("fireworks", "Invalid API key"),
                404 => ProviderError::model_not_found("fireworks", &body_str),
                429 => ProviderError::rate_limit("fireworks", None),
                400 => ProviderError::invalid_request("fireworks", &body_str),
                500..=599 => ProviderError::provider_unavailable("fireworks", &body_str),
                _ => HttpErrorMapper::map_status_code("fireworks", status, &body_str),
            });
        }

        // Create SSE stream
        use futures::StreamExt;
        let byte_stream = response.bytes_stream();

        let stream = byte_stream.filter_map(|result| async move {
            match result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    // Parse SSE data
                    for line in text.lines() {
                        if let Some(parsed) = parse_fireworks_sse_line(line) {
                            return Some(parsed);
                        }
                    }
                    None
                }
                Err(e) => Some(Err(ProviderError::network("fireworks", e.to_string()))),
            }
        });

        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        // Fireworks AI supports embeddings through specific models
        // For now, return not supported
        Err(ProviderError::not_supported(
            "fireworks",
            "Fireworks AI embeddings require specific embedding models. Use nomic-embed-text-v1.5 or similar.",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        let url = format!("{}/models", self.config.get_api_base());
        let mut headers = Vec::with_capacity(1);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        match self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None::<serde_json::Value>)
            .await
        {
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
            ProviderError::model_not_found("fireworks", format!("Unknown model: {}", model))
        })?;

        let input_cost = (input_tokens as f64) * (model_info.input_cost_per_million / 1_000_000.0);
        let output_cost =
            (output_tokens as f64) * (model_info.output_cost_per_million / 1_000_000.0);
        Ok(input_cost + output_cost)
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::parse_fireworks_sse_line;

    #[test]
    fn test_parse_fireworks_sse_line_done_marker() {
        assert!(parse_fireworks_sse_line("data: [DONE]").is_none());
    }

    #[test]
    fn test_parse_fireworks_sse_line_valid_chunk() {
        let line = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"llama-v3p1-70b-instruct","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let parsed = parse_fireworks_sse_line(line).expect("expected a parsed chunk result");
        let chunk = parsed.expect("expected valid chat chunk");
        assert_eq!(chunk.id, "chatcmpl-123");
        assert_eq!(chunk.choices.len(), 1);
    }

    #[test]
    fn test_parse_fireworks_sse_line_invalid_chunk() {
        let parsed = parse_fireworks_sse_line("data: {invalid json")
            .expect("expected parse attempt for malformed chunk");
        assert!(parsed.is_err());
    }
}
