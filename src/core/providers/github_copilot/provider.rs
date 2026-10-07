//! Main GitHub Copilot Provider Implementation
//!
//! Implements the LLMProvider trait for GitHub Copilot API.
//! Handles OAuth authentication and OpenAI-compatible chat completions.

use bytes::Bytes;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use super::authenticator::CopilotAuthenticator;
use super::config::{GITHUB_COPILOT_API_BASE, GitHubCopilotConfig, get_copilot_default_headers};
use super::model_info::{
    get_available_models, get_model_info, is_claude_model, supports_reasoning,
};
use crate::ProviderError;
use crate::core::providers::base::{BaseConfig, BaseHttpClient, HttpErrorMapper};
use crate::core::streaming::utils::is_done_marker;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    message::MessageRole,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse},
};

/// Static capabilities for GitHub Copilot provider
const GITHUB_COPILOT_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    #[cfg(feature = "gateway")]
    ProviderCapability::Responses,
];

/// GitHub Copilot provider implementation
#[derive(Debug)]
pub struct GitHubCopilotProvider {
    config: GitHubCopilotConfig,
    authenticator: CopilotAuthenticator,
    models: Vec<ModelInfo>,
    /// Cached API key
    cached_api_key: Arc<RwLock<Option<String>>>,
    /// Cached API base
    cached_api_base: Arc<RwLock<Option<String>>>,
    #[cfg(test)]
    native_endpoint_access: crate::core::net::ProviderEndpointAccess,
}

impl Clone for GitHubCopilotProvider {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            authenticator: self.authenticator.clone(),
            models: self.models.clone(),
            cached_api_key: Arc::new(RwLock::new(None)),
            cached_api_base: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            native_endpoint_access: self.native_endpoint_access,
        }
    }
}

impl GitHubCopilotProvider {
    pub(crate) fn credential_resource_identity(&self) -> [u8; 32] {
        self.authenticator.credential_resource_identity()
    }

    /// Create a new GitHub Copilot provider instance
    pub async fn new(config: GitHubCopilotConfig) -> Result<Self, ProviderError> {
        let authenticator = CopilotAuthenticator::new(&config);

        // Build model list from static configuration
        let models = get_available_models()
            .iter()
            .filter_map(|id| get_model_info(id))
            .map(|info| {
                // Endpoint support is account-specific and checked against
                // /models before inference; static IDs do not imply chat support.
                let mut capabilities = Vec::new();
                if info.supports_tools {
                    capabilities.push(ProviderCapability::ToolCalling);
                }

                ModelInfo {
                    id: info.model_id.to_string(),
                    name: info.display_name.to_string(),
                    provider: "github_copilot".to_string(),
                    max_context_length: info.max_context_length,
                    max_output_length: info.max_output_length,
                    supports_streaming: info.supports_streaming,
                    supports_tools: info.supports_tools,
                    supports_multimodal: info.supports_multimodal,
                    input_cost_per_1k_tokens: None, // Copilot is subscription-based
                    output_cost_per_1k_tokens: None,
                    currency: "USD".to_string(),
                    capabilities,
                    created_at: None,
                    updated_at: None,
                    metadata: HashMap::from([(
                        "endpoint_authority".into(),
                        serde_json::json!("account /models supported_endpoints"),
                    )]),
                }
            })
            .collect();

        Ok(Self {
            config,
            authenticator,
            models,
            cached_api_key: Arc::new(RwLock::new(None)),
            cached_api_base: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            native_endpoint_access: crate::core::net::ProviderEndpointAccess::PublicOnly,
        })
    }

    fn native_http_client(
        &self,
        api_base: &str,
        streaming: bool,
    ) -> Result<BaseHttpClient, ProviderError> {
        let config = BaseConfig {
            api_base: Some(api_base.to_string()),
            timeout: self.config.timeout,
            #[cfg(test)]
            endpoint_access: self.native_endpoint_access,
            ..Default::default()
        };
        if streaming {
            BaseHttpClient::new_for_provider_streaming_no_redirect("github_copilot", config)
        } else {
            BaseHttpClient::new_for_provider_no_redirect("github_copilot", config)
        }
    }

