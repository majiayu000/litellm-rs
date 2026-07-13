//! Amazon Nova Provider Implementation
//!
//! Main provider implementation for Amazon Nova multimodal models
//! Amazon Nova uses OpenAI-compatible API format

use serde_json::Value;
use std::collections::HashMap;

use crate::core::providers::base::{
    HeaderPair, HttpMethod, get_pricing_db, header, read_streaming_error_body,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChoice, ChatResponse, Usage},
};

use super::{AmazonNovaConfig, AmazonNovaModelRegistry};

const PROVIDER_NAME: &str = "amazon_nova";
const CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

fn build_model_info() -> Vec<ModelInfo> {
    let registry = AmazonNovaModelRegistry::new();
    registry
        .list_models()
        .iter()
        .map(|m| ModelInfo {
            id: m.id.clone(),
            name: m.name.clone(),
            provider: PROVIDER_NAME.to_string(),
            max_context_length: m.context_length,
            max_output_length: Some(m.max_output_tokens),
            supports_streaming: m.supports_streaming,
            supports_tools: m.supports_tools,
            supports_multimodal: m.supports_vision,
            input_cost_per_1k_tokens: Some(m.input_cost_per_1k),
            output_cost_per_1k_tokens: Some(m.output_cost_per_1k),
            currency: "USD".to_string(),
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            created_at: None,
            updated_at: None,
            metadata: std::collections::HashMap::new(),
        })
        .collect()
}

crate::define_pooled_http_provider_with_hooks!(
    provider: PROVIDER_NAME,
    struct_name: AmazonNovaProvider,
    config: super::AmazonNovaConfig,
    error_mapper: super::AmazonNovaErrorMapper,
    model_info: build_model_info,
    capabilities: CAPABILITIES,
    url_builder: |provider: &AmazonNovaProvider| -> String { provider.config.get_chat_endpoint() },
    http_method: HttpMethod::POST,
    supported_params: [
        "max_tokens",
        "max_completion_tokens",
        "temperature",
        "top_p",
        "stop",
        "stream",
        "stream_options",
        "tools",
        "tool_choice",
        "reasoning_effort",
        "metadata",
    ],
    build_headers: |provider: &AmazonNovaProvider| -> Vec<HeaderPair> {
        provider.get_request_headers()
    },
    with_api_key: |api_key: String| -> Result<AmazonNovaProvider, ProviderError> {
        AmazonNovaProvider::new(AmazonNovaConfig::with_api_key(api_key))
    },
    map_openai_params: |_provider: &AmazonNovaProvider,
                        params: HashMap<String, Value>,
                        _model: &str|
     -> Result<HashMap<String, Value>, ProviderError> { Ok(params) },
    request_transform: |provider: &AmazonNovaProvider, request: ChatRequest|
     -> Result<Value, ProviderError> { Ok(provider.transform_chat_request(request)) },
    response_transform: |provider: &AmazonNovaProvider,
                         raw_response: &[u8],
                         model: &str,
                         request_id: &str|
     -> Result<ChatResponse, ProviderError> {
        let response_data: Value = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))?;
        provider.transform_chat_response(response_data, model, request_id)
    },
    error_map: |_provider: &AmazonNovaProvider,
                status: u16,
                error_text: String,
                _request: &ChatRequest|
     -> ProviderError { ProviderError::api_error(PROVIDER_NAME, status, error_text) },
    health_check: |provider: &AmazonNovaProvider| {
        let has_key = provider.config.get_api_key().is_some();
        async move {
            if has_key {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            }
        }
    },
    streaming: |provider: &AmazonNovaProvider, request: ChatRequest, _context: RequestContext| {
        let url = provider.config.get_chat_endpoint();
        let api_key = provider.config.get_api_key().map(|key| key.to_string());
        let client = provider.streaming_client.clone();
        let header_timeout = provider.config.base.timeout_duration();

        let mut body = provider.transform_chat_request(request);
        body["stream"] = serde_json::Value::Bool(true);

        async move {
            let api_key = api_key.ok_or_else(|| {
                ProviderError::authentication(PROVIDER_NAME, "API key is required")
            })?;

            let request = client
                .post(&url)?
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body);
            let response = tokio::time::timeout(header_timeout, request.send())
                .await
                .map_err(|_| {
                    ProviderError::timeout(
                        PROVIDER_NAME,
                        format!(
                            "streaming request did not receive response headers within {header_timeout:?}"
                        ),
                    )
                })?
                .map_err(|error| ProviderError::network(PROVIDER_NAME, error.to_string()))?;

            let status = response.status();
            if !status.is_success() {
                let error_text = read_streaming_error_body(response)
                    .await
                    .map_err(|error| error.into_provider_error(PROVIDER_NAME))?;
                return Err(ProviderError::api_error(
                    PROVIDER_NAME,
                    status.as_u16(),
                    error_text,
                ));
            }

            use crate::core::providers::base::sse::{
                OpenAICompatibleTransformer, UnifiedSSEStream,
            };
            let stream = response.bytes_stream();
            let transformer = OpenAICompatibleTransformer::new(PROVIDER_NAME);
            let stream: std::pin::Pin<
                Box<
                    dyn futures::Stream<Item = Result<crate::core::types::responses::ChatChunk, ProviderError>>
                        + Send,
                >,
            > = Box::pin(UnifiedSSEStream::new(stream, transformer));
            Ok(stream)
        }
    },
    calculate_cost: |provider: &AmazonNovaProvider,
                     model: &str,
                     input_tokens: u32,
                     output_tokens: u32|
     -> Result<f64, ProviderError> {
        let normalized_model = provider.normalize_model_name(model);
        if let Some(info) = provider
            .supported_models
            .iter()
            .find(|m| m.id == normalized_model)
        {
            let input_cost = info
                .input_cost_per_1k_tokens
                .unwrap_or(0.0)
                * input_tokens as f64
                / 1000.0;
            let output_cost = info
                .output_cost_per_1k_tokens
                .unwrap_or(0.0)
                * output_tokens as f64
                / 1000.0;
            return Ok(input_cost + output_cost);
        }

        let usage = crate::core::pricing::Usage::new(input_tokens, output_tokens);

        Ok(get_pricing_db().calculate(model, &usage))
    },
);

