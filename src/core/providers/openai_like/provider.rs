//! OpenAI-Like Provider Implementation
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpMethod, header_owned, read_streaming_error_body,
};
use crate::core::providers::openai::{OpenAIResponseTransformer, models::OpenAIChatResponse};
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    image::{ImageEditRequest, ImageGenerationRequest},
    model::{ModelInfo, ProviderCapability},
    responses::{ChatChunk, ChatResponse, EmbeddingResponse, ImageGenerationResponse},
};

use super::{
    config::OpenAILikeConfig,
    error::{OpenAILikeError, PROVIDER_NAME},
    models::{OpenAILikeModelRegistry, get_openai_like_registry},
    request_headers::build_request_headers,
};
use crate::core::providers::{GeminiNativeRequest, ProviderError, gemini_transport_error};

pub(crate) static OPENAI_LIKE_CATALOG_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    ProviderCapability::FunctionCalling,
];

pub(crate) static OPENAI_COMPATIBLE_PROXY_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::Embeddings,
    ProviderCapability::ImageGeneration,
    ProviderCapability::ImageEdit,
    ProviderCapability::ImageVariation,
    ProviderCapability::Moderation,
    ProviderCapability::ToolCalling,
    ProviderCapability::FunctionCalling,
];

#[derive(Debug, Clone)]
pub struct OpenAILikeProvider {
    pool_manager: Arc<GlobalPoolManager>,
    config: OpenAILikeConfig,
    model_registry: &'static OpenAILikeModelRegistry,
    provider_name: String,
    capabilities: &'static [ProviderCapability],
    pub(crate) model_identity:
        Option<crate::core::providers::model_identity::DeploymentProviderBinding>,
}

impl OpenAILikeProvider {
    pub(crate) async fn gemini_generate_content(
        &self,
        request: GeminiNativeRequest,
    ) -> Result<reqwest::Response, ProviderError> {
        if !crate::core::providers::capability_dispatch::openai_like_provider_supports_gemini(
            &self.provider_name,
        ) {
            return Err(ProviderError::not_supported(
                "openai_like",
                "Gemini native generateContent",
            ));
        }
        let api_key = self.config.base.api_key.as_deref().unwrap_or_default();
        let url = crate::core::providers::gemini_native_url(
            &self.config.get_api_base(),
            api_key,
            &request,
        )?;
        let headers = self
            .config
            .base
            .headers
            .iter()
            .chain(&self.config.custom_headers)
            .map(|(key, value)| header_owned(key.clone(), value.clone()))
            .collect();
        let response = if request.stream {
            Self::map_gemini_stream_response(
                tokio::time::timeout(
                    Duration::from_secs(self.config.base.timeout),
                    self.pool_manager
                        .execute_streaming_request_preserving_endpoint_policy(
                            url.as_str(),
                            headers,
                            request.body,
                            "gemini_proxy",
                        ),
                )
                .await,
            )?
        } else {
            self.pool_manager
                .execute_request_preserving_endpoint_policy(
                    url.as_str(),
                    HttpMethod::POST,
                    headers,
                    Some(request.body),
                )
                .await
                .map_err(gemini_openai_like_transport_error)?
        };
        crate::core::providers::gemini_response_or_provider_error(response, api_key).await
    }