    /// The account model catalog, not the model name or price table, decides
    /// whether a request may use a particular Copilot endpoint.
    async fn require_endpoint(
        &self,
        model: &str,
        endpoint: &str,
        headers: &reqwest::header::HeaderMap,
    ) -> Result<(), ProviderError> {
        let api_base = self.get_api_base().await;
        let url = format!("{}/models", api_base.trim_end_matches('/'));
        let client = self.native_http_client(&api_base, false)?;
        let response = client
            .get(url)?
            .headers(headers.clone())
            .send()
            .await
            .map_err(|error| client.map_preserved_request_error(error))?;
        let response = self.check_native_status(response).await?;
        let catalog: serde_json::Value = response.json().await.map_err(|_| {
            ProviderError::response_parsing("github_copilot", "Invalid account model catalog")
        })?;
        let models = catalog
            .get("data")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ProviderError::response_parsing(
                    "github_copilot",
                    "Account model catalog has no data array",
                )
            })?;
        let model_info = models
            .iter()
            .find(|entry| entry.get("id").and_then(serde_json::Value::as_str) == Some(model))
            .ok_or_else(|| ProviderError::model_not_found("github_copilot", model))?;
        if !model_info
            .get("supported_endpoints")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|endpoints| {
                endpoints
                    .iter()
                    .any(|entry| entry.as_str() == Some(endpoint))
            })
        {
            return Err(ProviderError::not_supported(
                "github_copilot",
                format!("Model {model} does not advertise {endpoint}"),
            ));
        }
        Ok(())
    }

    async fn check_native_status(
        &self,
        response: reqwest::Response,
    ) -> Result<reqwest::Response, ProviderError> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        if status.as_u16() == 401 {
            self.clear_cache().await;
        }
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok());
        let body = crate::core::providers::base::read_streaming_error_body(response)
            .await
            .unwrap_or_else(|_| "Could not read Copilot error response".into());
        if status.as_u16() == 429 {
            return Err(ProviderError::rate_limit_with_retry(
                "github_copilot",
                body,
                retry_after,
            ));
        }
        Err(HttpErrorMapper::map_status_code(
            "github_copilot",
            status.as_u16(),
            &body,
        ))
    }

    #[cfg(feature = "gateway")]
    pub(crate) async fn native_response(
        &self,
        body: serde_json::Value,
        generation_attempted: &mut bool,
    ) -> Result<reqwest::Response, ProviderError> {
        use serde_json::Value;
        let model = body.get("model").and_then(Value::as_str).ok_or_else(|| {
            ProviderError::invalid_request("github_copilot", "model must be a string")
        })?;
        let mut headers = self.build_headers(&[]).await?;
        self.require_endpoint(model, "/responses", &headers).await?;
        let items = body.get("input").and_then(Value::as_array);
        let agent = items.is_some_and(|items| {
            items.iter().any(|item| {
                !matches!(
                    item.get("role").and_then(Value::as_str),
                    Some("user" | "system" | "developer")
                )
            })
        });
        headers.insert(
            "x-initiator",
            reqwest::header::HeaderValue::from_static(if agent { "agent" } else { "user" }),
        );
        if items.is_some_and(|items| {
            items
                .iter()
                .filter_map(|item| item.get("content").and_then(Value::as_array))
                .flatten()
                .any(|part| part.get("type").and_then(Value::as_str) == Some("input_image"))
        }) {
            headers.insert(
                "copilot-vision-request",
                reqwest::header::HeaderValue::from_static("true"),
            );
        }
        let api_base = self.get_api_base().await;
        let url = format!("{}/responses", api_base.trim_end_matches('/'));
        let client =
            self.native_http_client(&api_base, body.get("stream") == Some(&Value::Bool(true)))?;
        let request = client.post(url)?.headers(headers).json(&body);
        // Discovery/authentication failures above cannot have created a response.
        // Once send is polled, a transport failure can hide an accepted POST.
        *generation_attempted = true;
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(self.config.timeout),
            request.send(),
        )
        .await
        .map_err(|_| ProviderError::timeout("github_copilot", "Responses upstream timed out"))?
        .map_err(|error| client.map_preserved_request_error(error))?;
        self.check_native_status(response).await
    }

    /// Get the API key, using cache or refreshing if needed
    async fn get_api_key(&self) -> Result<String, ProviderError> {
        // Check cache first
        {
            let cache = self.cached_api_key.read().await;
            if let Some(ref key) = *cache {
                return Ok(key.clone());
            }
        }

        // Get fresh key
        let key = self.authenticator.get_api_key().await?;

        // Update cache
        {
            let mut cache = self.cached_api_key.write().await;
            *cache = Some(key.clone());
        }

        // Also update API base cache
        if let Some(api_base) = self.authenticator.get_api_base() {
            let mut cache = self.cached_api_base.write().await;
            *cache = Some(api_base);
        }

        Ok(key)
    }

    /// Get the API base URL
    async fn get_api_base(&self) -> String {
        // Check cache first
        {
            let cache = self.cached_api_base.read().await;
            if let Some(ref base) = *cache {
                return base.clone();
            }
        }

        // Use config or authenticator
        self.config
            .api_base
            .clone()
            .or_else(|| self.authenticator.get_api_base())
            .unwrap_or_else(|| GITHUB_COPILOT_API_BASE.to_string())
    }

    /// Clear cached credentials (for refresh)
    async fn clear_cache(&self) {
        {
            let mut cache = self.cached_api_key.write().await;
            *cache = None;
        }
        {
            let mut cache = self.cached_api_base.write().await;
            *cache = None;
        }
    }

    /// Transform messages for Copilot API
    fn transform_messages(&self, messages: &mut [ChatMessage]) {
        if self.config.disable_system_to_assistant {
            return;
        }

        // Convert system messages to assistant messages (Copilot requirement)
        for message in messages.iter_mut() {
            if message.role == MessageRole::System {
                message.role = MessageRole::Assistant;
            }
        }
    }

    /// Determine X-Initiator header value
    fn determine_initiator(&self, messages: &[ChatMessage]) -> &'static str {
        for message in messages {
            if message.role == MessageRole::Tool || message.role == MessageRole::Assistant {
                return "agent";
            }
        }
        "user"
    }

    /// Check if request contains vision content
    fn has_vision_content(&self, messages: &[ChatMessage]) -> bool {
        for message in messages {
            if let Some(crate::core::types::message::MessageContent::Parts(parts)) =
                &message.content
            {
                for part in parts {
                    if let crate::core::types::content::ContentPart::ImageUrl { .. } = part {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Build request headers
    async fn build_headers(
        &self,
        messages: &[ChatMessage],
    ) -> Result<reqwest::header::HeaderMap, ProviderError> {
        let api_key = self.get_api_key().await?;
        let default_headers = get_copilot_default_headers(&api_key);

        let mut headers = reqwest::header::HeaderMap::new();
        for (key, value) in default_headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    ProviderError::configuration(
                        "github_copilot",
                        format!("Invalid header name: {}", e),
                    )
                })?,
                value.parse().map_err(|e| {
                    ProviderError::configuration(
                        "github_copilot",
                        format!("Invalid header value: {}", e),
                    )
                })?,
            );
        }

        // Add X-Initiator header
        let initiator = self.determine_initiator(messages);
        headers.insert(
            "x-initiator",
            initiator.parse().map_err(|e| {
                ProviderError::configuration(
                    "github_copilot",
                    format!("Invalid x-initiator header value: {}", e),
                )
            })?,
        );

        // Add Copilot-Vision-Request if contains images
        if self.has_vision_content(messages) {
            headers.insert(
                "copilot-vision-request",
                "true".parse().map_err(|e| {
                    ProviderError::configuration(
                        "github_copilot",
                        format!("Invalid copilot-vision-request header value: {}", e),
                    )
                })?,
            );
        }

        Ok(headers)
    }
}

