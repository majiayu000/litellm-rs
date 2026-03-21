//! Request and response transformation for Mistral chat API

use serde_json::{Value, json};
use tracing::debug;

// New type system imports - base_llm removed
use crate::core::providers::mistral::MistralError;
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

/// Mistral chat transformation handler
#[derive(Debug, Clone)]
pub struct MistralChatTransformation {
    /// Supported OpenAI parameters for Mistral API
    supported_params: Vec<String>,
}

impl Default for MistralChatTransformation {
    fn default() -> Self {
        Self::new()
    }
}

impl MistralChatTransformation {
    /// Create a new transformation handler
    pub fn new() -> Self {
        Self {
            supported_params: vec![
                "messages".to_string(),
                "model".to_string(),
                "max_tokens".to_string(),
                "temperature".to_string(),
                "top_p".to_string(),
                "stream".to_string(),
                "stop".to_string(),
                "random_seed".to_string(),
                "response_format".to_string(),
                "tools".to_string(),
                "tool_choice".to_string(),
                "frequency_penalty".to_string(),
                "presence_penalty".to_string(),
                "n".to_string(),
                "parallel_tool_calls".to_string(),
                "guardrails".to_string(), // Mistral-specific (replaces safe_prompt)
                "safe_prompt".to_string(), // Mistral-specific (legacy, prefer guardrails)
            ],
        }
    }

    /// Get supported OpenAI parameters
    pub fn get_supported_params(&self) -> Vec<String> {
        self.supported_params.clone()
    }

