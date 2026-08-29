//! Anthropic provider implementation.

use crate::core::providers::unified_provider::ProviderError;
use crate::core::providers::{ChatContinuationRequest, ChatContinuationResponse};
use crate::core::traits::provider::ProviderConfig as _;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse},
};
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use super::client::AnthropicClient;
use super::config::AnthropicConfig;
use super::models::{
    ModelFeature, get_anthropic_registry, standalone_fable_model_info, supported_openai_params,
};
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
const COMPATIBLE_MODEL_MAX_OUTPUT_TOKENS: u32 = 128_000;

/// Anthropic provider.
#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    client: Box<AnthropicClient>,
    supported_models: Vec<ModelInfo>,
}

impl AnthropicProvider {
    /// Create
    pub fn new(config: AnthropicConfig) -> Result<Self, ProviderError> {
        let client = AnthropicClient::new(config.clone())?;
        let supported_models = if config.uses_compatible_model_allow_list() {
            config
                .configured_models
                .iter()
                .map(|model| ModelInfo {
                    id: model.clone(),
                    name: model.clone(),
                    provider: "anthropic".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(COMPATIBLE_MODEL_MAX_OUTPUT_TOKENS),
                    supports_streaming: true,
                    supports_tools: false,
                    supports_multimodal: config.allows_unknown_model_image_input(model),
                    input_cost_per_1k_tokens: None,
                    output_cost_per_1k_tokens: None,
                    currency: "USD".to_string(),
                    capabilities: vec![ProviderCapability::ChatCompletion],
                    created_at: None,
                    updated_at: None,
                    metadata: HashMap::new(),
                })
                .collect()
        } else {
            let registry = get_anthropic_registry();
            let mut models = registry
                .list_models()
                .into_iter()
                .map(|spec| spec.model_info.clone())
                .collect::<Vec<_>>();
            if !models.iter().any(|model| model.id == "claude-fable-5") {
                models.push(standalone_fable_model_info());
            }
            models
        };

        Ok(Self {
            client: Box::new(client),
            supported_models,
        })
    }

    pub(crate) async fn chat_with_continuation(
        &self,
        request: ChatContinuationRequest,
    ) -> Result<ChatContinuationResponse, ProviderError> {
        self.validate_request(request.request())?;
        self.client.chat_with_continuation(request).await
    }

    fn validate_request(&self, request: &ChatRequest) -> Result<(), ProviderError> {
        let registry = get_anthropic_registry();

        let model_spec = if self.client.uses_compatible_model_allow_list() {
            if !self.client.allows_unknown_model(&request.model) {
                return Err(ProviderError::invalid_request(
                    "anthropic",
                    format!("Unsupported model: {}", request.model),
                ));
            }
            None
        } else {
            registry.get_model_spec(&request.model)
        };

        let Some(model_spec) = model_spec else {
            if AnthropicClient::is_standalone_claude_5_protocol_model(&request.model) {
                return crate::core::providers::base::validate_chat_request_common(
                    "anthropic",
                    request,
                    COMPATIBLE_MODEL_MAX_OUTPUT_TOKENS,
                );
            }
            if !self.client.allows_unknown_model(&request.model) {
                return Err(ProviderError::invalid_request(
                    "anthropic",
                    format!("Unsupported model: {}", request.model),
                ));
            }

            crate::core::providers::base::validate_chat_request_common(
                "anthropic",
                request,
                COMPATIBLE_MODEL_MAX_OUTPUT_TOKENS,
            )?;

            if request
                .tools
                .as_ref()
                .is_some_and(|tools| !tools.is_empty())
                || AnthropicClient::has_anthropic_tools_extra_param(request)
                || request.functions.as_ref().is_some_and(|f| !f.is_empty())
                || request.function_call.is_some()
            {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!(
                        "Unknown model {} cannot declare tool calling support",
                        request.model
                    ),
                ));
            }