impl LLMProvider for GitHubCopilotProvider {
    fn name(&self) -> &'static str {
        "github_copilot"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        GITHUB_COPILOT_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        let is_reasoning = supports_reasoning(model);
        let is_claude = is_claude_model(model);

        if is_reasoning {
            if is_claude {
                &[
                    "temperature",
                    "top_p",
                    "max_tokens",
                    "max_completion_tokens",
                    "stream",
                    "stop",
                    "frequency_penalty",
                    "presence_penalty",
                    "n",
                    "response_format",
                    "seed",
                    "tools",
                    "tool_choice",
                    "user",
                    "thinking",
                    "reasoning_effort",
                ]
            } else {
                &[
                    "temperature",
                    "top_p",
                    "max_tokens",
                    "max_completion_tokens",
                    "stream",
                    "stop",
                    "frequency_penalty",
                    "presence_penalty",
                    "n",
                    "response_format",
                    "seed",
                    "tools",
                    "tool_choice",
                    "user",
                    "reasoning_effort",
                ]
            }
        } else {
            &[
                "temperature",
                "top_p",
                "max_tokens",
                "max_completion_tokens",
                "stream",
                "stop",
                "frequency_penalty",
                "presence_penalty",
                "n",
                "response_format",
                "seed",
                "tools",
                "tool_choice",
                "parallel_tool_calls",
                "user",
                "logprobs",
                "top_logprobs",
            ]
        }
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, ProviderError> {
        // GitHub Copilot uses the same parameters as OpenAI
        Ok(params)
    }

