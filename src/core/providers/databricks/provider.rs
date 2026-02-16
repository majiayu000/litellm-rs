//! Databricks Provider Implementation
//!
//! Main provider implementation for Databricks Foundation Model APIs.

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpMethod, get_pricing_db, header, header_owned,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    error_mapper::trait_def::ErrorMapper, provider::ProviderConfig,
    provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    message::MessageContent,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{
        ChatChoice, ChatChunk, ChatResponse, EmbeddingData, EmbeddingResponse, FinishReason, Usage,
    },
};

use super::streaming::create_databricks_stream;
use super::{DatabricksConfig, DatabricksErrorMapper, get_databricks_registry};

#[derive(Debug, Clone)]
pub struct DatabricksProvider {
    config: DatabricksConfig,
    pool_manager: Arc<GlobalPoolManager>,
    supported_models: Vec<ModelInfo>,
}

impl DatabricksProvider {
    /// Generate headers for Databricks API requests
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(3);

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));

        // Add custom user agent
        let user_agent = DatabricksConfig::build_user_agent(None);
        headers.push(header("User-Agent", user_agent));

        // Add custom headers
        for (key, value) in &self.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }

    /// Create a new Databricks provider
    pub fn new(config: DatabricksConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("databricks", e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration("databricks", e.to_string()))?,
        );
        let supported_models = get_databricks_registry().models().to_vec();

        Ok(Self {
            config,
            pool_manager,
            supported_models,
        })
    }

    /// Create provider from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = DatabricksConfig::from_env();
        Self::new(config)
    }

    /// Create provider with credentials
    pub fn with_credentials(
        api_key: impl Into<String>,
        api_base: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let config = DatabricksConfig::with_credentials(api_key, api_base);
        Self::new(config)
    }

    /// Get the endpoint name from model
    fn get_endpoint_name(&self, model: &str) -> String {
        // Remove provider prefix if present
        let model_name = model.strip_prefix("databricks/").unwrap_or(model);

        model_name.to_string()
    }

    /// Build the full URL for a serving endpoint
    fn build_endpoint_url(
        &self,
        model: &str,
        _endpoint_type: &str,
    ) -> Result<String, ProviderError> {
        let base = self.config.get_serving_endpoint_base().ok_or_else(|| {
            ProviderError::configuration("databricks", "API base URL is required")
        })?;

        let endpoint_name = self.get_endpoint_name(model);

        Ok(format!("{}/{}/invocations", base, endpoint_name))
    }

    /// Transform chat request to Databricks format
    fn transform_chat_request_to_value(&self, request: &ChatRequest) -> Value {
        let registry = get_databricks_registry();
        let is_claude = registry.is_claude_model(&request.model);

        let mut body = serde_json::json!({
            "messages": self.transform_messages(&request.messages, is_claude),
        });

        // Add optional parameters
        if let Some(temperature) = request.temperature {
            body["temperature"] = serde_json::json!(temperature);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(n) = request.n {
            body["n"] = serde_json::json!(n);
        }
        if let Some(stop) = &request.stop {
            body["stop"] = serde_json::json!(stop);
        }
        if request.stream {
            body["stream"] = serde_json::json!(true);
        }

        // Tool calling (Claude on Databricks)
        if let Some(tools) = &request.tools {
            body["tools"] = serde_json::json!(tools);
        }
        if let Some(tool_choice) = &request.tool_choice {
            body["tool_choice"] = serde_json::json!(tool_choice);
        }

        body
    }

    /// Transform messages for Databricks
    fn transform_messages(&self, messages: &[ChatMessage], is_claude: bool) -> Vec<Value> {
        messages
            .iter()
            .map(|msg| {
                let mut message = serde_json::json!({
                    "role": msg.role.to_string(),
                });

                // Handle content based on type
                match &msg.content {
                    Some(MessageContent::Text(text)) => {
                        message["content"] = serde_json::json!(text);
                    }
                    Some(MessageContent::Parts(parts)) => {
                        if is_claude {
                            // Claude can handle multimodal content
                            let content_parts: Vec<Value> = parts
                                .iter()
                                .map(|part| serde_json::to_value(part).unwrap_or(Value::Null))
                                .collect();
                            message["content"] = serde_json::json!(content_parts);
                        } else {
                            // For non-Claude models, extract text only
                            let text: String = parts
                                .iter()
                                .filter_map(|part| {
                                    if let crate::core::types::content::ContentPart::Text {
                                        text,
                                        ..
                                    } = part
                                    {
                                        Some(text.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            message["content"] = serde_json::json!(text);
                        }
                    }
                    None => {
                        message["content"] = serde_json::json!("");
                    }
                }

                // Add tool calls if present
                if let Some(tool_calls) = &msg.tool_calls {
                    message["tool_calls"] = serde_json::json!(tool_calls);
                }
                if let Some(tool_call_id) = &msg.tool_call_id {
                    message["tool_call_id"] = serde_json::json!(tool_call_id);
                }
                if let Some(name) = &msg.name {
                    message["name"] = serde_json::json!(name);
                }

                message
            })
            .collect()
    }

    /// Parse Databricks chat response
    fn parse_chat_response(
        &self,
        response: &Value,
        model: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let id = response
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("chatcmpl-databricks")
            .to_string();

        let created = response
            .get("created")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp());

        let mut choices = Vec::new();

        if let Some(choices_array) = response.get("choices").and_then(|v| v.as_array()) {
            for choice in choices_array {
                let index = choice.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                let message = if let Some(msg) = choice.get("message") {
                    let role = msg
                        .get("role")
                        .and_then(|v| v.as_str())
                        .map(|r| match r {
                            "assistant" => crate::core::types::message::MessageRole::Assistant,
                            "user" => crate::core::types::message::MessageRole::User,
                            "system" => crate::core::types::message::MessageRole::System,
                            "tool" => crate::core::types::message::MessageRole::Tool,
                            _ => crate::core::types::message::MessageRole::Assistant,
                        })
                        .unwrap_or(crate::core::types::message::MessageRole::Assistant);

                    let content = msg
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| MessageContent::Text(s.to_string()));

                    ChatMessage {
                        role,
                        content,
                        thinking: None,
                        name: None,
                        tool_calls: msg
                            .get("tool_calls")
                            .and_then(|tc| serde_json::from_value(tc.clone()).ok()),
                        tool_call_id: None,
                        function_call: None,
                    }
                } else {
                    ChatMessage {
                        role: crate::core::types::message::MessageRole::Assistant,
                        content: None,
                        thinking: None,
                        name: None,
                        tool_calls: None,
                        tool_call_id: None,
                        function_call: None,
                    }
                };

                let finish_reason = choice
                    .get("finish_reason")
                    .and_then(|v| v.as_str())
                    .and_then(|r| match r {
                        "stop" => Some(FinishReason::Stop),
                        "length" => Some(FinishReason::Length),
                        "tool_calls" => Some(FinishReason::ToolCalls),
                        "content_filter" => Some(FinishReason::ContentFilter),
                        _ => None,
                    });

                choices.push(ChatChoice {
                    index,
                    message,
                    finish_reason,
                    logprobs: None,
                });
            }
        }

        let usage = response.get("usage").map(|u| Usage {
            prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            completion_tokens: u
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        });

        Ok(ChatResponse {
            id,
            object: "chat.completion".to_string(),
            created,
            model: model.to_string(),
            choices,
            usage,
            system_fingerprint: None,
        })
    }
}

#[async_trait]
impl LLMProvider for DatabricksProvider {
    type Config = DatabricksConfig;
    type Error = ProviderError;
    type ErrorMapper = DatabricksErrorMapper;

    fn name(&self) -> &'static str {
        "databricks"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::Embeddings,
            ProviderCapability::ToolCalling,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        get_databricks_registry().get_supported_params(model)
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        let registry = get_databricks_registry();
        let supported = registry.get_supported_params(model);

        // Filter to only supported parameters
        let mapped: HashMap<String, Value> = params
            .into_iter()
            .filter(|(key, _)| supported.contains(&key.as_str()))
            .collect();

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        Ok(self.transform_chat_request_to_value(&request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response: Value = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("databricks", e.to_string()))?;

        self.parse_chat_response(&response, model)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        DatabricksErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        let url = self.build_endpoint_url(&request.model, "chat")?;
        let body = self.transform_chat_request_to_value(&request);

        let headers = self.get_request_headers();

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("databricks", e.to_string()))?;

        self.transform_response(&response_bytes, &request.model, &context.request_id)
            .await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        let url = self.build_endpoint_url(&request.model, "chat")?;

        // Create streaming request
        let mut body = self.transform_chat_request_to_value(&request);
        body["stream"] = serde_json::Value::Bool(true);

        // Get API key
        let api_key =
            self.config.base.api_key.as_ref().ok_or_else(|| {
                ProviderError::authentication("databricks", "API key is required")
            })?;

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .header("User-Agent", DatabricksConfig::build_user_agent(None))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("databricks", e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(self
                .get_error_mapper()
                .map_http_error(status.as_u16(), &error_text));
        }

        let stream = response.bytes_stream();
        Ok(Box::pin(create_databricks_stream(stream)))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        let url = self.build_endpoint_url(&request.model, "embeddings")?;

        // Build request body
        let body = serde_json::json!({
            "input": request.input,
        });

        let api_key =
            self.config.base.api_key.as_ref().ok_or_else(|| {
                ProviderError::authentication("databricks", "API key is required")
            })?;

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .header("User-Agent", DatabricksConfig::build_user_agent(None))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("databricks", e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(self
                .get_error_mapper()
                .map_http_error(status.as_u16(), &error_text));
        }

        let response_body: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("databricks", e.to_string()))?;

        // Parse embedding response
        let mut data = Vec::new();

        // Handle both Databricks-style and OpenAI-style responses
        if let Some(embeddings) = response_body.get("data").and_then(|v| v.as_array()) {
            for (i, embedding_obj) in embeddings.iter().enumerate() {
                if let Some(embedding) = embedding_obj.get("embedding").and_then(|v| v.as_array()) {
                    let vec: Vec<f32> = embedding
                        .iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect();

                    data.push(EmbeddingData {
                        object: "embedding".to_string(),
                        index: i as u32,
                        embedding: vec,
                    });
                }
            }
        } else if let Some(embedding) = response_body.get("embedding").and_then(|v| v.as_array()) {
            // Single embedding response
            let vec: Vec<f32> = embedding
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();

            data.push(EmbeddingData {
                object: "embedding".to_string(),
                index: 0,
                embedding: vec,
            });
        }

        let usage = response_body.get("usage").map(|u| Usage {
            prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            completion_tokens: 0,
            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        });

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data,
            model: request.model,
            usage,
            embeddings: None,
        })
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.base.api_key.is_some() && self.config.base.api_base.is_some() {
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
    ) -> Result<f64, Self::Error> {
        let usage = crate::core::providers::base::pricing::Usage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
            reasoning_tokens: None,
        };

        Ok(get_pricing_db().calculate(model, &usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_databricks_provider_name() {
        let config =
            DatabricksConfig::with_credentials("dapi-test-key", "https://test.databricks.net");
        let provider = DatabricksProvider::new(config).unwrap();
        assert_eq!(provider.name(), "databricks");
    }

    #[test]
    fn test_databricks_provider_capabilities() {
        let config =
            DatabricksConfig::with_credentials("dapi-test-key", "https://test.databricks.net");
        let provider = DatabricksProvider::new(config).unwrap();
        let caps = provider.capabilities();
        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::Embeddings));
    }

    #[test]
    fn test_databricks_provider_models() {
        let config =
            DatabricksConfig::with_credentials("dapi-test-key", "https://test.databricks.net");
        let provider = DatabricksProvider::new(config).unwrap();
        let models = provider.models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_get_endpoint_name() {
        let config =
            DatabricksConfig::with_credentials("dapi-test-key", "https://test.databricks.net");
        let provider = DatabricksProvider::new(config).unwrap();

        assert_eq!(
            provider.get_endpoint_name("databricks/dbrx-instruct"),
            "dbrx-instruct"
        );
        assert_eq!(provider.get_endpoint_name("llama-3-70b"), "llama-3-70b");
    }

    #[test]
    fn test_build_endpoint_url() {
        let config =
            DatabricksConfig::with_credentials("dapi-test-key", "https://test.databricks.net");
        let provider = DatabricksProvider::new(config).unwrap();

        let url = provider
            .build_endpoint_url("dbrx-instruct", "chat")
            .unwrap();
        assert!(url.contains("/serving-endpoints/"));
        assert!(url.contains("dbrx-instruct"));
        assert!(url.ends_with("/invocations"));
    }

    #[test]
    fn test_transform_messages() {
        let config =
            DatabricksConfig::with_credentials("dapi-test-key", "https://test.databricks.net");
        let provider = DatabricksProvider::new(config).unwrap();

        let messages = vec![ChatMessage {
            role: crate::core::types::message::MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            thinking: None,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
        }];

        let transformed = provider.transform_messages(&messages, false);
        assert_eq!(transformed.len(), 1);
        assert_eq!(transformed[0]["role"], "user");
        assert_eq!(transformed[0]["content"], "Hello");
    }

    #[test]
    fn test_transform_chat_request() {
        let config =
            DatabricksConfig::with_credentials("dapi-test-key", "https://test.databricks.net");
        let provider = DatabricksProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "dbrx-instruct".to_string(),
            messages: vec![ChatMessage {
                role: crate::core::types::message::MessageRole::User,
                content: Some(MessageContent::Text("Test".to_string())),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            ..Default::default()
        };

        let body = provider.transform_chat_request_to_value(&request);
        assert!(body.get("messages").is_some());
        assert!((body["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
        assert_eq!(body["max_tokens"], 100);
    }

    #[test]
    fn test_parse_chat_response() {
        let config =
            DatabricksConfig::with_credentials("dapi-test-key", "https://test.databricks.net");
        let provider = DatabricksProvider::new(config).unwrap();

        let response_json = serde_json::json!({
            "id": "chatcmpl-123",
            "created": 1700000000,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        });

        let response = provider
            .parse_chat_response(&response_json, "dbrx-instruct")
            .unwrap();
        assert_eq!(response.id, "chatcmpl-123");
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].finish_reason, Some(FinishReason::Stop));
        assert!(response.usage.is_some());
    }

    #[test]
    fn test_health_check() {
        let config =
            DatabricksConfig::with_credentials("dapi-test-key", "https://test.databricks.net");
        let provider = DatabricksProvider::new(config).unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let health = rt.block_on(provider.health_check());
        assert_eq!(health, HealthStatus::Healthy);
    }

    #[test]
    fn test_health_check_unhealthy() {
        let mut config =
            DatabricksConfig::with_credentials("dapi-test-key", "https://test.databricks.net");
        config.base.api_base = None;

        // This will fail validation, so we construct manually for testing
        let provider = DatabricksProvider {
            config,
            pool_manager: Arc::new(GlobalPoolManager::new().unwrap()),
            supported_models: vec![],
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let health = rt.block_on(provider.health_check());
        assert_eq!(health, HealthStatus::Unhealthy);
    }
}
