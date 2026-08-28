//! Anthropic request transformation.

use serde_json::{Value, json};

use crate::core::providers::anthropic::error::anthropic_api_error;
use crate::core::providers::anthropic::models::{ModelFeature, get_anthropic_registry};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use crate::core::types::message::MessageRole;
use crate::core::types::tools::ToolChoice;

use super::AnthropicClient;

#[derive(Debug, Clone, Copy)]
struct Claude5Protocol {
    thinking_always_on: bool,
}

impl AnthropicClient {
    pub(in crate::core::providers::anthropic) fn transform_chat_request(
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
        let claude_5_protocol = if self.config.uses_compatible_model_allow_list() {
            None
        } else {
            Self::claude_5_protocol(&request.model)
        };

        let model_spec = if self.config.uses_compatible_model_allow_list() {
            None
        } else {
            registry.get_model_spec(&request.model)
        };
        if model_spec.is_none()
            && claude_5_protocol.is_none()
            && !self.config.allows_unknown_model(&request.model)
        {
            return Err(anthropic_api_error(
                400,
                format!("Unsupported model: {}", request.model),
            ));
        }
        if let Some(protocol) = claude_5_protocol {
            Self::validate_claude_5_request(request, protocol)?;
        }
        if claude_5_protocol.is_none()
            && model_spec.is_some_and(|model_spec| {
                !model_spec.features.contains(&ModelFeature::ThinkingMode)
            })
            && request
                .messages
                .iter()
                .any(|message| message.thinking.is_some())
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!(
                    "Model {} does not support Anthropic thinking history",
                    request.model
                ),
            ));
        }
        if model_spec.is_none()
            && claude_5_protocol.is_none()
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
        if model_spec.is_none()
            && claude_5_protocol.is_none()
            && Self::has_unsupported_unknown_model_content(request)
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

        if request.stream {
            anthropic_request["stream"] = json!(true);
        }

        // Add tool support
        if let Some(tools) = &request.tools
            && !tools.is_empty()
        {
            if model_spec.is_none() && claude_5_protocol.is_none() {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!(
                        "Unknown model {} cannot declare tool calling support",
                        request.model
                    ),
                ));
            }
            if model_spec
                .is_some_and(|model_spec| !model_spec.features.contains(&ModelFeature::ToolCalling))
            {
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

        if request
            .thinking
            .as_ref()
            .is_some_and(|thinking| thinking.enabled)
            && claude_5_protocol.is_none()
            && request
                .tool_choice
                .as_ref()
                .is_some_and(Self::is_forced_tool_choice)
        {
            return Err(ProviderError::invalid_request(
                "anthropic",
                "Manual extended thinking only supports auto or none tool_choice",
            ));
        }

        // Add thinking configuration
        if let Some(thinking) = &request.thinking {
            if claude_5_protocol.is_some() {
                if thinking.enabled {
                    anthropic_request["thinking"] = json!({
                        "type": "adaptive",
                        "display": if thinking.include_thinking { "summarized" } else { "omitted" }
                    });
                } else {
                    anthropic_request["thinking"] = json!({"type": "disabled"});
                }
                if thinking.enabled
                    && let Some(effort) = thinking.effort
                {
                    anthropic_request["output_config"] = json!({"effort": effort.as_str()});
                }
            } else if model_spec.is_none() {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!(
                        "Unknown model {} cannot declare thinking support",
                        request.model
                    ),
                ));
            } else if thinking.enabled
                && model_spec.is_some_and(|model_spec| {
                    !model_spec.features.contains(&ModelFeature::ThinkingMode)
                })
            {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!("Model {} does not support thinking", request.model),
                ));
            } else if thinking.enabled {
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

    pub(in crate::core::providers::anthropic) fn is_claude_5_protocol_model(model: &str) -> bool {
        Self::claude_5_protocol(model).is_some()
    }

    fn claude_5_protocol(model: &str) -> Option<Claude5Protocol> {
        match model {
            "claude-fable-5" => Some(Claude5Protocol {
                thinking_always_on: true,
            }),
            "claude-opus-5" | "claude-sonnet-5" => Some(Claude5Protocol {
                thinking_always_on: false,
            }),
            _ => None,
        }
    }

    fn validate_claude_5_request(
        request: &ChatRequest,
        protocol: Claude5Protocol,
    ) -> Result<(), ProviderError> {
        if request
            .temperature
            .is_some_and(|temperature| temperature != 1.0)
        {
            return Err(ProviderError::invalid_request(
                "anthropic",
                format!(
                    "Model {} does not support non-default temperature",
                    request.model
                ),
            ));
        }
        if request
            .top_p
            .is_some_and(|top_p| !top_p.is_finite() || !(0.99..=1.0).contains(&top_p))
        {
            return Err(ProviderError::invalid_request(
                "anthropic",
                format!("Model {} does not support non-default top_p", request.model),
            ));
        }
        if request.extra_params.contains_key("top_k") {
            return Err(ProviderError::invalid_request(
                "anthropic",
                format!("Model {} does not support top_k", request.model),
            ));
        }
        if request
            .messages
            .iter()
            .rev()
            .find(|message| !matches!(message.role, MessageRole::System | MessageRole::Developer))
            .is_some_and(|message| message.role == MessageRole::Assistant)
        {
            return Err(ProviderError::invalid_request(
                "anthropic",
                format!("Model {} does not support assistant prefill", request.model),
            ));
        }
        if protocol.thinking_always_on
            && request
                .thinking
                .as_ref()
                .is_some_and(|thinking| !thinking.enabled)
        {
            return Err(ProviderError::invalid_request(
                "anthropic",
                format!("Model {} cannot disable thinking", request.model),
            ));
        }
        if request
            .functions
            .as_ref()
            .is_some_and(|functions| !functions.is_empty())
            || request.function_call.is_some()
        {
            return Err(ProviderError::not_supported(
                "anthropic",
                format!(
                    "Model {} does not support legacy functions/function_call; use tools/tool_choice",
                    request.model
                ),
            ));
        }
        if request
            .thinking
            .as_ref()
            .is_some_and(|thinking| thinking.enabled && thinking.budget_tokens.is_some())
        {
            return Err(ProviderError::invalid_request(
                "anthropic",
                format!(
                    "Model {} uses adaptive thinking and does not support budget_tokens",
                    request.model
                ),
            ));
        }

        Ok(())
    }

    fn is_forced_tool_choice(tool_choice: &ToolChoice) -> bool {
        match tool_choice {
            ToolChoice::String(choice) => choice == "required",
            ToolChoice::Specific { function, .. } => function.is_some(),
        }
    }
}