            if AnthropicClient::has_unsupported_unknown_model_content(request) {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!(
                        "Unknown model {} only supports text and image content",
                        request.model
                    ),
                ));
            }

            if AnthropicClient::has_image_content(request)
                && !self.client.allows_unknown_model_image_input(&request.model)
            {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!(
                        "Unknown model {} does not support image input",
                        request.model
                    ),
                ));
            }

            if request
                .thinking
                .as_ref()
                .is_some_and(|thinking| thinking.enabled)
            {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!(
                        "Unknown model {} cannot declare thinking support",
                        request.model
                    ),
                ));
            }

            return Ok(());
        };

        crate::core::providers::base::validate_chat_request_common(
            "anthropic",
            request,
            model_spec.limits.max_output_tokens,
        )?;

        let has_multimodal_content = AnthropicClient::has_multimodal_content(request);

        if has_multimodal_content
            && !model_spec
                .features
                .contains(&ModelFeature::MultimodalSupport)
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!(
                    "Model {} does not support multimodal content",
                    request.model
                ),
            ));
        }

        if request
            .tools
            .as_ref()
            .is_some_and(|tools| !tools.is_empty())
            && !model_spec.features.contains(&ModelFeature::ToolCalling)
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!("Model {} does not support tool calling", request.model),
            ));
        }

        Ok(())
    }
}

impl LLMProvider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn error_provider_name(&self) -> &'static str {
        "anthropic"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ToolCalling,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn supports_model(&self, model: &str) -> bool {
        if self.client.uses_compatible_model_allow_list() {
            return self.client.allows_unknown_model(model);
        }

        self.supported_models.iter().any(|info| info.id == model)
            || AnthropicClient::is_standalone_claude_5_protocol_model(model)
            || self.client.allows_unknown_model(model)
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        supported_openai_params(model)
    }

    async fn map_openai_params(
        &self,
        mut params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        if let Some(max_tokens) = params.remove("max_tokens") {
            params.insert("max_tokens".to_string(), max_tokens);
        }

        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        self.validate_request(&request)?;

        // Preserve the provider-trait core wire contract. The execution client
        // performs Anthropic-native block conversion at the HTTP boundary.
        let mut transformed = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
        });
        if let Some(max_tokens) = request.max_tokens {
            transformed["max_tokens"] = Value::Number(max_tokens.into());
        }
        if let Some(temperature) = request.temperature {
            let value = f64::from(temperature);
            transformed["temperature"] =
                Value::Number(serde_json::Number::from_f64(value).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "anthropic",
                        format!("invalid temperature value: {value}"),
                    )
                })?);
        }
        if let Some(top_p) = request.top_p {
            let value = f64::from(top_p);
            transformed["top_p"] =
                Value::Number(serde_json::Number::from_f64(value).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "anthropic",
                        format!("invalid top_p value: {value}"),
                    )
                })?);
        }
        if request.stream {
            transformed["stream"] = Value::Bool(true);
        }
        if let Some(tools) = request.tools
            && !tools.is_empty()
        {
            transformed["tools"] = serde_json::to_value(tools)?;
        }
        Ok(transformed)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let response_text = String::from_utf8_lossy(raw_response);
        let anthropic_response: Value = serde_json::from_str(&response_text)?;

        let response = serde_json::from_value(anthropic_response)?;
        Ok(response)
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(super::error::AnthropicErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        self.validate_request(&request)?;
        let response = self.client.chat(request.clone()).await?;
        Ok(response)
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        self.validate_request(&request)?;

        let registry = get_anthropic_registry();
        if let Some(model_spec) = registry.get_model_spec(&request.model)
            && !model_spec
                .features
                .contains(&ModelFeature::StreamingSupport)
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!("Model {} does not support streaming", request.model),
            ));
        }

        let stream = self.client.chat_stream_chunks(request).await?;

        Ok(Box::pin(stream))
    }

    async fn health_check(&self) -> HealthStatus {
        let health_check_model = if self.client.uses_compatible_model_allow_list() {
            self.supported_models.first().map(|model| model.id.clone())
        } else {
            Some("claude-3-haiku-20240307".to_string())
        };
        let Some(model) = health_check_model else {
            return HealthStatus::Unhealthy;
        };
        let test_request = ChatRequest {
            model,
            messages: vec![crate::core::types::chat::ChatMessage {
                role: crate::core::types::message::MessageRole::User,
                content: Some(crate::core::types::message::MessageContent::Text(
                    "ping".to_string(),
                )),
                ..Default::default()
            }],
            max_tokens: Some(1),
            ..Default::default()
        };

        match self.client.chat(test_request).await {
            Ok(_) => HealthStatus::Healthy,
            Err(ProviderError::Authentication { .. }) => HealthStatus::Unhealthy,
            Err(ProviderError::Network { .. }) => HealthStatus::Degraded,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        Ok(
            super::models::CostCalculator::calculate_cost(model, input_tokens, output_tokens)
                .unwrap_or(0.0),
        )
    }
}