    async fn transform_request(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, ProviderError> {
        // Transform messages
        self.transform_messages(&mut request.messages);

        // Convert to JSON value
        serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("github_copilot", e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let chat_response: ChatResponse = serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::api_error(
                "github_copilot",
                500,
                format!("Failed to parse response: {}", e),
            )
        })?;

        Ok(chat_response)
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(crate::core::traits::error_mapper::DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("GitHub Copilot chat request: model={}", request.model);

        // Transform messages
        self.transform_messages(&mut request.messages);

        // Build headers
        let headers = self.build_headers(&request.messages).await?;
        self.require_endpoint(&request.model, "/chat/completions", &headers)
            .await?;

        // Build URL
        let api_base = self.get_api_base().await;
        let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

        // Execute request
        let client = crate::core::http::outbound::default_outbound_client().clone();
        let response = client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::network("github_copilot", e.to_string()))?;

        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("github_copilot", e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&body);
            let status_code = status.as_u16();

            // Clear cache on auth errors
            if status_code == 401 {
                self.clear_cache().await;
            }

            return Err(match status_code {
                401 => ProviderError::authentication("github_copilot", "Invalid API key or token"),
                404 => ProviderError::model_not_found("github_copilot", body_str.to_string()),
                429 => ProviderError::rate_limit("github_copilot", None),
                400 => ProviderError::invalid_request("github_copilot", body_str.to_string()),
                500..=599 => {
                    ProviderError::provider_unavailable("github_copilot", body_str.to_string())
                }
                _ => HttpErrorMapper::map_status_code("github_copilot", status_code, &body_str),
            });
        }

