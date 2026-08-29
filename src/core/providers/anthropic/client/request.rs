//! Anthropic request transformation.

use serde_json::{Value, json};

use crate::core::providers::anthropic::error::anthropic_api_error;
use crate::core::providers::anthropic::models::{ModelFeature, get_anthropic_registry};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::providers::{ChatContinuationRequest, ChatContinuationResponse};
use crate::core::types::anthropic_continuation::{
    AnthropicContentBlockOrder, AnthropicThinkingBlock, ChatMessageExtensions,
};
use crate::core::types::chat::ChatRequest;
use crate::core::types::message::MessageRole;
use crate::core::types::thinking::ThinkingEffort;

use super::AnthropicClient;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AnthropicEffort {
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl AnthropicEffort {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

impl From<ThinkingEffort> for AnthropicEffort {
    fn from(value: ThinkingEffort) -> Self {
        match value {
            ThinkingEffort::Low => Self::Low,
            ThinkingEffort::Medium => Self::Medium,
            ThinkingEffort::High => Self::High,
        }
    }
}

impl AnthropicClient {
    pub(crate) async fn chat_with_continuation(
        &self,
        envelope: ChatContinuationRequest,
    ) -> Result<ChatContinuationResponse, ProviderError> {
        let (request, extensions) = envelope.into_parts();
        let tool_name_map = self.anthropic_tool_name_map_for_request(&request)?;
        let body = self.transform_chat_request_with_extensions(&request, &extensions)?;
        let mut headers = self.get_request_headers();
        headers.extend(self.compute_beta_headers(&request));
        let response = self.send_request("/v1/messages", body, headers).await?;
        self.transform_chat_response_with_continuation(response, &tool_name_map)
    }

    pub(crate) fn transform_chat_request(
        &self,
        request: &ChatRequest,
    ) -> Result<Value, ProviderError> {
        if self.config.uses_compatible_model_allow_list()
            && !self.config.allows_unknown_model(&request.model)
        {
            return Err(anthropic_api_error(
                400,
                format!("Unsupported model: {}", request.model),
            ));
        }

        let registry = get_anthropic_registry();
        let model_spec = if self.config.uses_compatible_model_allow_list() {
            None
        } else {
            registry.get_model_spec(&request.model)
        };
        let claude_5 = !self.config.uses_compatible_model_allow_list()
            && Self::is_claude_5_protocol_model(&request.model)
            && (model_spec.is_some()
                || Self::is_standalone_claude_5_protocol_model(&request.model));
        if model_spec.is_none() && !claude_5 && !self.config.allows_unknown_model(&request.model) {
            return Err(anthropic_api_error(
                400,
                format!("Unsupported model: {}", request.model),
            ));
        }
        if claude_5 {
            Self::validate_claude_5_legacy_functions(request)?;
            Self::validate_claude_5_sampling(request)?;
        } else if request.reasoning_effort.is_some() {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!(
                    "Model {} does not support reasoning_effort; exact Claude 5 models are required",
                    request.model
                ),
            ));
        }
        if model_spec.is_none()
            && !claude_5
            && (request
                .tools
                .as_ref()
                .is_some_and(|tools| !tools.is_empty())
                || Self::has_anthropic_tools_extra_param(request)
                || request.functions.as_ref().is_some_and(|f| !f.is_empty())
                || request.function_call.is_some())
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!(
                    "Unknown model {} cannot declare tool calling support",
                    request.model
                ),
            ));
        }
        if model_spec.is_none() && !claude_5 && Self::has_unsupported_unknown_model_content(request)
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!(
                    "Unknown model {} only supports text and image content",
                    request.model
                ),
            ));
        }
        if model_spec.is_none()
            && !claude_5
            && Self::has_image_content(request)
            && !self.config.allows_unknown_model_image_input(&request.model)
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!(
                    "Unknown model {} does not support image input",
                    request.model
                ),
            ));
        }

        // The Messages API only returns a single candidate; any n other than 1
        // (including 0) cannot be honored, so reject it instead of silently
        // returning the wrong number of choices.
        if let Some(n) = request.n
            && n != 1
        {
            return Err(ProviderError::invalid_request(
                "anthropic",
                format!("anthropic only supports n=1 (got n={})", n),
            ));
        }

        // Warn once about OpenAI-style parameters Anthropic has no equivalent for.
        let mut ignored_params = Vec::new();
        if request.frequency_penalty.is_some() {
            ignored_params.push("frequency_penalty");
        }
        if request.presence_penalty.is_some() {
            ignored_params.push("presence_penalty");
        }
        if request.seed.is_some() {
            ignored_params.push("seed");
        }
        if request.logit_bias.is_some() {
            ignored_params.push("logit_bias");
        }
        if !ignored_params.is_empty() {
            tracing::warn!(
                "Anthropic request ignores unsupported parameters: {}",
                ignored_params.join(", ")
            );
        }

        let (system_message, messages) = self.separate_system_messages(&request.messages)?;
        let tool_name_map = self.anthropic_tool_name_map_for_request(request)?;

        let anthropic_messages =
            self.transform_messages(messages, &request.model, model_spec, &tool_name_map)?;

        let mut anthropic_request = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "messages": anthropic_messages,
        });

        if let Some(system) = system_message {
            anthropic_request["system"] = json!(system);
        }

        // Add optional parameters
        if let Some(temperature) = request.temperature {
            anthropic_request["temperature"] = json!(temperature);
        }

        if let Some(top_p) = request.top_p {
            anthropic_request["top_p"] = json!(top_p);
        }

        if let Some(user) = &request.user {
            anthropic_request["metadata"] = json!({ "user_id": user });
        }

        if self.config.enable_cache_control
            && let Some(cache_control) = request.extra_params.get("cache_control")
        {
            Self::ensure_cache_control_supported(&request.model, model_spec)?;
            anthropic_request["cache_control"] = cache_control.clone();
        }

        if let Some(stop) = &request.stop {
            anthropic_request["stop_sequences"] = json!(stop);
        }

        // Add tool support
        if let Some(tools) = &request.tools
            && !tools.is_empty()
        {
            if model_spec.is_none() && !claude_5 {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!(
                        "Unknown model {} cannot declare tool calling support",
                        request.model
                    ),
                ));
            }
            if model_spec.is_some_and(|spec| !spec.features.contains(&ModelFeature::ToolCalling)) {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!("Model {} does not support tool calling", request.model),
                ));
            }
            let anthropic_tools = self.transform_tools(tools)?;
            anthropic_request["tools"] = json!(anthropic_tools);

            if let Some(tool_choice) = &request.tool_choice {
                anthropic_request["tool_choice"] =
                    self.transform_tool_choice(tool_choice, &tool_name_map)?;
            }
        }

        // Add thinking configuration. Claude 5 reasoning_effort and typed
        // thinking are normalized here so Chat and Responses share one rule.
        if claude_5 {
            if let Some((thinking, effort)) = Self::claude_5_thinking_config(request)? {
                anthropic_request["thinking"] = thinking;
                if let Some(effort) = effort {
                    anthropic_request["output_config"] = json!({"effort": effort.as_str()});
                }
            }
        } else if let Some(thinking) = &request.thinking
            && thinking.enabled
        {
            let Some(model_spec) = model_spec else {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!(
                        "Unknown model {} cannot declare thinking support",
                        request.model
                    ),
                ));
            };
            if !model_spec.features.contains(&ModelFeature::ThinkingMode) {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!("Model {} does not support thinking", request.model),
                ));
            }
            let budget = thinking.budget_tokens.unwrap_or(10_000);
            // Anthropic requires max_tokens > budget_tokens. If the default (4096)
            // is not greater than budget_tokens, raise max_tokens to budget + 1.
            let current_max = request.max_tokens.unwrap_or(4096);
            if current_max <= budget {
                anthropic_request["max_tokens"] = json!(budget + 1);
            }
            anthropic_request["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": budget
            });
        }

        // Structured outputs: pass json_schema response_format to Anthropic.
        if let Some(rf) = &request.response_format
            && rf.format_type == "json_schema"
            && let Some(schema) = &rf.json_schema
        {
            anthropic_request["response_format"] = json!({
                "type": "json_schema",
                "json_schema": schema
            });
        }

        // Anthropic built-in (server-side) tools passed via extra_params.
        // These are appended after any user-defined function tools.
        if let Some(arr) = request
            .extra_params
            .get("anthropic_tools")
            .and_then(|v| v.as_array())
            .filter(|a| !a.is_empty())
        {
            let mut merged: Vec<Value> = anthropic_request
                .get("tools")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            merged.extend(arr.iter().cloned());
            anthropic_request["tools"] = json!(merged);
        }

        Ok(anthropic_request)
    }

    pub(crate) fn transform_chat_request_with_extensions(
        &self,
        request: &ChatRequest,
        extensions: &[ChatMessageExtensions],
    ) -> Result<Value, ProviderError> {
        if request.messages.len() != extensions.len() {
            return Err(ProviderError::invalid_request(
                "anthropic",
                "message extension length mismatch",
            ));
        }
        let mut transformed = self.transform_chat_request(request)?;
        let messages = transformed["messages"].as_array_mut().ok_or_else(|| {
            anthropic_api_error(500, "Anthropic request messages must be an array")
        })?;
        let mut wire_index = 0;
        for (message, extension) in request.messages.iter().zip(extensions) {
            if matches!(message.role, MessageRole::System | MessageRole::Developer) {
                if !extension.is_empty() {
                    return Err(ProviderError::invalid_request(
                        "anthropic",
                        "Anthropic continuation is only valid on assistant messages",
                    ));
                }
                continue;
            }
            let wire_message = messages.get_mut(wire_index).ok_or_else(|| {
                anthropic_api_error(500, "Anthropic message extension index drift")
            })?;
            wire_index += 1;
            let Some(thinking) = extension.anthropic_thinking() else {
                continue;
            };
            if message.role != MessageRole::Assistant {
                return Err(ProviderError::invalid_request(
                    "anthropic",
                    "Anthropic continuation is only valid on assistant messages",
                ));
            }
            let wire_content = wire_message
                .get_mut("content")
                .ok_or_else(|| anthropic_api_error(500, "Anthropic message content is missing"))?;
            let mut content = match std::mem::take(wire_content) {
                Value::Array(blocks) => blocks,
                Value::String(text) if text.is_empty() => Vec::new(),
                Value::String(text) => vec![json!({"type": "text", "text": text})],
                Value::Null => Vec::new(),
                _ => {
                    return Err(anthropic_api_error(
                        500,
                        "Anthropic message content must be text or an array",
                    ));
                }
            };
            if let Some(order) = extension.anthropic_block_order() {
                content = Self::replay_ordered_continuation(content, thinking.blocks(), order)?;
            } else {
                content.splice(
                    0..0,
                    thinking.blocks().iter().map(Self::thinking_block_to_value),
                );
            }
            *wire_content = Value::Array(content);
        }
        Ok(transformed)
    }

    fn thinking_block_to_value(block: &AnthropicThinkingBlock) -> Value {
        match block {
            AnthropicThinkingBlock::Thinking {
                thinking,
                signature,
            } => json!({
                "type": "thinking",
                "thinking": thinking,
                "signature": signature.expose(),
            }),
            AnthropicThinkingBlock::RedactedThinking { data } => json!({
                "type": "redacted_thinking",
                "data": data.expose(),
            }),
        }
    }

    fn replay_ordered_continuation(
        content: Vec<Value>,
        thinking: &[AnthropicThinkingBlock],
        order: &[AnthropicContentBlockOrder],
    ) -> Result<Vec<Value>, ProviderError> {
        let (tool_blocks, remaining): (Vec<_>, Vec<_>) = content
            .into_iter()
            .partition(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"));
        let mut seen_thinking = vec![false; thinking.len()];
        let mut seen_tools = vec![false; tool_blocks.len()];
        let mut replay = Vec::with_capacity(order.len() + remaining.len());

        for marker in order {
            match *marker {
                AnthropicContentBlockOrder::Thinking { index } => {
                    let Some(block) = thinking.get(index) else {
                        return Err(ProviderError::invalid_request(
                            "anthropic",
                            format!(
                                "Anthropic continuation references missing thinking block {index}"
                            ),
                        ));
                    };
                    if std::mem::replace(&mut seen_thinking[index], true) {
                        return Err(ProviderError::invalid_request(
                            "anthropic",
                            format!("Anthropic continuation repeats thinking block {index}"),
                        ));
                    }
                    replay.push(Self::thinking_block_to_value(block));
                }
                AnthropicContentBlockOrder::ToolUse { index } => {
                    let Some(block) = tool_blocks.get(index) else {
                        return Err(ProviderError::invalid_request(
                            "anthropic",
                            format!(
                                "Anthropic continuation references missing tool-use block {index}"
                            ),
                        ));
                    };
                    if std::mem::replace(&mut seen_tools[index], true) {
                        return Err(ProviderError::invalid_request(
                            "anthropic",
                            format!("Anthropic continuation repeats tool-use block {index}"),
                        ));
                    }
                    replay.push(block.clone());
                }
            }
        }
        if seen_thinking.iter().any(|seen| !seen) || seen_tools.iter().any(|seen| !seen) {
            return Err(ProviderError::invalid_request(
                "anthropic",
                "Anthropic continuation block order does not cover every thinking and tool-use block",
            ));
        }
        replay.extend(remaining);
        Ok(replay)
    }

    pub(crate) fn is_claude_5_protocol_model(model: &str) -> bool {
        matches!(
            model,
            "claude-fable-5" | "claude-opus-5" | "claude-sonnet-5"
        )
    }

    /// Claude 5 models without a registry entry are unsupported by default.
    /// Fable is the sole temporary exception because the runtime pricing
    /// authority already contains its exact ID. Opus and Sonnet remain
    /// unsupported until #1216/#1222 supply exact callable and priced entries;
    /// activation must come from that authority rather than another name rule.
    pub(crate) fn is_standalone_claude_5_protocol_model(model: &str) -> bool {
        model == "claude-fable-5"
    }

    fn validate_claude_5_sampling(request: &ChatRequest) -> Result<(), ProviderError> {
        if request.model != "claude-fable-5" {
            return Ok(());
        }

        let mut unsupported = Vec::with_capacity(2);
        if request.temperature.is_some() {
            unsupported.push("temperature");
        }
        if request.top_p.is_some() {
            unsupported.push("top_p");
        }
        if unsupported.is_empty() {
            return Ok(());
        }

        Err(ProviderError::not_supported(
            "anthropic",
            format!(
                "Model claude-fable-5 does not support {}",
                unsupported.join(" or ")
            ),
        ))
    }

    fn validate_claude_5_legacy_functions(request: &ChatRequest) -> Result<(), ProviderError> {
        if request
            .functions
            .as_ref()
            .is_some_and(|functions| !functions.is_empty())
            || request.function_call.is_some()
            || request
                .messages
                .iter()
                .any(|message| message.function_call.is_some())
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!(
                    "Model {} does not support legacy functions/function_call; use tools/tool_choice",
                    request.model
                ),
            ));
        }
        Ok(())
    }

    pub(super) fn claude_5_thinking_config(
        request: &ChatRequest,
    ) -> Result<Option<(Value, Option<AnthropicEffort>)>, ProviderError> {
        let requested_effort = request
            .reasoning_effort
            .as_deref()
            .map(Self::parse_reasoning_effort)
            .transpose()?;
        let Some(thinking) = request.thinking.as_ref() else {
            return Ok(match requested_effort {
                Some(effort) => Some((
                    json!({"type": "adaptive", "display": "summarized"}),
                    Some(effort),
                )),
                None if request.model == "claude-fable-5" => {
                    Some((json!({"type": "adaptive", "display": "summarized"}), None))
                }
                None => None,
            });
        };

        if !thinking.enabled {
            if request.model == "claude-fable-5" {
                return Err(ProviderError::invalid_request(
                    "anthropic",
                    "claude-fable-5 cannot disable thinking",
                ));
            }
            if requested_effort.is_some() {
                return Err(ProviderError::invalid_request(
                    "anthropic",
                    "reasoning_effort conflicts with thinking.enabled=false",
                ));
            }
            return Ok(Some((json!({"type": "disabled"}), None)));
        }
        if thinking.budget_tokens.is_some() {
            return Err(ProviderError::invalid_request(
                "anthropic",
                "Claude 5 adaptive thinking does not support budget_tokens",
            ));
        }
        let typed_effort = thinking.effort.map(AnthropicEffort::from);
        if let (Some(typed), Some(requested)) = (typed_effort, requested_effort)
            && typed != requested
        {
            return Err(ProviderError::invalid_request(
                "anthropic",
                format!(
                    "Conflicting thinking effort values: thinking.effort={}, reasoning_effort={}",
                    typed.as_str(),
                    requested.as_str()
                ),
            ));
        }
        let effort = typed_effort.or(requested_effort);
        Ok(Some((
            json!({
                "type": "adaptive",
                "display": if thinking.include_thinking { "summarized" } else { "omitted" }
            }),
            effort,
        )))
    }

    fn parse_reasoning_effort(value: &str) -> Result<AnthropicEffort, ProviderError> {
        match value {
            "low" => Ok(AnthropicEffort::Low),
            "medium" => Ok(AnthropicEffort::Medium),
            "high" => Ok(AnthropicEffort::High),
            "xhigh" => Ok(AnthropicEffort::XHigh),
            "max" => Ok(AnthropicEffort::Max),
            other => Err(ProviderError::invalid_request(
                "anthropic",
                format!(
                    "Unsupported reasoning_effort '{other}'; expected low, medium, high, xhigh, or max"
                ),
            )),
        }
    }
}