    /// Transform a chat completion request to Mistral format
    pub fn transform_request(&self, request: ChatRequest) -> Result<Value, MistralError> {
        let mut transformed = json!({
            "model": self.normalize_model_name(&request.model),
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

        if request.stream {
            transformed["stream"] = json!(true);
        }

        if let Some(stop) = request.stop {
            transformed["stop"] = json!(stop);
        }

        // Mistral uses random_seed instead of seed
        if let Some(seed) = request.seed {
            transformed["random_seed"] = json!(seed);
        }

        // Handle response format
        if let Some(format) = request.response_format
            && let Ok(format_val) = serde_json::to_value(format)
        {
            transformed["response_format"] = format_val;
        }

        if let Some(freq) = request.frequency_penalty {
            transformed["frequency_penalty"] = json!(freq);
        }

        if let Some(pres) = request.presence_penalty {
            transformed["presence_penalty"] = json!(pres);
        }

        if let Some(n) = request.n {
            transformed["n"] = json!(n);
        }

        // Handle tools and function calling
        if let Some(tools) = request.tools {
            transformed["tools"] = json!(tools);
        }

        if let Some(tool_choice) = request.tool_choice {
            transformed["tool_choice"] = json!(tool_choice);
        }

        if let Some(parallel) = request.parallel_tool_calls {
            transformed["parallel_tool_calls"] = json!(parallel);
        }

        // Mistral-specific: guardrails (new name) supersedes safe_prompt (legacy).
        // Only include if explicitly provided by the caller; never hardcode a default.
        if let Some(guardrails) = request.extra_params.get("guardrails") {
            transformed["guardrails"] = guardrails.clone();
        } else if let Some(safe_prompt) = request.extra_params.get("safe_prompt") {
            transformed["safe_prompt"] = safe_prompt.clone();
        }

        debug!(
            "Transformed Mistral request: {}",
            serde_json::to_string_pretty(&transformed).unwrap_or_default()
        );

        Ok(transformed)
    }

    /// Normalize model name for Mistral API
    fn normalize_model_name(&self, model: &str) -> String {
        // Remove common prefixes from model name
        model.replace("mistral/", "").replace("mistralai/", "")
    }

    /// Transform messages to Mistral format
    fn transform_messages(&self, messages: &[ChatMessage]) -> Value {
        let transformed: Vec<Value> = messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::System | MessageRole::Developer => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Function => "tool", // Mistral uses "tool" for function messages
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

    /// Transform a Mistral response to standard format
    pub fn transform_response(&self, response: Value) -> Result<ChatResponse, MistralError> {
        // Parse the response
        let id = response
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("mistral-response")
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
            .unwrap_or("mistral")
            .to_string();

        // Transform choices
        let choices = self.transform_choices(response.get("choices"))?;

        // Transform usage
        let usage = self.transform_usage(response.get("usage"));

        Ok(ChatResponse {
            id,
            object,
            created,
            model,
            choices,
            usage,
            system_fingerprint: None,
        })
    }

    /// Transform choices from response
    fn transform_choices(
        &self,
        choices_value: Option<&Value>,
    ) -> Result<Vec<ChatChoice>, MistralError> {
        let choices_array = choices_value.and_then(|v| v.as_array()).ok_or_else(|| {
            ProviderError::response_parsing(
                "mistral",
                "Missing or invalid choices in response".to_string(),
            )
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
    ) -> Result<ChatMessage, MistralError> {
        let message_obj = message_value.ok_or_else(|| {
            ProviderError::response_parsing("mistral", "Missing message in choice".to_string())
        })?;

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

        let name = message_obj
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let function_call = message_obj
            .get("function_call")
            .and_then(|v| serde_json::from_value::<FunctionCall>(v.clone()).ok());

        let tool_calls = message_obj
            .get("tool_calls")
            .and_then(|v| serde_json::from_value::<Vec<RequestToolCall>>(v.clone()).ok());

        let tool_call_id = message_obj
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(ChatMessage {
            role,
            content,
            name,
            function_call,
            tool_calls,
            tool_call_id,
            thinking: None,
        })
    }

    /// Transform usage from response
    fn transform_usage(&self, usage_value: Option<&Value>) -> Option<Usage> {
        usage_value.map(|usage| {
            let prompt_tokens = usage
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;

            let completion_tokens = usage
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;

            let total_tokens = usage
                .get("total_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or((prompt_tokens + completion_tokens) as u64)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mistral_chat_transformation_new() {
        let transformation = MistralChatTransformation::new();
        let params = transformation.get_supported_params();
        assert!(params.contains(&"messages".to_string()));
        assert!(params.contains(&"model".to_string()));
        assert!(params.contains(&"temperature".to_string()));
        assert!(params.contains(&"safe_prompt".to_string()));
    }

    #[test]
    fn test_mistral_chat_transformation_default() {
        let transformation = MistralChatTransformation::default();
        assert!(!transformation.get_supported_params().is_empty());
    }

    #[test]
    fn test_normalize_model_name() {
        let transformation = MistralChatTransformation::new();
        assert_eq!(
            transformation.normalize_model_name("mistral/mistral-large"),
            "mistral-large"
        );
        assert_eq!(
            transformation.normalize_model_name("mistralai/mistral-medium"),
            "mistral-medium"
        );
        assert_eq!(
            transformation.normalize_model_name("mistral-small"),
            "mistral-small"
        );
    }

    #[test]
    fn test_transform_request_basic() {
        let transformation = MistralChatTransformation::new();
        let request = ChatRequest {
            model: "mistral-large".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                thinking: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            ..Default::default()
        };

        let result = transformation.transform_request(request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["model"], "mistral-large");
        assert!(value["messages"].is_array());
        // safe_prompt must NOT be injected when caller omits it
        assert!(value.get("safe_prompt").is_none());
        assert!(value.get("guardrails").is_none());
    }

    #[test]
    fn test_transform_request_with_options() {
        let transformation = MistralChatTransformation::new();
        let request = ChatRequest {
            model: "mistral-large".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                thinking: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: Some(0.5),
            max_tokens: Some(100),
            top_p: Some(0.5),
            seed: Some(42),
            stream: true,
            ..Default::default()
        };

        let result = transformation.transform_request(request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["temperature"], 0.5);
        assert_eq!(value["max_tokens"], 100);
        assert_eq!(value["top_p"], 0.5);
        assert_eq!(value["random_seed"], 42);
        assert_eq!(value["stream"], true);
    }

    #[test]
    fn test_transform_messages_roles() {
        let transformation = MistralChatTransformation::new();
        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: Some(MessageContent::Text("System".to_string())),
                thinking: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("User".to_string())),
                thinking: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: Some(MessageContent::Text("Assistant".to_string())),
                thinking: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::Tool,
                content: Some(MessageContent::Text("Tool".to_string())),
                thinking: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: Some("call_123".to_string()),
            },
        ];

        let result = transformation.transform_messages(&messages);
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0]["role"], "system");
        assert_eq!(arr[1]["role"], "user");
        assert_eq!(arr[2]["role"], "assistant");
        assert_eq!(arr[3]["role"], "tool");
        assert_eq!(arr[3]["tool_call_id"], "call_123");
    }

    #[test]
    fn test_transform_response() {
        let transformation = MistralChatTransformation::new();
        let response = json!({
            "id": "cmpl-123",
            "object": "chat.completion",
            "created": 1699000000,
            "model": "mistral-large",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });

        let result = transformation.transform_response(response);
        assert!(result.is_ok());
        let chat_response = result.unwrap();
        assert_eq!(chat_response.id, "cmpl-123");
        assert_eq!(chat_response.model, "mistral-large");
        assert_eq!(chat_response.choices.len(), 1);
        assert_eq!(
            chat_response.choices[0].finish_reason,
            Some(FinishReason::Stop)
        );
        assert!(chat_response.usage.is_some());
        let usage = chat_response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn test_transform_response_finish_reasons() {
        let transformation = MistralChatTransformation::new();

        let test_cases = vec![
            ("stop", FinishReason::Stop),
            ("length", FinishReason::Length),
            ("function_call", FinishReason::FunctionCall),
            ("tool_calls", FinishReason::ToolCalls),
            ("unknown", FinishReason::Stop),
        ];

        for (reason_str, expected) in test_cases {
            let response = json!({
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "Hi" },
                    "finish_reason": reason_str
                }]
            });

            let result = transformation.transform_response(response).unwrap();
            assert_eq!(result.choices[0].finish_reason, Some(expected));
        }
    }

    #[test]
    fn test_transform_response_missing_choices() {
        let transformation = MistralChatTransformation::new();
        let response = json!({
            "id": "cmpl-123",
            "model": "mistral-large"
        });

        let result = transformation.transform_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_usage() {
        let transformation = MistralChatTransformation::new();
        let usage_value = json!({
            "prompt_tokens": 100,
            "completion_tokens": 50,
            "total_tokens": 150
        });

        let usage = transformation.transform_usage(Some(&usage_value));
        assert!(usage.is_some());
        let u = usage.unwrap();
        assert_eq!(u.prompt_tokens, 100);
        assert_eq!(u.completion_tokens, 50);
        assert_eq!(u.total_tokens, 150);
    }

    #[test]
    fn test_transform_usage_none() {
        let transformation = MistralChatTransformation::new();
        let usage = transformation.transform_usage(None);
        assert!(usage.is_none());
    }

    #[test]
    fn test_transform_message_with_tool_calls() {
        let transformation = MistralChatTransformation::new();
        let message_value = json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "call_123",
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "arguments": "{\"location\": \"NYC\"}"
                }
            }]
        });

        let result = transformation.transform_message(Some(&message_value));
        assert!(result.is_ok());
        let msg = result.unwrap();
        assert_eq!(msg.role, MessageRole::Assistant);
        assert!(msg.tool_calls.is_some());
    }

    #[test]
    fn test_get_supported_params() {
        let transformation = MistralChatTransformation::new();
        let params = transformation.get_supported_params();

        assert!(params.contains(&"messages".to_string()));
        assert!(params.contains(&"model".to_string()));
        assert!(params.contains(&"max_tokens".to_string()));
        assert!(params.contains(&"temperature".to_string()));
        assert!(params.contains(&"top_p".to_string()));
        assert!(params.contains(&"stream".to_string()));
        assert!(params.contains(&"stop".to_string()));
        assert!(params.contains(&"random_seed".to_string()));
        assert!(params.contains(&"response_format".to_string()));
        assert!(params.contains(&"tools".to_string()));
        assert!(params.contains(&"tool_choice".to_string()));
        assert!(params.contains(&"frequency_penalty".to_string()));
        assert!(params.contains(&"presence_penalty".to_string()));
        assert!(params.contains(&"n".to_string()));
        assert!(params.contains(&"parallel_tool_calls".to_string()));
        assert!(params.contains(&"guardrails".to_string()));
        assert!(params.contains(&"safe_prompt".to_string()));
    }

    #[test]
    fn test_transform_request_new_params() {
        use std::collections::HashMap;
        let transformation = MistralChatTransformation::new();
        let mut extra_params = HashMap::new();
        extra_params.insert("guardrails".to_string(), serde_json::json!(true));
        let request = ChatRequest {
            model: "mistral-large".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hi".to_string())),
                thinking: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            frequency_penalty: Some(0.5),
            presence_penalty: Some(0.5),
            n: Some(2),
            parallel_tool_calls: Some(true),
            extra_params,
            ..Default::default()
        };

        let Ok(value) = transformation.transform_request(request) else {
            panic!("transform_request failed");
        };
        assert_eq!(value["frequency_penalty"], 0.5);
        assert_eq!(value["presence_penalty"], 0.5);
        assert_eq!(value["n"], 2);
        assert_eq!(value["parallel_tool_calls"], true);
        assert_eq!(value["guardrails"], true);
        assert!(value.get("safe_prompt").is_none());
    }

    #[test]
    fn test_safe_prompt_legacy_passthrough() {
        use std::collections::HashMap;
        let transformation = MistralChatTransformation::new();
        let mut extra_params = HashMap::new();
        extra_params.insert("safe_prompt".to_string(), serde_json::json!(true));
        let request = ChatRequest {
            model: "mistral-large".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hi".to_string())),
                thinking: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            extra_params,
            ..Default::default()
        };

        let Ok(value) = transformation.transform_request(request) else {
            panic!("transform_request failed");
        };
        assert_eq!(value["safe_prompt"], true);
        assert!(value.get("guardrails").is_none());
    }
}