        serde_json::from_slice(&body).map_err(|e| {
            ProviderError::api_error(
                "github_copilot",
                500,
                format!("Failed to parse response: {}", e),
            )
        })
    }

    async fn chat_completion_stream(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        debug!("GitHub Copilot streaming request: model={}", request.model);

        // Transform messages
        self.transform_messages(&mut request.messages);

        // Enable streaming
        request.stream = true;

        // Build headers
        let headers = self.build_headers(&request.messages).await?;
        self.require_endpoint(&request.model, "/chat/completions", &headers)
            .await?;

        // Build URL
        let api_base = self.get_api_base().await;
        let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

        // Execute request
        let client = crate::core::http::outbound::streaming_outbound_client().clone();
        let response = crate::core::providers::base::connection_pool::send_streaming_request(
            client.post(&url).headers(headers).json(&request),
            "github_copilot",
        )
        .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_str =
                crate::core::providers::base::connection_pool::read_streaming_error_body(response)
                    .await
                    .map_err(|err| err.into_provider_error("github_copilot"))?;

            // Clear cache on auth errors
            if status == 401 {
                self.clear_cache().await;
            }

            return Err(match status {
                401 => ProviderError::authentication("github_copilot", "Invalid API key or token"),
                404 => ProviderError::model_not_found("github_copilot", body_str.clone()),
                429 => ProviderError::rate_limit("github_copilot", None),
                400 => ProviderError::invalid_request("github_copilot", body_str.clone()),
                500..=599 => {
                    ProviderError::provider_unavailable("github_copilot", body_str.clone())
                }
                _ => HttpErrorMapper::map_status_code("github_copilot", status, &body_str),
            });
        }

        // Create SSE stream
        let stream = GitHubCopilotStream::new(response.bytes_stream());
        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        debug!("GitHub Copilot embeddings request: model={}", request.model);

        // Build headers
        let api_key = self.get_api_key().await?;
        let headers_map = get_copilot_default_headers(&api_key);
        let mut headers = reqwest::header::HeaderMap::new();
        for (key, value) in headers_map {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                value.parse(),
            ) {
                headers.insert(name, val);
            }
        }

        // Build URL
        let api_base = self.get_api_base().await;
        let url = format!("{}/embeddings", api_base.trim_end_matches('/'));

        // Execute request
        let client = crate::core::http::outbound::default_outbound_client().clone();
        let response = client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::network("github_copilot", e.to_string()))?;

        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("github_copilot", e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&body);
            let status_code = status.as_u16();
            return Err(match status_code {
                401 => ProviderError::authentication("github_copilot", "Invalid API key or token"),
                404 => ProviderError::model_not_found("github_copilot", body_str.to_string()),
                429 => ProviderError::rate_limit("github_copilot", None),
                400 => ProviderError::invalid_request("github_copilot", body_str.to_string()),
                500..=599 => {
                    ProviderError::provider_unavailable("github_copilot", body_str.to_string())
                }
                _ => HttpErrorMapper::map_status_code("github_copilot", status_code, &body_str),
            });
        }

        serde_json::from_slice(&body).map_err(|e| {
            ProviderError::api_error(
                "github_copilot",
                500,
                format!("Failed to parse response: {}", e),
            )
        })
    }

    async fn health_check(&self) -> HealthStatus {
        // Try to get API key as health check
        match self.get_api_key().await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        // GitHub Copilot is subscription-based, no per-token cost
        Ok(0.0)
    }
}

/// SSE stream implementation for GitHub Copilot
pub struct GitHubCopilotStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    buffer: String,
}

impl GitHubCopilotStream {
    pub fn new(stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static) -> Self {
        Self {
            inner: Box::pin(stream),
            buffer: String::new(),
        }
    }

    fn parse_sse_line(&self, line: &str) -> Option<Result<ChatChunk, ProviderError>> {
        if line.is_empty() || line.starts_with(':') {
            return None;
        }

        if let Some(data) = line.strip_prefix("data: ") {
            let data = data.trim();

            if is_done_marker(data) {
                return None;
            }

            match serde_json::from_str::<ChatChunk>(data) {
                Ok(chunk) => Some(Ok(chunk)),
                Err(e) => Some(Err(ProviderError::api_error(
                    "github_copilot",
                    500,
                    format!("Failed to parse chunk: {}", e),
                ))),
            }
        } else {
            None
        }
    }
}

impl Stream for GitHubCopilotStream {
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            // Check if we have complete lines in the buffer
            if let Some(newline_pos) = self.buffer.find('\n') {
                let line = self.buffer[..newline_pos].to_string();
                self.buffer = self.buffer[newline_pos + 1..].to_string();

                if let Some(result) = self.parse_sse_line(&line) {
                    return std::task::Poll::Ready(Some(result));
                }
                continue;
            }

