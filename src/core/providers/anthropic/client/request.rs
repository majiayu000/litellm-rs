//! Anthropic request transformation.

use serde_json::{Value, json};

use crate::core::providers::anthropic::error::anthropic_api_error;
use crate::core::providers::anthropic::models::{ModelFeature, get_anthropic_registry};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;

use super::AnthropicClient;

impl AnthropicClient {
    pub(super) fn transform_chat_request(
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
        if model_spec.is_none() && !self.config.allows_unknown_model(&request.model) {
            return Err(anthropic_api_error(
                400,
                format!("Unsupported model: {}", request.model),
            ));
        }
        if model_spec.is_none()
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
        if model_spec.is_none() && Self::has_unsupported_unknown_model_content(request) {
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

        // Add tool support
        if let Some(tools) = &request.tools
            && !tools.is_empty()
        {
            let Some(model_spec) = model_spec else {
                return Err(ProviderError::not_supported(
                    "anthropic",
                    format!(
                        "Unknown model {} cannot declare tool calling support",
                        request.model
                    ),
                ));
            };
            if !model_spec.features.contains(&ModelFeature::ToolCalling) {
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

        // Add thinking configuration
        if let Some(thinking) = &request.thinking
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
}
