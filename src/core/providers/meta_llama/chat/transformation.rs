//! Request and response transformation for Meta Llama chat API
//!
//! This module handles the transformation of requests and responses between
//! the standard format and Llama's OpenAI-compatible API format.
//!
//! Llama API specifics:
//! - Supports function calling and tools
//! - Only json_schema is supported for response_format
//! - OpenAI-compatible endpoint at <https://api.llama.com/compat/v1>

use serde_json::{Value, json};
use std::collections::HashMap;
use tracing::{debug, warn};

// New type system imports - base_llm removed
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    message::MessageContent,
    message::MessageRole,
    responses::{ChatChoice, ChatResponse, FinishReason, Usage},
    tools::FunctionCall,
    tools::ToolCall as RequestToolCall,
};

/// Llama chat transformation handler
#[derive(Debug, Clone)]
pub struct LlamaChatTransformation {
    /// Supported OpenAI parameters for Llama API
    supported_params: Vec<String>,
}

impl Default for LlamaChatTransformation {
    fn default() -> Self {
        Self::new()
    }
}

impl LlamaChatTransformation {
    /// Create a new transformation handler
    pub fn new() -> Self {
        Self {
            supported_params: vec![
                "messages".to_string(),
                "model".to_string(),
                "max_tokens".to_string(),
                "temperature".to_string(),
                "top_p".to_string(),
                "n".to_string(),
                "stream".to_string(),
                "stop".to_string(),
                "presence_penalty".to_string(),
                "frequency_penalty".to_string(),
                "logit_bias".to_string(),
                "user".to_string(),
                "seed".to_string(),
                "response_format".to_string(),
                "tools".to_string(),
                "tool_choice".to_string(),
                "functions".to_string(),
                "function_call".to_string(),
            ],
        }
    }

    /// Get supported OpenAI parameters
    pub fn get_supported_params(&self) -> Vec<String> {
        self.supported_params.clone()
    }

    /// Transform a chat completion request to Llama format
    pub fn transform_request(&self, request: ChatRequest) -> Result<Value, ProviderError> {
        let mut transformed = json!({
            "model": request.model,
            "messages": self.transform_messages(&request.messages),
        });

        // Add optional parameters
        if let Some(temp) = request.temperature {
            transformed["temperature"] = json!(temp);
        }

        if let Some(top_p) = request.top_p {
            transformed["top_p"] = json!(top_p);
        }

        if let Some(max_tokens) = request.max_tokens {
            transformed["max_tokens"] = json!(max_tokens);
        }

        if let Some(n) = request.n {
            transformed["n"] = json!(n);
        }

        if request.stream {
            transformed["stream"] = json!(true);
        }

        if let Some(stop) = request.stop {
            transformed["stop"] = json!(stop);
        }

        if let Some(presence) = request.presence_penalty {
            transformed["presence_penalty"] = json!(presence);
        }

        if let Some(frequency) = request.frequency_penalty {
            transformed["frequency_penalty"] = json!(frequency);
        }

        if let Some(user) = request.user {
            transformed["user"] = json!(user);
        }

        // Handle response format - only json_schema is supported
        if let Some(format) = request.response_format
            && let Ok(format_val) = serde_json::to_value(format)
            && let Some(format_type) = format_val.get("type").and_then(|t| t.as_str())
        {
            if format_type == "json_schema" {
                transformed["response_format"] = format_val;
            } else {
                warn!(
                    "Llama API only supports json_schema for response_format, ignoring: {}",
                    format_type
                );
            }
        }

        // Handle tools and function calling
        if let Some(tools) = request.tools {
            transformed["tools"] = json!(tools);
        }

        if let Some(tool_choice) = request.tool_choice {
            transformed["tool_choice"] = json!(tool_choice);
        }

        debug!(
            "Transformed Llama request: {}",
            serde_json::to_string_pretty(&transformed).unwrap_or_default()
        );

        Ok(transformed)
    }

    /// Transform messages to Llama format
    fn transform_messages(&self, messages: &[ChatMessage]) -> Value {
        let transformed: Vec<Value> = messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Function => "function",
                    MessageRole::Tool => "tool",
                };

                let content = match &msg.content {
                    Some(MessageContent::Text(text)) => json!(text),
                    Some(MessageContent::Parts(parts)) => json!(parts),
                    None => json!(null),
                };

                let mut message = json!({
                    "role": role,
                    "content": content,
                });

                // Add tool calls if present
                if let Some(tool_calls) = &msg.tool_calls {
                    message["tool_calls"] = json!(tool_calls);
                }

                // Add tool call ID if present
                if let Some(tool_call_id) = &msg.tool_call_id {
                    message["tool_call_id"] = json!(tool_call_id);
                }

