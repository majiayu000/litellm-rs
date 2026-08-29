//! Anthropic Client
//!
//! Error handling

use std::time::Duration;

use reqwest::Response;
use serde_json::{Value, json};
use tokio::time::timeout;

use crate::core::providers::base::{
    BaseConfig, BaseHttpClient, HeaderPair, apply_provider_headers, header, header_owned,
    header_static, read_streaming_error_body,
};
#[cfg(test)]
use crate::core::providers::shared::parse_retry_after_from_body;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::{ChatMessage, ChatRequest},
    content::ContentPart,
    message::{MessageContent, MessageRole},
    responses::ChatResponse,
    tools::{Tool, ToolChoice},
};

use super::config::AnthropicConfig;
use super::error::{
    AnthropicErrorMapper, anthropic_api_error, anthropic_network_error, anthropic_parse_error,
};
#[cfg(test)]
use super::models::get_anthropic_registry;
use super::models::{ModelFeature, ModelSpec};

const EXTENDED_CACHE_TTL_BETA: &str = "extended-cache-ttl-2025-04-11";

/// Anthropic API client
#[derive(Debug, Clone)]
pub struct AnthropicClient {
    config: AnthropicConfig,
    http_client: BaseHttpClient,
    streaming_client: BaseHttpClient,
}

impl AnthropicClient {
    /// Create
    pub fn new(config: AnthropicConfig) -> Result<Self, ProviderError> {
        config
            .validate_policy_client_settings()
            .map_err(|error| ProviderError::configuration("anthropic", error))?;
        let base_config = BaseConfig {
            api_base: Some(config.base_url.clone()),
            endpoint_access: config.endpoint_access,
            timeout: config.request_timeout,
            ..Default::default()
        };
        let http_client = BaseHttpClient::new_for_provider("anthropic", base_config.clone())?;
        let streaming_client =
            BaseHttpClient::new_for_provider_streaming("anthropic", base_config)?;

        Ok(Self {
            config,
            http_client,
            streaming_client,
        })
    }

    pub(crate) fn allows_unknown_model(&self, model: &str) -> bool {
        self.config.allows_unknown_model(model)
    }

    pub(crate) fn uses_compatible_model_allow_list(&self) -> bool {
        self.config.uses_compatible_model_allow_list()
    }

    pub(crate) fn allows_unknown_model_image_input(&self, model: &str) -> bool {
        self.config.allows_unknown_model_image_input(model)
    }