            // Need more data
            match self.inner.as_mut().poll_next(cx) {
                std::task::Poll::Ready(Some(Ok(bytes))) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&bytes));
                }
                std::task::Poll::Ready(Some(Err(e))) => {
                    return std::task::Poll::Ready(Some(Err(ProviderError::network(
                        "github_copilot",
                        e.to_string(),
                    ))));
                }
                std::task::Poll::Ready(None) => {
                    // Stream ended, check remaining buffer
                    if !self.buffer.is_empty() {
                        let line = std::mem::take(&mut self.buffer);
                        if let Some(result) = self.parse_sse_line(&line) {
                            return std::task::Poll::Ready(Some(result));
                        }
                    }
                    return std::task::Poll::Ready(None);
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::message::MessageContent;

    #[tokio::test]
    async fn test_github_copilot_provider_creation() {
        let provider = GitHubCopilotProvider::new(GitHubCopilotConfig::default())
            .await
            .expect("provider should build");
        assert_eq!(provider.name(), "github_copilot");
    }

    #[tokio::test]
    async fn test_github_copilot_provider_capabilities() {
        let provider = GitHubCopilotProvider::new(GitHubCopilotConfig::default())
            .await
            .expect("provider should build");
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ToolCalling));
    }

    #[tokio::test]
    async fn test_github_copilot_provider_models() {
        let provider = GitHubCopilotProvider::new(GitHubCopilotConfig::default())
            .await
            .expect("provider should build");
        let models = provider.models();

        assert!(!models.is_empty());
        let model_ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert!(model_ids.contains(&"gpt-6-sol"));
        assert!(model_ids.contains(&"claude-opus-5.5"));
    }

    #[tokio::test]
    async fn test_github_copilot_provider_supported_params() {
        let provider = GitHubCopilotProvider::new(GitHubCopilotConfig::default())
            .await
            .expect("provider should build");

        let params = provider.get_supported_openai_params("gpt-6-sol");
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"reasoning_effort"));

        let params = provider.get_supported_openai_params("gpt-6-astra");
        assert!(params.contains(&"reasoning_effort"));

        let params = provider.get_supported_openai_params("claude-opus-5.5");
        assert!(params.contains(&"thinking"));
        assert!(params.contains(&"reasoning_effort"));
    }

    #[test]
    fn test_github_copilot_stream_parse_done_marker() {
        let stream = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
        let parser = GitHubCopilotStream::new(stream);
        assert!(parser.parse_sse_line("data: [DONE]").is_none());
    }

    #[test]
    fn test_github_copilot_stream_parse_valid_chunk() {
        let stream = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
        let parser = GitHubCopilotStream::new(stream);
        let line = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;

        let parsed = parser
            .parse_sse_line(line)
            .expect("expected parser to return a chunk result");
        let chunk = parsed.expect("expected valid chat chunk");
        assert_eq!(chunk.id, "chatcmpl-123");
        assert_eq!(chunk.choices.len(), 1);
    }

    fn text_message(role: MessageRole, text: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: Some(MessageContent::Text(text.to_string())),
            ..Default::default()
        }
    }

    #[test]
    fn test_determine_initiator() {
        let config = GitHubCopilotConfig::default();
        let provider = GitHubCopilotProvider {
            authenticator: CopilotAuthenticator::new(&config),
            config,
            models: vec![],
            cached_api_key: Arc::new(RwLock::new(None)),
            cached_api_base: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            native_endpoint_access: crate::core::net::ProviderEndpointAccess::PublicOnly,
        };

        let messages = vec![text_message(MessageRole::User, "Hello")];
        assert_eq!(provider.determine_initiator(&messages), "user");

        let messages = vec![
            text_message(MessageRole::User, "Hello"),
            text_message(MessageRole::Assistant, "Hi!"),
        ];
        assert_eq!(provider.determine_initiator(&messages), "agent");
    }

    #[tokio::test]
    async fn test_github_copilot_provider_cost_calculation() {
        let provider = GitHubCopilotProvider::new(GitHubCopilotConfig::default())
            .await
            .expect("provider should build");

        let cost = provider.calculate_cost("gpt-4o", 1000, 500).await;
        assert!(cost.is_ok());
        assert_eq!(cost.expect("cost should be available"), 0.0);
    }
}

#[cfg(all(test, feature = "gateway"))]
#[path = "native_tests.rs"]
mod native_tests;