/// Provider builder
pub struct AnthropicProviderBuilder {
    config: Option<AnthropicConfig>,
}

impl AnthropicProviderBuilder {
    /// Create
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Set configuration
    pub fn with_config(mut self, config: AnthropicConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        let api_key = api_key.into();
        if let Some(ref mut config) = self.config {
            config.api_key = Some(api_key);
        } else {
            self.config = Some(AnthropicConfig::new(api_key));
        }
        self
    }

    /// Set base URL
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        if let Some(ref mut config) = self.config {
            config.base_url = base_url.into();
        }
        self
    }

    /// Build provider
    pub fn build(self) -> Result<AnthropicProvider, ProviderError> {
        let config = self.config.ok_or_else(|| {
            ProviderError::configuration("anthropic", "Configuration is required")
        })?;

        AnthropicProvider::new(config)
    }
}

impl Default for AnthropicProviderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create
pub fn create_anthropic_provider(
    config: AnthropicConfig,
) -> Result<AnthropicProvider, ProviderError> {
    AnthropicProvider::new(config)
}

/// Create
pub fn create_anthropic_provider_from_env() -> Result<AnthropicProvider, ProviderError> {
    let config = AnthropicConfig::from_env()?;
    config
        .validate()
        .map_err(|e| ProviderError::configuration("anthropic", e))?;
    AnthropicProvider::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let config = AnthropicConfig::new_test("test-key");
        let provider = AnthropicProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_capabilities() {
        let config = AnthropicConfig::new_test("test-key");
        let provider = AnthropicProvider::new(config).unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::ToolCalling));
    }

    #[test]
    fn test_provider_builder() {
        let provider = AnthropicProviderBuilder::new()
            .with_api_key("test-key")
            .with_base_url("https://api.anthropic.com")
            .build();

        assert!(provider.is_ok());
    }

    #[test]
    fn test_model_support() {
        let config = AnthropicConfig::new_test("test-key");
        let provider = AnthropicProvider::new(config).unwrap();

        assert!(provider.supports_model("claude-3-5-sonnet-20241022"));
        assert!(provider.supports_model("claude-3-haiku-20240307"));
        assert!(!provider.supports_model("gpt-4"));
    }

    #[test]
    fn configured_unknown_models_are_visible_to_support_checks() {
        let config = AnthropicConfig::new_test("test-key")
            .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
            .with_allow_unknown_models(true)
            .with_configured_models(vec!["mimo-v2.5".to_string()])
            .with_configured_multimodal_models(vec!["mimo-v2.5".to_string()]);
        let provider = match AnthropicProvider::new(config) {
            Ok(provider) => provider,
            Err(err) => panic!("provider should build: {err}"),
        };

        assert!(provider.supports_model("mimo-v2.5"));
        assert!(!provider.supports_model("unlisted-compatible-model"));
        assert!(!provider.supports_model("claude-3-5-sonnet-20241022"));
        assert_eq!(provider.models().len(), 1);
        assert_eq!(provider.models()[0].id, "mimo-v2.5");
    }

    #[tokio::test]
    async fn unknown_models_are_rejected_by_default() {
        let config = AnthropicConfig::new_test("test-key");
        let provider = AnthropicProvider::new(config).unwrap();
        let request = ChatRequest::new("mimo-v2.5").add_user_message("Hello");

        let err = provider
            .transform_request(request, RequestContext::new())
            .await
            .expect_err("unknown model should be rejected without explicit opt-in");

        assert!(format!("{err}").contains("Unsupported model: mimo-v2.5"));
    }

    #[tokio::test]
    async fn allow_unknown_models_accepts_anthropic_compatible_model_ids() {
        let config = AnthropicConfig::new_test("test-key")
            .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
            .with_allow_unknown_models(true)
            .with_configured_models(vec!["mimo-v2.5".to_string()])
            .with_configured_multimodal_models(vec!["mimo-v2.5".to_string()]);
        let provider = match AnthropicProvider::new(config) {
            Ok(provider) => provider,
            Err(err) => panic!("provider should build: {err}"),
        };
        let request = ChatRequest::new("mimo-v2.5")
            .add_user_message("Reply with exactly: anthropic-compatible-ok");

        let transformed = provider
            .transform_request(request, RequestContext::new())
            .await
            .expect("explicit opt-in should allow non-Anthropic model IDs");

        assert_eq!(transformed["model"], "mimo-v2.5");
    }

    #[tokio::test]
    async fn compatible_allow_list_rejects_registry_model_ids_not_configured() {
        let config = AnthropicConfig::new_test("test-key")
            .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
            .with_allow_unknown_models(true)
            .with_configured_models(vec!["mimo-v2.5".to_string()]);
        let provider = match AnthropicProvider::new(config) {
            Ok(provider) => provider,
            Err(err) => panic!("provider should build: {err}"),
        };
        let request = ChatRequest::new("claude-3-haiku-20240307").add_user_message("Hello");

        let err = match provider
            .transform_request(request, RequestContext::new())
            .await
        {
            Ok(_) => panic!("compatible allow-list must reject unlisted registry model IDs"),
            Err(err) => err,
        };

        assert!(format!("{err}").contains("Unsupported model: claude-3-haiku-20240307"));
    }

    #[tokio::test]
    async fn compatible_models_enforce_configured_output_limit() {
        let config = AnthropicConfig::new_test("test-key")
            .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
            .with_allow_unknown_models(true)
            .with_configured_models(vec!["mimo-v2.5".to_string()]);
        let provider = match AnthropicProvider::new(config) {
            Ok(provider) => provider,
            Err(err) => panic!("provider should build: {err}"),
        };
        let request = ChatRequest::new("mimo-v2.5")
            .add_user_message("Hello")
            .with_max_tokens(COMPATIBLE_MODEL_MAX_OUTPUT_TOKENS + 1);

        let err = match provider
            .transform_request(request, RequestContext::new())
            .await
        {
            Ok(_) => panic!("compatible models must enforce max output tokens"),
            Err(err) => err,
        };

        assert!(format!("{err}").contains("max_tokens 128001 exceeds model limit of 128000"));
    }

    #[tokio::test]
    async fn allow_listed_unknown_models_accept_base64_images() {
        use crate::core::types::{
            content::{ContentPart, ImageUrl},
            message::{MessageContent, MessageRole},
        };

        let config = AnthropicConfig::new_test("test-key")
            .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
            .with_allow_unknown_models(true)
            .with_configured_models(vec!["mimo-v2.5".to_string()])
            .with_configured_multimodal_models(vec!["mimo-v2.5".to_string()]);
        let provider = match AnthropicProvider::new(config) {
            Ok(provider) => provider,
            Err(err) => panic!("provider should build: {err}"),
        };
        let request = ChatRequest {
            model: "mimo-v2.5".to_string(),
            messages: vec![crate::core::types::chat::ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Parts(vec![
                    ContentPart::Text {
                        text: "Describe this image".to_string(),
                    },
                    ContentPart::ImageUrl {
                        image_url: ImageUrl {
                            url: "data:image/png;base64,aGVsbG8=".to_string(),
                            detail: None,
                        },
                    },
                ])),
                ..Default::default()
            }],
            ..Default::default()
        };

        let transformed = match provider
            .transform_request(request, RequestContext::new())
            .await
        {
            Ok(transformed) => transformed,
            Err(err) => {
                panic!("explicit compatible model should preserve base64 image input: {err}")
            }
        };

        assert_eq!(transformed["model"], "mimo-v2.5");
        assert_eq!(
            transformed["messages"][0]["content"][1]["type"],
            "image_url"
        );
    }

    #[tokio::test]
    async fn text_only_unknown_models_reject_image_input() {
        use crate::core::types::{
            content::{ContentPart, ImageUrl},
            message::{MessageContent, MessageRole},
        };

        let config = AnthropicConfig::new_test("test-key")
            .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
            .with_allow_unknown_models(true)
            .with_configured_models(vec!["mimo-v2.5-pro".to_string()]);
        let provider = match AnthropicProvider::new(config) {
            Ok(provider) => provider,
            Err(err) => panic!("provider should build: {err}"),
        };
        let request = ChatRequest {
            model: "mimo-v2.5-pro".to_string(),
            messages: vec![crate::core::types::chat::ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Parts(vec![
                    ContentPart::Text {
                        text: "Describe this image".to_string(),
                    },
                    ContentPart::ImageUrl {
                        image_url: ImageUrl {
                            url: "data:image/png;base64,aGVsbG8=".to_string(),
                            detail: None,
                        },
                    },
                ])),
                ..Default::default()
            }],
            ..Default::default()
        };

        let err = match provider
            .transform_request(request, RequestContext::new())
            .await
        {
            Ok(_) => panic!("text-only compatible model must reject image input"),
            Err(err) => err,
        };

        assert!(format!("{err}").contains("does not support image input"));
    }

    #[tokio::test]
    async fn unknown_models_reject_anthropic_tools_extra_param() {
        let config = AnthropicConfig::new_test("test-key")
            .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
            .with_allow_unknown_models(true)
            .with_configured_models(vec!["mimo-v2.5".to_string()]);
        let provider = match AnthropicProvider::new(config) {
            Ok(provider) => provider,
            Err(err) => panic!("provider should build: {err}"),
        };
        let mut request = ChatRequest::new("mimo-v2.5").add_user_message("Hello");
        request.extra_params.insert(
            "anthropic_tools".to_string(),
            serde_json::json!([{"type": "computer_20241022", "name": "computer"}]),
        );

        let err = match provider
            .transform_request(request, RequestContext::new())
            .await
        {
            Ok(_) => panic!("unknown models must fail closed for Anthropic built-in tools"),
            Err(err) => err,
        };

        assert!(format!("{err}").contains("tool calling support"));
    }

    #[tokio::test]
    async fn unknown_models_reject_message_level_tool_blocks() {
        use crate::core::types::{
            content::ContentPart,
            message::{MessageContent, MessageRole},
        };

        let config = AnthropicConfig::new_test("test-key")
            .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
            .with_allow_unknown_models(true)
            .with_configured_models(vec!["mimo-v2.5".to_string()]);
        let provider = match AnthropicProvider::new(config) {
            Ok(provider) => provider,
            Err(err) => panic!("provider should build: {err}"),
        };
        let request = ChatRequest {
            model: "mimo-v2.5".to_string(),
            messages: vec![crate::core::types::chat::ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Parts(vec![ContentPart::ToolUse {
                    id: "toolu_1".to_string(),
                    name: "computer".to_string(),
                    input: serde_json::json!({}),
                }])),
                ..Default::default()
            }],
            ..Default::default()
        };

        let err = match provider
            .transform_request(request, RequestContext::new())
            .await
        {
            Ok(_) => panic!("unknown models must reject message-level tool blocks"),
            Err(err) => err,
        };

        assert!(format!("{err}").contains("only supports text and image content"));
    }
}