                message
            })
            .collect();

        json!(transformed)
    }

    /// Transform a Llama response to standard format
    pub fn transform_response(&self, response: Value) -> Result<ChatResponse, ProviderError> {
        // Parse the response
        let id = response
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("llama-response")
            .to_string();

        let object = response
            .get("object")
            .and_then(|v| v.as_str())
            .unwrap_or("chat.completion")
            .to_string();

        let created = response
            .get("created")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp());

        let model = response
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("llama")
            .to_string();

        // Transform choices
        let choices = self.transform_choices(response.get("choices"))?;

        // Transform usage
        let usage = self.transform_usage(response.get("usage"));

        // Get system fingerprint if present
        let system_fingerprint = response
            .get("system_fingerprint")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(ChatResponse {
            id,
            object,
            created,
            model,
            choices,
            usage,
            system_fingerprint,
        })
    }

    /// Transform choices from response
    fn transform_choices(
        &self,
        choices_value: Option<&Value>,
    ) -> Result<Vec<ChatChoice>, ProviderError> {
        let choices_array = choices_value.and_then(|v| v.as_array()).ok_or_else(|| {
            ProviderError::serialization("meta", "Missing or invalid choices in response")
        })?;

        let mut choices = Vec::new();

        for choice_value in choices_array {
            let index = choice_value
                .get("index")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;

            let message = self.transform_message(choice_value.get("message"))?;

            let finish_reason = choice_value
                .get("finish_reason")
                .and_then(|v| v.as_str())
                .map(|r| match r {
                    "stop" => FinishReason::Stop,
                    "length" => FinishReason::Length,
                    "function_call" => FinishReason::FunctionCall,
                    "tool_calls" => FinishReason::ToolCalls,
                    _ => FinishReason::Stop,
                });

            choices.push(ChatChoice {
                index,
                message,
                finish_reason,
                logprobs: None,
            });
        }

        Ok(choices)
    }

    /// Transform a message from response
    fn transform_message(
        &self,
        message_value: Option<&Value>,
    ) -> Result<ChatMessage, ProviderError> {
        let message_obj = message_value
            .ok_or_else(|| ProviderError::serialization("meta", "Missing message in choice"))?;

        let role = match message_obj
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("assistant")
        {
            "system" => MessageRole::System,
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "function" => MessageRole::Function,
            "tool" => MessageRole::Tool,
            _ => MessageRole::Assistant,
        };

        let content = message_obj
            .get("content")
            .and_then(|v| v.as_str())
            .map(|s| MessageContent::Text(s.to_string()));

        let tool_calls = message_obj
            .get("tool_calls")
            .and_then(|v| serde_json::from_value::<Vec<RequestToolCall>>(v.clone()).ok());

        let name = message_obj
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let tool_call_id = message_obj
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let function_call = message_obj
            .get("function_call")
            .and_then(|v| serde_json::from_value::<FunctionCall>(v.clone()).ok());

        Ok(ChatMessage {
            role,
            content,
            thinking: None,
            name,
            tool_calls,
            tool_call_id,
            function_call,
        })
    }

    /// Transform usage from response
    fn transform_usage(&self, usage_value: Option<&Value>) -> Option<Usage> {
        usage_value.map(|usage| {
            let prompt_tokens = usage
                .get("prompt_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;

            let completion_tokens = usage
                .get("completion_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;

            let total_tokens = usage
                .get("total_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(prompt_tokens as i64 + completion_tokens as i64)
                as u32;

            Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }
        })
    }

    /// Map OpenAI parameters to Llama-specific ones
    pub fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> HashMap<String, Value> {
        let mut mapped = HashMap::new();

        for (key, value) in params {
            if self.supported_params.contains(&key) {
                // Special handling for response_format
                if key == "response_format" {
                    if let Some(format_type) = value.get("type").and_then(|t| t.as_str()) {
                        if format_type == "json_schema" {
                            mapped.insert(key, value);
                        } else {
                            warn!(
                                "Llama only supports json_schema for response_format, skipping: {}",
                                format_type
                            );
                        }
                    }
                } else {
                    mapped.insert(key, value);
                }
            } else {
                debug!("Parameter '{}' not supported by Llama API, skipping", key);
            }
        }

        mapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transformation_creation() {
        let transformation = LlamaChatTransformation::new();
        assert!(!transformation.supported_params.is_empty());
    }

    #[test]
    fn test_response_format_filtering() {
        let transformation = LlamaChatTransformation::new();

        // Test with json_schema (should be included)
        let mut params = HashMap::new();
        params.insert(
            "response_format".to_string(),
            json!({"type": "json_schema"}),
        );

        let mapped = transformation.map_openai_params(params, "llama");
        assert!(mapped.contains_key("response_format"));

        // Test with json_object (should be filtered out)
        let mut params = HashMap::new();
        params.insert(
            "response_format".to_string(),
            json!({"type": "json_object"}),
        );

        let mapped = transformation.map_openai_params(params, "llama");
        assert!(!mapped.contains_key("response_format"));
    }
}