impl AmazonNovaProvider {
    /// Create provider from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = AmazonNovaConfig::from_env();
        Self::new(config)
    }

    /// Transform ChatRequest to Amazon Nova format (OpenAI compatible)
    fn transform_chat_request(&self, request: ChatRequest) -> Value {
        let mut body = serde_json::json!({
            "model": self.normalize_model_name(&request.model),
            "messages": request.messages,
        });

        // Handle max_tokens with preference for max_completion_tokens
        if let Some(max_completion_tokens) = request.max_completion_tokens {
            body["max_tokens"] = serde_json::json!(max_completion_tokens);
        } else if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }

        if let Some(stop) = &request.stop {
            body["stop"] = serde_json::json!(stop);
        }

        if request.stream {
            body["stream"] = serde_json::json!(true);
        }

        if let Some(tools) = &request.tools {
            body["tools"] = serde_json::json!(tools);
        }

        if let Some(tool_choice) = &request.tool_choice {
            body["tool_choice"] = serde_json::json!(tool_choice);
        }

        if let Some(reasoning_effort) = &request.reasoning_effort {
            body["reasoning_effort"] = serde_json::json!(reasoning_effort);
        }

        body
    }

    /// Normalize model name to full format
    fn normalize_model_name(&self, model: &str) -> String {
        // If already fully qualified, return as-is
        if model.starts_with("amazon.nova") {
            return model.to_string();
        }

        // Map short names to full names
        match model {
            "nova-2-lite" => "amazon.nova-2-lite-v1:0".to_string(),
            "nova-pro" => "amazon.nova-pro-v1:0".to_string(),
            "nova-lite" => "amazon.nova-lite-v1:0".to_string(),
            "nova-micro" => "amazon.nova-micro-v1:0".to_string(),
            "nova-premier" => "amazon.nova-premier-v1:0".to_string(),
            _ => model.to_string(),
        }
    }

    /// Transform Amazon Nova response to ChatResponse
    fn transform_chat_response(
        &self,
        response_data: Value,
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let id = response_data
            .get("id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| request_id.to_string());

        let created = response_data
            .get("created")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as i64;

        let response_model = response_data
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("amazon-nova/{}", model));

        // Parse choices
        let choices: Vec<ChatChoice> = response_data
            .get("choices")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        // Parse usage
        let usage: Option<Usage> = response_data
            .get("usage")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        Ok(ChatResponse {
            id,
            object: "chat.completion".to_string(),
            created,
            model: response_model,
            choices,
            usage,
            system_fingerprint: response_data
                .get("system_fingerprint")
                .and_then(|v| v.as_str())
                .map(String::from),
        })
    }

    /// Generate request headers for Amazon Nova API
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(2);

        if let Some(api_key) = self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        headers.push(header("Content-Type", "application/json".to_string()));
        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
    use crate::core::types::chat::ChatMessage;
    use crate::core::types::message::{MessageContent, MessageRole};

    #[test]
    fn test_provider_creation_fails_without_api_key() {
        let config = AmazonNovaConfig::default();
        let result = AmazonNovaProvider::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_creation_with_api_key() {
        let config = AmazonNovaConfig::with_api_key("test-key");
        let result = AmazonNovaProvider::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_policy_clients_honor_endpoint_access() {
        let mut public = AmazonNovaConfig::with_api_key("test-key");
        public.base.api_base = Some("http://127.0.0.1:8080/v1".to_string());
        assert!(AmazonNovaProvider::new(public).is_err());

        let mut private = AmazonNovaConfig::with_api_key("test-key");
        private.base.api_base = Some("http://127.0.0.1:8080/v1".to_string());
        private.base.endpoint_access = crate::core::net::ProviderEndpointAccess::PrivateNetwork;
        assert!(AmazonNovaProvider::new(private).is_ok());
    }

    #[tokio::test]
    async fn test_streaming_header_wait_uses_config_timeout() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("streaming listener should bind");
        let address = listener
            .local_addr()
            .expect("streaming listener address should exist");
        let server = tokio::spawn(async move {
            let _connection = listener.accept().await.expect("request should connect");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        });
        let mut config = AmazonNovaConfig::with_api_key("test-key");
        config.base.api_base = Some(format!("http://{address}/v1"));
        config.base.endpoint_access = crate::core::net::ProviderEndpointAccess::PrivateNetwork;
        config.base.timeout = 1;
        let provider = AmazonNovaProvider::new(config).expect("private provider should build");
        let request = ChatRequest {
            model: "nova-pro".to_string(),
            stream: true,
            ..Default::default()
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(1500),
            LLMProvider::chat_completion_stream(&provider, request, RequestContext::default()),
        )
        .await
        .expect("configured header timeout should terminate the request");

        assert!(matches!(result, Err(ProviderError::Timeout { .. })));
        server.abort();
    }

    #[test]
    fn test_provider_name() {
        let config = AmazonNovaConfig::with_api_key("test-key");
        let provider = AmazonNovaProvider::new(config).unwrap();
        assert_eq!(provider.name(), "amazon_nova");
    }

    #[test]
    fn test_provider_capabilities() {
        let config = AmazonNovaConfig::with_api_key("test-key");
        let provider = AmazonNovaProvider::new(config).unwrap();
        let caps = provider.capabilities();
        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::ToolCalling));
    }

    #[test]
    fn test_provider_models() {
        let config = AmazonNovaConfig::with_api_key("test-key");
        let provider = AmazonNovaProvider::new(config).unwrap();
        let models = provider.models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_normalize_model_name() {
        let config = AmazonNovaConfig::with_api_key("test-key");
        let provider = AmazonNovaProvider::new(config).unwrap();

        assert_eq!(
            provider.normalize_model_name("nova-pro"),
            "amazon.nova-pro-v1:0"
        );
        assert_eq!(
            provider.normalize_model_name("nova-2-lite"),
            "amazon.nova-2-lite-v1:0"
        );
        assert_eq!(
            provider.normalize_model_name("nova-lite"),
            "amazon.nova-lite-v1:0"
        );
        assert_eq!(
            provider.normalize_model_name("amazon.nova-pro-v1:0"),
            "amazon.nova-pro-v1:0"
        );
    }

    #[test]
    fn test_transform_chat_request() {
        let config = AmazonNovaConfig::with_api_key("test-key");
        let provider = AmazonNovaProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "nova-pro".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                ..Default::default()
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            ..Default::default()
        };

        let body = provider.transform_chat_request(request);
        assert_eq!(body["model"], "amazon.nova-pro-v1:0");
        let temp = body["temperature"].as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.01);
        assert_eq!(body["max_tokens"], 100);
    }

    #[test]
    fn test_transform_chat_request_with_max_completion_tokens() {
        let config = AmazonNovaConfig::with_api_key("test-key");
        let provider = AmazonNovaProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "nova-pro".to_string(),
            messages: vec![],
            max_tokens: Some(100),
            max_completion_tokens: Some(200), // Should take precedence
            reasoning_effort: Some("high".to_string()),
            ..Default::default()
        };

        let body = provider.transform_chat_request(request);
        assert_eq!(body["max_tokens"], 200);
        assert_eq!(body["reasoning_effort"], "high");
    }

    #[test]
    fn test_transform_chat_response() {
        let config = AmazonNovaConfig::with_api_key("test-key");
        let provider = AmazonNovaProvider::new(config).unwrap();

        let response_data = serde_json::json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677858242,
            "model": "amazon.nova-pro-v1:0",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello! How can I help you?"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            }
        });

        let response = provider.transform_chat_response(response_data, "nova-pro", "req-123");
        assert!(response.is_ok());
        assert_eq!(response.unwrap().id, "chatcmpl-123");
    }
}
