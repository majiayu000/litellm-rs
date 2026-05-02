//! Main Cloudflare Workers AI Provider Implementation
//!
//! Implements the LLMProvider trait for Cloudflare's Workers AI models.

use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::CloudflareConfig;
use super::model_info::{calculate_cost, get_available_models, get_model_info};
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header};
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
    responses::{ChatChunk, ChatResponse, EmbeddingResponse, FinishReason},
};

/// Static capabilities for Cloudflare provider
const CLOUDFLARE_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
];

/// Cloudflare Workers AI provider implementation
#[derive(Debug, Clone)]
pub struct CloudflareProvider {
    config: CloudflareConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl CloudflareProvider {
    /// Create a new Cloudflare provider instance
    pub async fn new(config: CloudflareConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("cloudflare", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                "cloudflare",
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        // Build model list from static configuration
        let models = get_available_models()
            .iter()
            .filter_map(|id| get_model_info(id))
            .map(|info| {
                let capabilities = vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ];

                ModelInfo {
                    id: format!("cloudflare/{}", info.model_id),
                    name: info.display_name.to_string(),
                    provider: "cloudflare".to_string(),
                    max_context_length: info.max_context_length,
                    max_output_length: Some(info.max_output_length),
                    supports_streaming: info.supports_streaming,
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

    /// Create provider with account ID and token
    pub async fn with_credentials(
        account_id: impl Into<String>,
        api_token: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let config = CloudflareConfig {
            account_id: Some(account_id.into()),
            api_token: Some(api_token.into()),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let account_id = self
            .config
            .get_account_id()
            .ok_or_else(|| ProviderError::configuration("cloudflare", "Account ID is required"))?;

        let url = format!(
            "{}/accounts/{}/ai/run/{}",
            self.config.get_api_base(),
            account_id,
            endpoint
        );

        let mut headers = Vec::with_capacity(2);
        if let Some(api_token) = self.config.get_api_token() {
            headers.push(header("Authorization", format!("Bearer {}", api_token)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| ProviderError::network("cloudflare", e.to_string()))?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("cloudflare", e.to_string()))?;

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::api_error(
                "cloudflare",
                500,
                format!("Failed to parse response: {}", e),
            )
        })
    }

    /// Transform OpenAI-style request to Cloudflare format
    fn transform_to_cloudflare_format(&self, request: &ChatRequest) -> serde_json::Value {
        // Cloudflare uses a simpler format
        let mut messages = Vec::new();
        for msg in &request.messages {
            let mut message = serde_json::json!({
                "role": msg.role.to_string().to_lowercase(),
            });

            if let Some(ref content) = msg.content {
                message["content"] = serde_json::json!(content.to_string());
            }

            messages.push(message);
        }

        let mut body = serde_json::json!({
            "messages": messages,
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

        if request.stream {
            body["stream"] = serde_json::json!(true);
        }

        body
    }
}

impl LLMProvider for CloudflareProvider {
    fn name(&self) -> &'static str {
        "cloudflare"
    }

    fn error_provider_name(&self) -> &'static str {
        "cloudflare"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        CLOUDFLARE_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "temperature",
            "top_p",
            "max_tokens",
            "stream",
            "stop",
            "frequency_penalty",
            "presence_penalty",
            "n",
            "seed",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, ProviderError> {
        // Cloudflare supports a subset of OpenAI parameters directly
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, ProviderError> {
        Ok(self.transform_to_cloudflare_format(&request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let cloudflare_response: serde_json::Value =
            serde_json::from_slice(raw_response).map_err(|e| {
                ProviderError::api_error(
                    "cloudflare",
                    500,
                    format!("Failed to parse response: {}", e),
                )
            })?;

        // Transform Cloudflare response to OpenAI format
        let content = cloudflare_response["result"]["response"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(ChatResponse {
            id: request_id.to_string(),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![crate::core::types::responses::ChatChoice {
                index: 0,
                message: crate::core::types::chat::ChatMessage {
                    role: crate::core::types::message::MessageRole::Assistant,
                    content: Some(crate::core::types::message::MessageContent::Text(content)),
                    thinking: None,
                    name: None,
                    function_call: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: None, // Cloudflare doesn't provide usage stats
            system_fingerprint: None,
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
        debug!("Cloudflare chat request: model={}", request.model);

        // Remove cloudflare/ prefix if present
        let model = request
            .model
            .strip_prefix("cloudflare/")
            .unwrap_or(&request.model);

        // Transform request
        let cloudflare_request = self.transform_to_cloudflare_format(&request);

        // Execute request
        let response = self.execute_request(model, cloudflare_request).await?;

        // Transform response
        let content = response["result"]["response"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let request_id = uuid::Uuid::new_v4().to_string();

        Ok(ChatResponse {
            id: request_id.clone(),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: request.model.clone(),
            choices: vec![crate::core::types::responses::ChatChoice {
                index: 0,
                message: crate::core::types::chat::ChatMessage {
                    role: crate::core::types::message::MessageRole::Assistant,
                    content: Some(crate::core::types::message::MessageContent::Text(content)),
                    thinking: None,
                    name: None,
                    function_call: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        })
    }

    async fn chat_completion_stream(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        debug!("Cloudflare streaming request: model={}", request.model);

        // Set streaming flag
        request.stream = true;

        // NOTE: SSE streaming for Cloudflare not yet implemented
        // For now, return an error as streaming implementation needs more work
        Err(ProviderError::not_supported(
            "cloudflare",
            "Streaming is not yet fully implemented for Cloudflare provider",
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        // Cloudflare also supports embeddings models, but we'll implement text generation first
        Err(ProviderError::not_supported(
            "cloudflare",
            "Embeddings are not yet implemented for Cloudflare provider",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Simple health check - try to list models
        let account_id = match self.config.get_account_id() {
            Some(id) => id,
            None => return HealthStatus::Unhealthy,
        };

        let url = format!(
            "{}/accounts/{}/ai/models/search",
            self.config.get_api_base(),
            account_id
        );

        let mut headers = Vec::with_capacity(1);
        if let Some(api_token) = self.config.get_api_token() {
            headers.push(header("Authorization", format!("Bearer {}", api_token)));
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
    ) -> Result<f64, ProviderError> {
        calculate_cost(model, input_tokens, output_tokens)
            .ok_or_else(|| ProviderError::model_not_found("cloudflare", model))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    fn create_test_config() -> CloudflareConfig {
        CloudflareConfig {
            account_id: Some("test_account".to_string()),
            api_token: Some("test_token".to_string()),
            ..Default::default()
        }
    }

    async fn create_test_provider() -> CloudflareProvider {
        CloudflareProvider::new(create_test_config()).await.unwrap()
    }

    // ==================== Provider Creation Tests ====================

    #[tokio::test]
    async fn test_provider_creation() {
        let config = CloudflareConfig {
            account_id: Some("test_account".to_string()),
            api_token: Some("test_token".to_string()),
            ..Default::default()
        };

        let provider = CloudflareProvider::new(config).await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "cloudflare");
        assert!(!provider.models().is_empty());
    }

    #[tokio::test]
    async fn test_provider_creation_with_custom_api_base() {
        let config = CloudflareConfig {
            account_id: Some("test_account".to_string()),
            api_token: Some("test_token".to_string()),
            api_base: Some("https://custom.cloudflare.com".to_string()),
            ..Default::default()
        };

        let provider = CloudflareProvider::new(config).await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_provider_with_credentials_factory() {
        let provider = CloudflareProvider::with_credentials("account123", "token456").await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "cloudflare");
    }

    #[tokio::test]
    async fn test_provider_without_credentials() {
        let config = CloudflareConfig {
            account_id: None,
            api_token: None,
            ..Default::default()
        };

        let provider = CloudflareProvider::new(config).await;
        assert!(provider.is_err());
    }

    #[tokio::test]
    async fn test_provider_without_account_id() {
        let config = CloudflareConfig {
            account_id: None,
            api_token: Some("test_token".to_string()),
            ..Default::default()
        };

        let provider = CloudflareProvider::new(config).await;
        assert!(provider.is_err());
    }

    #[tokio::test]
    async fn test_provider_without_api_token() {
        let config = CloudflareConfig {
            account_id: Some("test_account".to_string()),
            api_token: None,
            ..Default::default()
        };

        let provider = CloudflareProvider::new(config).await;
        assert!(provider.is_err());
    }

    // ==================== Provider Capabilities Tests ====================

    #[test]
    fn test_capabilities() {
        assert!(CLOUDFLARE_CAPABILITIES.contains(&ProviderCapability::ChatCompletion));
        assert!(CLOUDFLARE_CAPABILITIES.contains(&ProviderCapability::ChatCompletionStream));
        assert_eq!(CLOUDFLARE_CAPABILITIES.len(), 2);
    }

    #[tokio::test]
    async fn test_provider_name() {
        let provider = create_test_provider().await;
        assert_eq!(provider.name(), "cloudflare");
    }

    #[tokio::test]
    async fn test_provider_capabilities_method() {
        let provider = create_test_provider().await;
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    }

    #[tokio::test]
    async fn test_provider_supported_openai_params() {
        let provider = create_test_provider().await;
        let params = provider.get_supported_openai_params("@cf/meta/llama-3-8b-instruct");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"top_p"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"stream"));
        assert!(params.contains(&"stop"));
        assert!(params.contains(&"frequency_penalty"));
        assert!(params.contains(&"presence_penalty"));
        assert!(params.contains(&"n"));
        assert!(params.contains(&"seed"));
    }

    #[tokio::test]
    async fn test_provider_models_not_empty() {
        let provider = create_test_provider().await;
        assert!(!provider.models().is_empty());
    }

    #[tokio::test]
    async fn test_provider_models_have_cloudflare_prefix() {
        let provider = create_test_provider().await;
        for model in provider.models() {
            assert!(
                model.id.starts_with("cloudflare/"),
                "Model {} should start with cloudflare/",
                model.id
            );
            assert_eq!(model.provider, "cloudflare");
        }
    }

    // ==================== Transform Request Tests ====================

    #[test]
    fn test_transform_request() {
        let config = CloudflareConfig {
            account_id: Some("test".to_string()),
            api_token: Some("test".to_string()),
            ..Default::default()
        };

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let provider = runtime.block_on(CloudflareProvider::new(config)).unwrap();

        let request = ChatRequest {
            model: "@cf/meta/llama-3-8b-instruct".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                thinking: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            stream: false,
            ..Default::default()
        };

        let transformed = provider.transform_to_cloudflare_format(&request);
        assert!(transformed["messages"].is_array());
        let temp_value = transformed["temperature"].as_f64().unwrap();
        assert!(
            (temp_value - 0.7).abs() < 1e-6,
            "Expected 0.7, got {}",
            temp_value
        );
        assert_eq!(transformed["max_tokens"], 100);
    }

    #[tokio::test]
    async fn test_transform_request_with_top_p() {
        let provider = create_test_provider().await;

        let request = ChatRequest {
            model: "@cf/meta/llama-3-8b-instruct".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            top_p: Some(0.9),
            ..Default::default()
        };

        let transformed = provider.transform_to_cloudflare_format(&request);
        let top_p_value = transformed["top_p"].as_f64().unwrap();
        assert!((top_p_value - 0.9).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_transform_request_with_streaming() {
        let provider = create_test_provider().await;

        let request = ChatRequest {
            model: "@cf/meta/llama-3-8b-instruct".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            stream: true,
            ..Default::default()
        };

        let transformed = provider.transform_to_cloudflare_format(&request);
        assert_eq!(transformed["stream"], true);
    }

    #[tokio::test]
    async fn test_transform_request_multiple_messages() {
        let provider = create_test_provider().await;

        let request = ChatRequest {
            model: "@cf/meta/llama-3-8b-instruct".to_string(),
            messages: vec![
                ChatMessage {
                    role: MessageRole::System,
                    content: Some(MessageContent::Text(
                        "You are a helpful assistant.".to_string(),
                    )),
                    ..Default::default()
                },
                ChatMessage {
                    role: MessageRole::User,
                    content: Some(MessageContent::Text("Hello".to_string())),
                    ..Default::default()
                },
                ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text("Hi there!".to_string())),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let transformed = provider.transform_to_cloudflare_format(&request);
        let messages = transformed["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 3);
    }

    #[tokio::test]
    async fn test_transform_request_no_optional_params() {
        let provider = create_test_provider().await;

        let request = ChatRequest {
            model: "@cf/meta/llama-3-8b-instruct".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            ..Default::default()
        };

        let transformed = provider.transform_to_cloudflare_format(&request);
        assert!(transformed["messages"].is_array());
        assert!(transformed.get("temperature").is_none() || transformed["temperature"].is_null());
        assert!(transformed.get("max_tokens").is_none() || transformed["max_tokens"].is_null());
    }

    // ==================== Transform Response Tests ====================

    #[tokio::test]
    async fn test_transform_response_success() {
        let provider = create_test_provider().await;

        let response_json = serde_json::json!({
            "result": {
                "response": "Hello! I'm doing well, thank you for asking."
            },
            "success": true
        });
        let response_bytes = serde_json::to_vec(&response_json).unwrap();

        let result = provider
            .transform_response(
                &response_bytes,
                "@cf/meta/llama-3-8b-instruct",
                "test-request-id",
            )
            .await;

        assert!(result.is_ok());
        let chat_response = result.unwrap();
        assert_eq!(chat_response.id, "test-request-id");
        assert_eq!(chat_response.model, "@cf/meta/llama-3-8b-instruct");
        assert!(!chat_response.choices.is_empty());
    }

    #[tokio::test]
    async fn test_transform_response_empty_content() {
        let provider = create_test_provider().await;

        let response_json = serde_json::json!({
            "result": {},
            "success": true
        });
        let response_bytes = serde_json::to_vec(&response_json).unwrap();

        let result = provider
            .transform_response(
                &response_bytes,
                "@cf/meta/llama-3-8b-instruct",
                "test-request-id",
            )
            .await;

        assert!(result.is_ok());
        let chat_response = result.unwrap();
        assert!(!chat_response.choices.is_empty());
    }

    #[tokio::test]
    async fn test_transform_response_invalid_json() {
        let provider = create_test_provider().await;

        let response_bytes = b"not valid json";

        let result = provider
            .transform_response(
                response_bytes,
                "@cf/meta/llama-3-8b-instruct",
                "test-request-id",
            )
            .await;

        assert!(result.is_err());
    }

    // ==================== OpenAI Params Mapping Tests ====================

    #[tokio::test]
    async fn test_map_openai_params_passthrough() {
        let provider = create_test_provider().await;

        let mut params = HashMap::new();
        params.insert("temperature".to_string(), serde_json::json!(0.7));
        params.insert("max_tokens".to_string(), serde_json::json!(100));

        let result = provider
            .map_openai_params(params.clone(), "@cf/meta/llama-3-8b-instruct")
            .await;

        assert!(result.is_ok());
        let mapped = result.unwrap();
        assert_eq!(mapped, params);
    }

    // ==================== Cost Calculation Tests ====================

    #[tokio::test]
    async fn test_calculate_cost_known_model() {
        let provider = create_test_provider().await;

        let cost = provider
            .calculate_cost("@cf/meta/llama-3-8b-instruct", 1000, 500)
            .await;

        assert!(cost.is_ok());
        let cost_value = cost.unwrap();
        assert!(cost_value >= 0.0);
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let provider = create_test_provider().await;

        let cost = provider.calculate_cost("unknown-model", 1000, 500).await;

        assert!(cost.is_err());
    }

    // ==================== Streaming Tests ====================

    #[tokio::test]
    async fn test_streaming_not_implemented() {
        let provider = create_test_provider().await;

        let request = ChatRequest {
            model: "@cf/meta/llama-3-8b-instruct".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            stream: true,
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.chat_completion_stream(request, context).await;

        assert!(result.is_err());
    }

    // ==================== Embeddings Tests ====================

    #[tokio::test]
    async fn test_embeddings_not_implemented() {
        let provider = create_test_provider().await;

        let request = EmbeddingRequest {
            model: "@cf/baai/bge-base-en-v1.5".to_string(),
            input: crate::core::types::embedding::EmbeddingInput::Text("test".to_string()),
            encoding_format: None,
            dimensions: None,
            user: None,
            task_type: None,
        };

        let context = RequestContext::default();
        let result = provider.embeddings(request, context).await;

        assert!(result.is_err());
    }

    // ==================== Error Mapper Tests ====================

    #[tokio::test]
    async fn test_get_error_mapper() {
        let provider = create_test_provider().await;
        let _mapper = provider.get_error_mapper();
        // Verify we can get an error mapper - just checking it doesn't panic
    }

    // ==================== Transform Request Trait Tests ====================

    #[tokio::test]
    async fn test_transform_request_trait() {
        let provider = create_test_provider().await;

        let request = ChatRequest {
            model: "@cf/meta/llama-3-8b-instruct".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.transform_request(request, context).await;

        assert!(result.is_ok());
        let transformed = result.unwrap();
        assert!(transformed["messages"].is_array());
    }

    // ==================== Clone/Debug Tests ====================

    #[tokio::test]
    async fn test_provider_clone() {
        let provider = create_test_provider().await;
        let cloned = provider.clone();

        assert_eq!(provider.name(), cloned.name());
        assert_eq!(provider.models().len(), cloned.models().len());
    }

    #[tokio::test]
    async fn test_provider_debug() {
        let provider = create_test_provider().await;
        let debug_str = format!("{:?}", provider);

        assert!(debug_str.contains("CloudflareProvider"));
    }
}