    /// Request
    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let tool_name_map = self.anthropic_tool_name_map_for_request(&request)?;
        let anthropic_request = self.transform_chat_request(&request)?;
        let mut headers = self.get_request_headers();
        headers.extend(self.compute_beta_headers(&request));
        let response = self
            .send_request("/v1/messages", anthropic_request, headers)
            .await?;
        self.transform_chat_response_with_tool_name_map(response, &tool_name_map)
    }

    /// Request
    pub async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<reqwest::Response, ProviderError> {
        let mut anthropic_request = self.transform_chat_request(&request)?;
        anthropic_request["stream"] = json!(true);
        let mut headers = self.get_request_headers();
        headers.extend(self.compute_beta_headers(&request));
        self.send_stream_request("/v1/messages", anthropic_request, headers)
            .await
    }

    /// Request
    async fn send_request(
        &self,
        endpoint: &str,
        body: Value,
        headers: Vec<HeaderPair>,
    ) -> Result<Value, ProviderError> {
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), endpoint);

        let response = timeout(
            Duration::from_secs(self.config.request_timeout),
            apply_provider_headers(self.http_client.post(&url)?.json(&body), headers).send(),
        )
        .await
        .map_err(|_| anthropic_network_error("Request timeout"))?
        .map_err(|e| anthropic_network_error(format!("Network error: {}", e)))?;

        self.handle_response(response).await
    }

    /// Request
    async fn send_stream_request(
        &self,
        endpoint: &str,
        body: Value,
        headers: Vec<HeaderPair>,
    ) -> Result<Response, ProviderError> {
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), endpoint);

        let response = timeout(
            Duration::from_secs(self.config.request_timeout),
            apply_provider_headers(self.streaming_client.post(&url)?.json(&body), headers).send(),
        )
        .await
        .map_err(|_| anthropic_network_error("Request timeout"))?
        .map_err(|e| anthropic_network_error(format!("Network error: {}", e)))?;

        // Check
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = read_streaming_error_body(response)
                .await
                .unwrap_or_else(|_| "failed to read upstream error body".to_string());
            return Err(self.map_http_error(status, &error_text));
        }

        Ok(response)
    }

    /// Build request headers using the unified HeaderPair pattern.
    pub fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(5);

        // Authentication header
        if let Some(ref api_key) = self.config.api_key {
            headers.push(header("x-api-key", api_key.clone()));
        }

        // Version header
        headers.push(header("anthropic-version", self.config.api_version.clone()));

        // Content type and user agent - zero allocation for static values
        headers.push(header_static("Content-Type", "application/json"));
        headers.push(header_static("User-Agent", "LiteLLM-Rust/1.0"));

        // Custom headers
        for (key, value) in &self.config.custom_headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }

    /// Compute the `anthropic-beta` header values required for the given request.
    ///
    /// Returns an empty Vec when no beta features are needed.
    fn compute_beta_headers(&self, request: &ChatRequest) -> Vec<HeaderPair> {
        self.compute_beta_headers_with_extensions(request, &[])
    }

    fn compute_beta_headers_with_extensions(
        &self,
        request: &ChatRequest,
        extensions: &[crate::core::providers::ChatMessageContinuation],
    ) -> Vec<HeaderPair> {
        let mut features: Vec<String> = Vec::new();

        // Extended / interleaved thinking requires the beta header.
        if request.thinking.as_ref().is_some_and(|t| t.enabled)
            || extensions.iter().any(|extension| {
                extension
                    .anthropic_thinking()
                    .is_some_and(|thinking| !thinking.blocks().is_empty())
            })
        {
            Self::push_beta_feature(&mut features, "interleaved-thinking-2025-05-14");
        }

        // Computer-use built-in tool requires its own beta header.
        if let Some(arr) = request
            .extra_params
            .get("anthropic_tools")
            .and_then(|v| v.as_array())
        {
            for tool in arr {
                if tool.get("type").and_then(|t| t.as_str()) == Some("computer_20241022") {
                    Self::push_beta_feature(&mut features, "computer-use-2024-10-22");
                    break;
                }
            }
        }

        if Self::request_uses_extended_cache_ttl(request) {
            Self::push_beta_feature(&mut features, EXTENDED_CACHE_TTL_BETA);
        }

        // Caller-supplied beta flags via extra_params["anthropic_beta"].
        if let Some(beta_val) = request.extra_params.get("anthropic_beta") {
            match beta_val {
                Value::Array(arr) => {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            Self::push_beta_feature(&mut features, s);
                        }
                    }
                }
                Value::String(s) => {
                    Self::push_beta_feature(&mut features, s);
                }
                _ => {}
            }
        }

        if features.is_empty() {
            return vec![];
        }

        vec![header("anthropic-beta", features.join(","))]
    }

    fn push_beta_feature(features: &mut Vec<String>, feature: &str) {
        if !features.iter().any(|existing| existing == feature) {
            features.push(feature.to_string());
        }
    }

    fn request_uses_extended_cache_ttl(request: &ChatRequest) -> bool {
        request
            .extra_params
            .get("cache_control")
            .is_some_and(Self::cache_control_value_uses_extended_ttl)
    }

    fn cache_control_value_uses_extended_ttl(value: &Value) -> bool {
        match value {
            Value::Object(map) => {
                map.get("ttl").and_then(Value::as_str) == Some("1h")
                    || map
                        .values()
                        .any(Self::cache_control_value_uses_extended_ttl)
            }
            Value::Array(values) => values
                .iter()
                .any(Self::cache_control_value_uses_extended_ttl),
            _ => false,
        }
    }

    /// Handle
    async fn handle_response(&self, response: Response) -> Result<Value, ProviderError> {
        let status = response.status().as_u16();
        let response_text = match response.text().await {
            Ok(response_text) => response_text,
            Err(_) if status != 200 => "failed to read upstream error body".to_string(),
            Err(error) => {
                return Err(anthropic_network_error(format!(
                    "Failed to read response: {error}"
                )));
            }
        };

        if status != 200 {
            return Err(self.map_http_error(status, &response_text));
        }

        serde_json::from_str(&response_text)
            .map_err(|e| anthropic_parse_error(format!("Failed to parse JSON: {}", e)))
    }

    /// Error
    fn map_http_error(&self, status: u16, body: &str) -> ProviderError {
        AnthropicErrorMapper::from_http_status(status, body)
    }
    /// Separate system messages from user messages
    pub(super) fn separate_system_messages(
        &self,
        messages: &[ChatMessage],
    ) -> Result<(Option<String>, Vec<ChatMessage>), ProviderError> {
        let mut system_parts = Vec::new();
        let mut user_messages = Vec::new();

        for message in messages {
            match message.role {
                MessageRole::System | MessageRole::Developer => {
                    if let Some(content) = &message.content {
                        match content {
                            MessageContent::Text(text) => {
                                system_parts.push(text.clone());
                            }
                            MessageContent::Parts(parts) => {
                                for part in parts {
                                    match part {
                                        ContentPart::Text { text } => {
                                            system_parts.push(text.clone());
                                        }
                                        other => {
                                            return Err(request_utils::unsupported_content_part(
                                                other,
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    user_messages.push(message.clone());
                }
            }
        }

        let system_message = if system_parts.is_empty() {
            None
        } else {
            Some(system_parts.join("\n"))
        };

        Ok((system_message, user_messages))
    }

    /// Transform messages to Anthropic format
    pub(super) fn transform_messages(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
        model_spec: Option<&ModelSpec>,
        tool_name_map: &request_utils::ToolNameMap,
    ) -> Result<Vec<Value>, ProviderError> {
        let mut anthropic_messages = Vec::new();

        for message in messages {
            if matches!(&message.role, MessageRole::Tool | MessageRole::Function) {
                let tool_use_id = message.tool_call_id.clone().ok_or_else(|| {
                    anthropic_parse_error("Tool/function message missing tool_call_id")
                })?;
                let content =
                    self.tool_result_content(message.content.clone(), model, model_spec)?;
                anthropic_messages.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": content
                    }]
                }));
                continue;
            }

            let role = match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool | MessageRole::Function => unreachable!(),
                MessageRole::System | MessageRole::Developer => continue, // Already handled
            };

            let content = if let Some(content) = message.content {
                match content {
                    MessageContent::Text(text) => json!(text),
                    MessageContent::Parts(parts) => {
                        let mut anthropic_parts = Vec::new();

                        for part in parts {
                            match part {
                                ContentPart::Text { text } => {
                                    anthropic_parts.push(json!({
                                        "type": "text",
                                            "text": text
                                    }));
                                }
                                ContentPart::ImageUrl { .. }
                                | ContentPart::Image { .. }
                                | ContentPart::Document { .. } => {
                                    anthropic_parts.push(self.content_part_to_anthropic_block(
                                        part, model, model_spec,
                                    )?);
                                }
                                ContentPart::ToolUse { id, name, input } if role == "assistant" => {
                                    anthropic_parts.push(json!({
                                        "type": "tool_use",
                                        "id": id,
                                        "name": request_utils::declared_tool_name(
                                            &name, tool_name_map, "Tool use"
                                        )?,
                                        "input": input
                                    }));
                                }
                                ContentPart::ToolResult {
                                    tool_use_id,
                                    content,
                                    is_error,
                                } if role == "user" => {
                                    let mut tool_result = json!({
                                        "type": "tool_result",
                                        "tool_use_id": tool_use_id,
                                        "content": content
                                    });
                                    if let Some(is_error) = is_error {
                                        tool_result["is_error"] = json!(is_error);
                                    }
                                    anthropic_parts.push(tool_result);
                                }
                                other => {
                                    return Err(request_utils::unsupported_content_part(&other));
                                }
                            }
                        }

                        json!(anthropic_parts)
                    }
                }
            } else {
                json!("")
            };

            let mut anthropic_message = json!({
                "role": role,
                "content": content
            });

            // Add tool_call
            if let Some(tool_calls) = &message.tool_calls {
                let mut anthropic_content =
                    Self::content_value_to_blocks(&anthropic_message["content"]);
                for tool_call in tool_calls {
                    let input = serde_json::from_str::<Value>(&tool_call.function.arguments)
                        .map_err(|error| {
                            ProviderError::invalid_request(
                                "anthropic",
                                format!(
                                    "Tool call '{}' arguments must be valid JSON: {}",
                                    tool_call.id, error
                                ),
                            )
                        })?;
                    anthropic_content.push(json!({
                        "type": "tool_use",
                        "id": tool_call.id,
                        "name": request_utils::declared_tool_name(
                            &tool_call.function.name, tool_name_map, "Tool call"
                        )?,
                        "input": input
                    }));
                }
                anthropic_message["content"] = json!(anthropic_content);
            }

            anthropic_messages.push(anthropic_message);
        }

        Ok(anthropic_messages)
    }

    fn content_part_to_anthropic_block(
        &self,
        part: ContentPart,
        model: &str,
        model_spec: Option<&ModelSpec>,
    ) -> Result<Value, ProviderError> {
        match part {
            ContentPart::Text { text } => Ok(json!({
                "type": "text",
                "text": text
            })),
            ContentPart::ImageUrl { image_url } => {
                if !Self::supports_multimodal_content(model_spec) {
                    return Err(ProviderError::not_supported(
                        "anthropic",
                        format!("Model {} does not support image content", model),
                    ));
                }
                if !image_url.url.starts_with("data:") {
                    return Err(anthropic_api_error(
                        400,
                        "URL images not yet supported, use base64 format",
                    ));
                }

                let (metadata, data) = image_url.url.split_once(',').ok_or_else(|| {
                    ProviderError::invalid_request("anthropic", "Invalid data URL image content")
                })?;
                let media_type = metadata
                    .strip_prefix("data:")
                    .and_then(|s| s.split(';').next())
                    .unwrap_or("image/jpeg");

                Ok(json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": media_type,
                        "data": data
                    }
                }))
            }
            ContentPart::Image { source, .. } => {
                if !Self::supports_multimodal_content(model_spec) {
                    return Err(ProviderError::not_supported(
                        "anthropic",
                        format!("Model {} does not support image content", model),
                    ));
                }
                Ok(json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": source.media_type,
                        "data": source.data
                    }
                }))
            }
            ContentPart::Document {
                source,
                cache_control,
            } => {
                if !Self::supports_multimodal_content(model_spec) {
                    return Err(ProviderError::not_supported(
                        "anthropic",
                        format!("Model {} does not support document content", model),
                    ));
                }
                let mut document = json!({
                    "type": "document",
                    "source": {
                        "type": "base64",
                        "media_type": source.media_type,
                        "data": source.data
                    }
                });
                if self.config.enable_cache_control
                    && let Some(cache_control) = cache_control
                {
                    Self::ensure_cache_control_supported(model, model_spec)?;
                    document["cache_control"] = json!(cache_control);
                }
                Ok(document)
            }
            other => Err(request_utils::unsupported_content_part(&other)),
        }
    }

    fn supports_multimodal_content(model_spec: Option<&ModelSpec>) -> bool {
        model_spec.is_none_or(|spec| spec.features.contains(&ModelFeature::MultimodalSupport))
    }

    pub(super) fn ensure_cache_control_supported(
        model: &str,
        model_spec: Option<&ModelSpec>,
    ) -> Result<(), ProviderError> {
        let Some(model_spec) = model_spec else {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!(
                    "Unknown model {} cannot declare cache control support",
                    model
                ),
            ));
        };
        if model_spec.features.contains(&ModelFeature::CacheControl) {
            return Ok(());
        }
        Err(ProviderError::not_supported(
            "anthropic",
            format!("Model {} does not support cache control", model),
        ))
    }

    pub(crate) fn has_multimodal_content(request: &ChatRequest) -> bool {
        request.messages.iter().any(|msg| {
            if let Some(crate::core::types::message::MessageContent::Parts(parts)) = &msg.content {
                parts
                    .iter()
                    .any(|part| !matches!(part, ContentPart::Text { .. }))
            } else {
                false
            }
        })
    }

    pub(crate) fn has_anthropic_tools_extra_param(request: &ChatRequest) -> bool {
        request
            .extra_params
            .get("anthropic_tools")
            .and_then(|value| value.as_array())
            .is_some_and(|tools| !tools.is_empty())
    }

    pub(crate) fn has_unsupported_unknown_model_content(request: &ChatRequest) -> bool {
        request.messages.iter().any(|message| {
            matches!(message.role, MessageRole::Tool | MessageRole::Function)
                || message.thinking.is_some()
                || message
                    .tool_calls
                    .as_ref()
                    .is_some_and(|calls| !calls.is_empty())
                || message.function_call.is_some()
                || matches!(
                    &message.content,
                    Some(crate::core::types::message::MessageContent::Parts(parts))
                        if parts.iter().any(|part| !matches!(
                            part,
                            ContentPart::Text { .. }
                                | ContentPart::ImageUrl { .. }
                                | ContentPart::Image { .. }
                        ))
                )
        })
    }

    pub(crate) fn has_image_content(request: &ChatRequest) -> bool {
        request.messages.iter().any(|message| {
            matches!(
                &message.content,
                Some(crate::core::types::message::MessageContent::Parts(parts))
                    if parts.iter().any(|part| matches!(
                        part,
                        ContentPart::ImageUrl { .. } | ContentPart::Image { .. }
                    ))
            )
        })
    }

    fn content_value_to_blocks(content: &Value) -> Vec<Value> {
        if let Some(text) = content.as_str() {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![json!({
                    "type": "text",
                    "text": text,
                })]
            }
        } else {
            content.as_array().cloned().unwrap_or_default()
        }
    }

    fn tool_result_content(
        &self,
        content: Option<MessageContent>,
        model: &str,
        model_spec: Option<&ModelSpec>,
    ) -> Result<Value, ProviderError> {
        match content {
            Some(MessageContent::Text(text)) => Ok(json!(text)),
            Some(MessageContent::Parts(parts)) => {
                let blocks = parts
                    .into_iter()
                    .map(|part| match part {
                        ContentPart::Text { .. }
                        | ContentPart::ImageUrl { .. }
                        | ContentPart::Image { .. }
                        | ContentPart::Document { .. } => {
                            self.content_part_to_anthropic_block(part, model, model_spec)
                        }
                        other => Err(request_utils::unsupported_content_part(&other)),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(json!(blocks))
            }
            None => Ok(json!("")),
        }
    }

    /// Transform tool definitions
    pub(super) fn transform_tools(&self, tools: &[Tool]) -> Result<Vec<Value>, ProviderError> {
        request_utils::anthropic_tools(tools)
    }

    /// Transform tool choice
    pub(super) fn transform_tool_choice(
        &self,
        tool_choice: &ToolChoice,
        tool_name_map: &request_utils::ToolNameMap,
    ) -> Result<Value, ProviderError> {
        match tool_choice {
            ToolChoice::String(choice) => match choice.as_str() {
                "auto" => Ok(json!({"type": "auto"})),
                "none" => Ok(json!({"type": "none"})),
                "required" => Ok(json!({"type": "any"})),
                _ => Ok(json!({"type": "auto"})),
            },
            ToolChoice::Specific { function, .. } => {
                if let Some(func) = function {
                    Ok(json!({
                        "type": "tool",
                        "name": request_utils::declared_tool_name(
                            &func.name, tool_name_map, "Tool choice"
                        )?
                    }))
                } else {
                    Ok(json!({"type": "auto"}))
                }
            }
        }
    }
}

mod request;
mod request_utils;
mod response;
mod usage;

#[cfg(test)]
mod compatible_tests;
#[cfg(test)]
mod request_tests;
#[cfg(test)]
mod tests;