    pub(crate) fn map_gemini_stream_response<T>(
        result: Result<Result<T, ProviderError>, tokio::time::error::Elapsed>,
    ) -> Result<T, ProviderError> {
        result
            .map_err(|_| ProviderError::timeout("gemini_proxy", "Gemini response header timeout"))?
            .map_err(gemini_openai_like_transport_error)
    }
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
    pub(crate) async fn new_for_catalog_no_redirect(
        config: OpenAILikeConfig,
        capabilities: &'static [ProviderCapability],
    ) -> Result<Self, OpenAILikeError> {
        let mut provider = Self::new_for_catalog(config, capabilities).await?;
        provider.pool_manager = Arc::new(
            GlobalPoolManager::new_for_provider_no_redirect(
                PROVIDER_NAME,
                provider.config.base.clone(),
            )
            .map_err(|error| OpenAILikeError::configuration(PROVIDER_NAME, error.to_string()))?,
        );
        Ok(provider)
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
            model_identity: None,
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
    pub async fn with_api_base(api_base: impl Into<String>) -> Result<Self, OpenAILikeError> {
        let config = OpenAILikeConfig::new(api_base).with_skip_api_key(true);
        Self::new(config).await
    }
    pub async fn with_api_key(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, OpenAILikeError> {
        let config = OpenAILikeConfig::with_api_key(api_base, api_key);
        Self::new(config).await
    }

    fn get_request_headers(&self) -> Vec<HeaderPair> {
        build_request_headers(&self.config)
    }

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
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.map_err(|error| {
                self.map_error_response(
                    status.as_u16(),
                    &format!("failed to read upstream error body: {error}"),
                )
            })?;
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

    fn rewrite_request_model(&self, model: &str) -> String {
        self.model_identity
            .as_ref()
            .map(|binding| binding.identity().wire_model().to_string())
            .unwrap_or_else(|| self.config.get_effective_model(model))
    }

    async fn execute_embeddings(
        &self,
        mut request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, OpenAILikeError> {
        request.model = self.rewrite_request_model(&request.model);
        let url = format!("{}/embeddings", self.config.get_api_base());
        let headers = self.get_request_headers();
        let body = Some(
            serde_json::to_value(&request)
                .map_err(|e| OpenAILikeError::serialization(PROVIDER_NAME, e.to_string()))?,
        );

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body)
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.map_err(|error| {
                self.map_error_response(
                    status.as_u16(),
                    &format!("failed to read upstream error body: {error}"),
                )
            })?;
            return Err(self.map_error_response(status.as_u16(), &body));
        }

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| OpenAILikeError::network(PROVIDER_NAME, e.to_string()))?;

        serde_json::from_slice(&response_bytes)
            .map_err(|e| OpenAILikeError::response_parsing(PROVIDER_NAME, e.to_string()))
    }

