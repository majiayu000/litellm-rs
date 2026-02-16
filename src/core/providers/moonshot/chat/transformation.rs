//! Request and response transformation for Moonshot chat API

use serde_json::{Value, json};
use tracing::{debug, warn};

use crate::core::providers::moonshot::MoonshotError;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    message::MessageContent,
    message::MessageRole,
    responses::{ChatChoice, ChatResponse, FinishReason, Usage},
    tools::FunctionCall,
    tools::ToolCall,
};

/// Moonshot chat transformation handler
#[derive(Debug, Clone)]
pub struct MoonshotChatTransformation {
    /// Supported parameters for Moonshot API
    supported_params: Vec<String>,
}

impl Default for MoonshotChatTransformation {
    fn default() -> Self {
        Self::new()
    }
}

impl MoonshotChatTransformation {
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
                "user".to_string(),
                "tools".to_string(),
                "tool_choice".to_string(),
            ],
        }
    }

    /// Get supported parameters
    pub fn get_supported_params(&self) -> Vec<String> {
        self.supported_params.clone()
    }

    /// Transform a chat completion request to Moonshot format
    pub fn transform_request(&self, request: ChatRequest) -> Result<Value, MoonshotError> {
        let mut transformed = json!({
            "model": self.normalize_model_name(&request.model),
            "messages": self.transform_messages(&request.messages)?,
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

        // Handle tools and function calling
        if let Some(tools) = request.tools {
            transformed["tools"] = serde_json::to_value(tools).unwrap_or(json!([]));
        }

        if let Some(tool_choice) = request.tool_choice {
            transformed["tool_choice"] = serde_json::to_value(tool_choice).unwrap_or(json!("auto"));
        }

        debug!(
            "Transformed Moonshot request: {}",
            serde_json::to_string_pretty(&transformed).unwrap_or_default()
        );

        Ok(transformed)
    }

    /// Normalize model name for Moonshot API
    fn normalize_model_name(&self, model: &str) -> String {
        // Remove common prefixes from model name
        model.replace("moonshot/", "").replace("moonshotai/", "")
    }

    /// Transform messages to Moonshot format
    fn transform_messages(&self, messages: &[ChatMessage]) -> Result<Value, MoonshotError> {
        let transformed: Vec<Value> = messages
            .iter()
            .map(|msg| {
                let mut message = json!({
                    "role": self.transform_role(&msg.role),
                });

                // Add content
                if let Some(content) = &msg.content {
                    match content {
                        MessageContent::Text(text) => {
                            message["content"] = json!(text);
                        }
                        MessageContent::Parts(parts) => {
                            // Moonshot doesn't support multi-part messages yet
                            // Extract text parts and combine them
                            let text_parts: Vec<String> = parts
                                .iter()
                                .filter_map(|part| {
                                    if let crate::core::types::content::ContentPart::Text { text } =
                                        part
                                    {
                                        Some(text.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            if !text_parts.is_empty() {
                                message["content"] = json!(text_parts.join("\n"));
                            } else {
                                warn!("No text content found in multi-part message");
                                message["content"] = json!("");
                            }
                        }
                    }
                }

                // Add name if present (for function messages)
                if let Some(name) = &msg.name {
                    message["name"] = json!(name);
                }

                // Add function call if present
                if let Some(function_call) = &msg.function_call {
                    message["function_call"] =
                        serde_json::to_value(function_call).unwrap_or(json!(null));
                }

                // Add tool calls if present
                if let Some(tool_calls) = &msg.tool_calls {
                    message["tool_calls"] = serde_json::to_value(tool_calls).unwrap_or(json!([]));
                }

                message
            })
            .collect();

        Ok(json!(transformed))
    }

    /// Transform role to string format
    fn transform_role(&self, role: &MessageRole) -> String {
        match role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Function => "function",
            MessageRole::Tool => "tool",
        }
        .to_string()
    }

    /// Transform a Moonshot response to standard format
    pub fn transform_response(&self, response: Value) -> Result<ChatResponse, MoonshotError> {
        // Parse the response
        let id = response
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("moonshot-response")
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
            .unwrap_or("moonshot")
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
    ) -> Result<Vec<ChatChoice>, MoonshotError> {
        let choices_array = choices_value.and_then(|v| v.as_array()).ok_or_else(|| {
            ProviderError::response_parsing("moonshot", "Missing or invalid choices in response")
        })?;

        let mut choices = Vec::new();

        for choice_value in choices_array {
            let index = choice_value
                .get("index")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;

            let message = self.transform_message(choice_value.get("message"))?;

            let finish_reason = choice_value
                .get("finish_reason")
                .and_then(|v| v.as_str())
                .map(|s| match s {
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
    ) -> Result<ChatMessage, MoonshotError> {
        let message_obj = message_value.ok_or_else(|| {
            ProviderError::response_parsing("moonshot", "Missing message in choice")
        })?;

        let role = message_obj
            .get("role")
            .and_then(|v| v.as_str())
            .map(|r| self.parse_role(r))
            .unwrap_or(MessageRole::Assistant);

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
            .and_then(|v| serde_json::from_value::<Vec<ToolCall>>(v.clone()).ok());

        let tool_call_id = message_obj
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(ChatMessage {
            role,
            content,
            thinking: None,
            name,
            function_call,
            tool_calls,
            tool_call_id,
        })
    }

    /// Parse role from string
    fn parse_role(&self, role: &str) -> MessageRole {
        match role.to_lowercase().as_str() {
            "system" => MessageRole::System,
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "function" => MessageRole::Function,
            "tool" => MessageRole::Tool,
            _ => MessageRole::Assistant,
        }
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
    fn test_moonshot_transformation_new() {
        let transformation = MoonshotChatTransformation::new();
        let params = transformation.get_supported_params();
        assert!(params.contains(&"messages".to_string()));
        assert!(params.contains(&"model".to_string()));
        assert!(params.contains(&"temperature".to_string()));
        assert!(params.contains(&"tools".to_string()));
    }

    #[test]
    fn test_moonshot_transformation_default() {
        let transformation = MoonshotChatTransformation::default();
        assert!(!transformation.get_supported_params().is_empty());
    }

    #[test]
    fn test_normalize_model_name() {
        let transformation = MoonshotChatTransformation::new();
        assert_eq!(
            transformation.normalize_model_name("moonshot/moonshot-v1-8k"),
            "moonshot-v1-8k"
        );
        assert_eq!(
            transformation.normalize_model_name("moonshotai/moonshot-v1-32k"),
            "moonshot-v1-32k"
        );
        assert_eq!(
            transformation.normalize_model_name("moonshot-v1-128k"),
            "moonshot-v1-128k"
        );
    }

    #[test]
    fn test_transform_role() {
        let transformation = MoonshotChatTransformation::new();
        assert_eq!(
            transformation.transform_role(&MessageRole::System),
            "system"
        );
        assert_eq!(transformation.transform_role(&MessageRole::User), "user");
        assert_eq!(
            transformation.transform_role(&MessageRole::Assistant),
            "assistant"
        );
        assert_eq!(
            transformation.transform_role(&MessageRole::Function),
            "function"
        );
        assert_eq!(transformation.transform_role(&MessageRole::Tool), "tool");
    }

    #[test]
    fn test_parse_role() {
        let transformation = MoonshotChatTransformation::new();
        assert_eq!(transformation.parse_role("system"), MessageRole::System);
        assert_eq!(transformation.parse_role("user"), MessageRole::User);
        assert_eq!(
            transformation.parse_role("assistant"),
            MessageRole::Assistant
        );
        assert_eq!(transformation.parse_role("function"), MessageRole::Function);
        assert_eq!(transformation.parse_role("tool"), MessageRole::Tool);
        assert_eq!(transformation.parse_role("unknown"), MessageRole::Assistant);
        assert_eq!(transformation.parse_role("SYSTEM"), MessageRole::System);
    }

    #[test]
    fn test_transform_request_basic() {
        let transformation = MoonshotChatTransformation::new();
        let request = ChatRequest {
            model: "moonshot-v1-8k".to_string(),
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
        assert_eq!(value["model"], "moonshot-v1-8k");
        assert!(value["messages"].is_array());
    }

    #[test]
    fn test_transform_request_with_options() {
        let transformation = MoonshotChatTransformation::new();
        let request = ChatRequest {
            model: "moonshot-v1-8k".to_string(),
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
            stream: true,
            ..Default::default()
        };

        let result = transformation.transform_request(request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["temperature"], 0.5);
        assert_eq!(value["max_tokens"], 100);
        assert_eq!(value["top_p"], 0.5);
        assert_eq!(value["stream"], true);
    }

    #[test]
    fn test_transform_usage() {
        let transformation = MoonshotChatTransformation::new();
        let usage_value = json!({
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        });

        let usage = transformation.transform_usage(Some(&usage_value));
        assert!(usage.is_some());
        let u = usage.unwrap();
        assert_eq!(u.prompt_tokens, 10);
        assert_eq!(u.completion_tokens, 20);
        assert_eq!(u.total_tokens, 30);
    }

    #[test]
    fn test_transform_usage_none() {
        let transformation = MoonshotChatTransformation::new();
        let usage = transformation.transform_usage(None);
        assert!(usage.is_none());
    }

    #[test]
    fn test_transform_response() {
        let transformation = MoonshotChatTransformation::new();
        let response = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "moonshot-v1-8k",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello there!"
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
        assert_eq!(chat_response.id, "chatcmpl-123");
        assert_eq!(chat_response.model, "moonshot-v1-8k");
        assert_eq!(chat_response.choices.len(), 1);
        assert_eq!(
            chat_response.choices[0].message.role,
            MessageRole::Assistant
        );
    }

    #[test]
    fn test_transform_choices_missing() {
        let transformation = MoonshotChatTransformation::new();
        let result = transformation.transform_choices(None);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_finish_reasons() {
        let transformation = MoonshotChatTransformation::new();
        let choices = json!([
            {"index": 0, "message": {"role": "assistant", "content": "a"}, "finish_reason": "stop"},
            {"index": 1, "message": {"role": "assistant", "content": "b"}, "finish_reason": "length"},
            {"index": 2, "message": {"role": "assistant", "content": "c"}, "finish_reason": "function_call"},
            {"index": 3, "message": {"role": "assistant", "content": "d"}, "finish_reason": "tool_calls"}
        ]);

        let result = transformation.transform_choices(Some(&choices));
        assert!(result.is_ok());
        let choices = result.unwrap();
        assert_eq!(choices[0].finish_reason, Some(FinishReason::Stop));
        assert_eq!(choices[1].finish_reason, Some(FinishReason::Length));
        assert_eq!(choices[2].finish_reason, Some(FinishReason::FunctionCall));
        assert_eq!(choices[3].finish_reason, Some(FinishReason::ToolCalls));
    }
}