    async fn execute_image_generation(
        &self,
        mut request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, OpenAILikeError> {
        request.model = match request.model {
            Some(model) => Some(self.rewrite_request_model(&model)),
            None => self
                .model_identity
                .as_ref()
                .map(|binding| binding.identity().wire_model().to_string()),
        };
        let url = format!("{}/images/generations", self.config.get_api_base());
        let headers = self.get_request_headers();
        let body = Some(
            serde_json::to_value(&request)
                .map_err(|e| OpenAILikeError::serialization(PROVIDER_NAME, e.to_string()))?,
        );

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body)
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.map_err(|error| {
                self.map_error_response(
                    status.as_u16(),
                    &format!("failed to read upstream error body: {error}"),
                )
            })?;
            return Err(self.map_error_response(status.as_u16(), &body));
        }

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| OpenAILikeError::network(PROVIDER_NAME, e.to_string()))?;

        serde_json::from_slice(&response_bytes)
            .map_err(|e| OpenAILikeError::response_parsing(PROVIDER_NAME, e.to_string()))
    }

    fn transform_chat_request(&self, request: ChatRequest) -> Result<Value, OpenAILikeError> {
        let model = self
            .model_identity
            .as_ref()
            .map(|binding| binding.identity().wire_model().to_string())
            .unwrap_or_else(|| {
                super::models::xai_native_wire_model(
                    &self.provider_name,
                    self.config.model_prefix.is_some(),
                    self.config.get_effective_model(&request.model),
                )
            });
        let xai_model = self
            .model_identity
            .as_ref()
            .filter(|binding| binding.identity().capability_catalog_provider() == Some("xai"))
            .and_then(|binding| binding.identity().capability_catalog_model())
            .or_else(|| (self.provider_name == "xai").then_some(model.as_str()));
        let xai_model = xai_model.map(str::to_string);

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

        let mut extra_params = request.extra_params;
        let reasoning_effort = super::models::take_xai_reasoning_effort(
            xai_model.as_deref(),
            request.reasoning_effort,
            &mut extra_params,
        )?;
        let has_reasoning_effort = reasoning_effort.is_some();

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
            self.insert_reasoning_effort(&mut openai_request, xai_model.as_deref(), effort)?;
        }

        if let Some(obj) = openai_request.as_object_mut() {
            for (key, value) in extra_params {
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

        if xai_model.is_some() && has_reasoning_effort {
            super::models::reject_xai_reasoning_incompatible_params(&openai_request)?;
        }

        crate::core::providers::registry::catalog_policy::filter_request(
            &self.provider_name,
            &mut openai_request,
        );

        Ok(openai_request)
    }

    fn insert_reasoning_effort(
        &self,
        request: &mut Value,
        xai_model: Option<&str>,
        effort: String,
    ) -> Result<(), OpenAILikeError> {
        let Some(model) = xai_model else {
            request["reasoning_effort"] = Value::String(effort);
            return Ok(());
        };
        if super::models::xai_reasoning_efforts_for_model(model).is_some()
            && !super::models::xai_accepts_reasoning_effort(model, &effort)
        {
            return Err(OpenAILikeError::configuration(
                PROVIDER_NAME,
                format!("unsupported reasoning_effort '{effort}' for xAI model {model}"),
            ));
        }

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

    fn transform_chat_response(&self, response: Value) -> Result<ChatResponse, OpenAILikeError> {
        let resp: OpenAIChatResponse = serde_json::from_value(response)
            .map_err(|e| OpenAILikeError::response_parsing(PROVIDER_NAME, e.to_string()))?;
        OpenAIResponseTransformer::transform(resp)
            .map_err(|e| OpenAILikeError::response_parsing(PROVIDER_NAME, e.to_string()))
    }

    fn map_error_response(&self, status: u16, body: &str) -> OpenAILikeError {
        if let Some(error) =
            crate::core::providers::registry::catalog_policy::catalog_error_response(
                &self.provider_name,
                status,
                body,
            )
        {
            return error;
        }

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

    pub fn get_model_info(&self, model_id: &str) -> ModelInfo {
        if let Some(info) = crate::core::providers::registry::catalog_policy::catalog_model_info(
            &self.provider_name,
            model_id,
        ) {
            return info;
        }
        let mut info = self.model_registry.get_model_info(model_id);
        if self.provider_name != "xai" && super::models::is_xai_priced_model(model_id) {
            info.input_cost_per_1k_tokens = None;
            info.output_cost_per_1k_tokens = None;
        }
        info
    }

    pub fn config(&self) -> &OpenAILikeConfig {
        &self.config
    }
}

fn gemini_openai_like_transport_error(error: ProviderError) -> ProviderError {
    match error {
        ProviderError::Configuration { message, .. } => {
            ProviderError::configuration("gemini_proxy", message)
        }
        ProviderError::Timeout { .. } => gemini_transport_error(true),
        _ => gemini_transport_error(false),
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
        crate::core::providers::registry::catalog_policy::catalog_model_infos(&self.provider_name)
            .unwrap_or(&[])
    }

    fn supports_model(&self, model: &str) -> bool {
        if self.provider_name == "xai" {
            return self
                .model_registry
                .is_known_model(&self.config.get_effective_model(model));
        }
        crate::core::providers::registry::catalog_policy::catalog_provider_supports_model(
            &self.provider_name,
            model,
        )
        .unwrap_or(true)
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

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        self.execute_embeddings(request).await
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        self.execute_image_generation(request).await
    }

    async fn image_edit(
        &self,
        mut request: ImageEditRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        request.model = request
            .model
            .map(|model| self.config.get_effective_model(&model));
        crate::core::providers::openai::execute_image_edit(
            self.config.base.clone(),
            &self.config.get_api_base(),
            self.get_request_headers(),
            request,
            PROVIDER_NAME,
        )
        .await
    }
    async fn health_check(&self) -> HealthStatus {
        let url = format!("{}/models", self.config.get_api_base());
        match self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, self.get_request_headers(), None)
            .await
        {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(_) if crate::core::providers::registry::catalog_policy::health_failure_is_unhealthy(
                &self.provider_name,
            ) => HealthStatus::Unhealthy,
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
        if let Some(result) =
            crate::core::providers::model_identity::calculate_managed_provider_cost(
                &crate::core::providers::Provider::OpenAILike(self.clone()),
                model,
                input_tokens,
                output_tokens,
            )
        {
            return result;
        }
        let model_info = self.get_model_info(model);
        if self.config.provider_name == "meta_llama"
            && crate::core::providers::registry::catalog_policy::catalog_model_info(
                &self.provider_name,
                model,
            )
            .is_some()
            && (model_info.input_cost_per_1k_tokens.is_none()
                || model_info.output_cost_per_1k_tokens.is_none())
        {
            return Err(ProviderError::invalid_request(
                "meta_llama",
                format!("pricing is unavailable for model '{model}'"),
            ));
        }
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
        crate::core::providers::registry::catalog_policy::catalog_provider_supported_openai_params(
            &self.provider_name,
        )
    }
    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        Ok(
            crate::core::providers::registry::catalog_policy::filter_openai_params(
                &self.provider_name,
                params,
            ),
        )
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
