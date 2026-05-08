//! Type definitions for streaming responses

use crate::core::models::openai::Usage;
use crate::core::types::message::MessageRole;
use crate::core::types::thinking::ThinkingDelta;
use bytes::Bytes;

/// Simple Event structure for SSE compatibility
#[derive(Debug, Clone, Default)]
pub struct Event {
    /// Event type
    pub event: Option<String>,
    /// Event data
    pub data: String,
}

impl Event {
    /// Create a new empty event
    pub fn new() -> Self {
        Self {
            event: None,
            data: String::new(),
        }
    }

    /// Set the event type
    pub fn event(mut self, event: &str) -> Self {
        self.event = Some(event.to_string());
        self
    }

    /// Set the event data
    pub fn data(mut self, data: &str) -> Self {
        self.data = data.to_string();
        self
    }

    /// Convert event to bytes for SSE transmission
    pub fn to_bytes(&self) -> Bytes {
        let mut result = String::new();
        if let Some(event) = &self.event {
            result.push_str(&format!("event: {}\n", event));
        }
        result.push_str(&format!("data: {}\n\n", self.data));
        Bytes::from(result)
    }
}

/// Streaming response chunk for chat completions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatCompletionChunk {
    /// Unique identifier for the completion
    pub id: String,
    /// Object type (always "chat.completion.chunk")
    pub object: String,
    /// Unix timestamp of creation
    pub created: u64,
    /// Model used for completion
    pub model: String,
    /// System fingerprint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    /// Array of completion choices
    pub choices: Vec<ChatCompletionChunkChoice>,
    /// Usage statistics (only in final chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Choice in a streaming chat completion chunk
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatCompletionChunkChoice {
    /// Index of the choice
    pub index: u32,
    /// Delta containing the incremental content
    pub delta: ChatCompletionDelta,
    /// Reason for finishing (only in final chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// Log probabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

/// Delta containing incremental content in streaming response
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ChatCompletionDelta {
    /// Role of the message (only in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<MessageRole>,
    /// Incremental content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Thinking/reasoning delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingDelta>,
    /// OpenAI-compatible reasoning content alias
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Tool calls (for function calling)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
    /// Tool call ID for tool-result style deltas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Refusal text for safety refusals
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// Function call delta for older OpenAI-compatible clients
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCallDelta>,
}

// ToolCallDelta and FunctionCallDelta are re-exported from the canonical
// types tree (core::types::responses::*). Duplicates eliminated as part
// of the parallel-type-tree collapse tracked in #519.
pub use crate::core::types::responses::{FunctionCallDelta, ToolCallDelta};

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Event Tests ====================

    #[test]
    fn test_event_new() {
        let event = Event::new();
        assert!(event.event.is_none());
        assert!(event.data.is_empty());
    }

    #[test]
    fn test_event_default() {
        let event = Event::default();
        assert!(event.event.is_none());
        assert!(event.data.is_empty());
    }

    #[test]
    fn test_event_with_data() {
        let event = Event::default().data("Hello, World!");
        assert!(event.event.is_none());
        assert_eq!(event.data, "Hello, World!");
    }

    #[test]
    fn test_event_with_event_type() {
        let event = Event::default().event("message");
        assert_eq!(event.event, Some("message".to_string()));
        assert!(event.data.is_empty());
    }

    #[test]
    fn test_event_with_both() {
        let event = Event::default().event("message").data("test data");
        assert_eq!(event.event, Some("message".to_string()));
        assert_eq!(event.data, "test data");
    }

    #[test]
    fn test_event_builder_chain() {
        let event = Event::new().event("error").data("{\"error\": \"test\"}");
        assert_eq!(event.event, Some("error".to_string()));
        assert_eq!(event.data, "{\"error\": \"test\"}");
    }

    #[test]
    fn test_event_to_bytes_data_only() {
        let event = Event::default().data("test");
        let bytes = event.to_bytes();
        let result = String::from_utf8_lossy(&bytes);
        assert_eq!(result, "data: test\n\n");
    }

    #[test]
    fn test_event_to_bytes_with_event_type() {
        let event = Event::default().event("message").data("test");
        let bytes = event.to_bytes();
        let result = String::from_utf8_lossy(&bytes);
        assert_eq!(result, "event: message\ndata: test\n\n");
    }

    #[test]
    fn test_event_to_bytes_empty() {
        let event = Event::default();
        let bytes = event.to_bytes();
        let result = String::from_utf8_lossy(&bytes);
        assert_eq!(result, "data: \n\n");
    }

    #[test]
    fn test_event_to_bytes_json_data() {
        let event = Event::default().data("{\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}");
        let bytes = event.to_bytes();
        let result = String::from_utf8_lossy(&bytes);
        assert!(result.contains("data: {\"choices\""));
        assert!(result.ends_with("\n\n"));
    }

    #[test]
    fn test_event_clone() {
        let event1 = Event::default().event("test").data("data");
        let event2 = event1.clone();
        assert_eq!(event1.event, event2.event);
        assert_eq!(event1.data, event2.data);
    }

    // ==================== ChatCompletionChunk Tests ====================

    #[test]
    fn test_chat_completion_chunk_serialize() {
        let chunk = ChatCompletionChunk {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            system_fingerprint: None,
            choices: vec![],
            usage: None,
        };

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("chatcmpl-123"));
        assert!(json.contains("chat.completion.chunk"));
        assert!(json.contains("gpt-4"));
    }

    #[test]
    fn test_chat_completion_chunk_deserialize() {
        let json = r#"{
            "id": "chatcmpl-abc",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4-turbo",
            "choices": []
        }"#;

        let chunk: ChatCompletionChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.id, "chatcmpl-abc");
        assert_eq!(chunk.object, "chat.completion.chunk");
        assert_eq!(chunk.created, 1700000000);
        assert_eq!(chunk.model, "gpt-4-turbo");
        assert!(chunk.choices.is_empty());
    }

    #[test]
    fn test_chat_completion_chunk_with_choices() {
        let chunk = ChatCompletionChunk {
            id: "test".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            system_fingerprint: Some("fp_123".to_string()),
            choices: vec![ChatCompletionChunkChoice {
                index: 0,
                delta: ChatCompletionDelta {
                    role: Some(MessageRole::Assistant),
                    content: Some("Hello".to_string()),
                    tool_calls: None,
                    ..Default::default()
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
        };

        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_chat_completion_chunk_with_usage() {
        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        };

        let chunk = ChatCompletionChunk {
            id: "test".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            system_fingerprint: None,
            choices: vec![],
            usage: Some(usage),
        };

        assert!(chunk.usage.is_some());
        assert_eq!(chunk.usage.unwrap().total_tokens, 30);
    }

    #[test]
    fn test_chat_completion_chunk_clone() {
        let chunk = ChatCompletionChunk {
            id: "test".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 123,
            model: "gpt-4".to_string(),
            system_fingerprint: None,
            choices: vec![],
            usage: None,
        };

        let cloned = chunk.clone();
        assert_eq!(chunk.id, cloned.id);
        assert_eq!(chunk.created, cloned.created);
    }

    // ==================== ChatCompletionChunkChoice Tests ====================

    #[test]
    fn test_chunk_choice_serialize() {
        let choice = ChatCompletionChunkChoice {
            index: 0,
            delta: ChatCompletionDelta {
                role: Some(MessageRole::Assistant),
                content: Some("Test".to_string()),
                tool_calls: None,
                ..Default::default()
            },
            finish_reason: None,
            logprobs: None,
        };

        let json = serde_json::to_string(&choice).unwrap();
        assert!(json.contains("\"index\":0"));
        assert!(json.contains("Test"));
    }

    #[test]
    fn test_chunk_choice_with_finish_reason() {
        let choice = ChatCompletionChunkChoice {
            index: 0,
            delta: ChatCompletionDelta {
                role: None,
                content: None,
                tool_calls: None,
                ..Default::default()
            },
            finish_reason: Some("stop".to_string()),
            logprobs: None,
        };

        assert_eq!(choice.finish_reason, Some("stop".to_string()));
    }

    #[test]
    fn test_chunk_choice_deserialize() {
        let json = r#"{
            "index": 1,
            "delta": {"content": "hello"},
            "finish_reason": null
        }"#;

        let choice: ChatCompletionChunkChoice = serde_json::from_str(json).unwrap();
        assert_eq!(choice.index, 1);
        assert_eq!(choice.delta.content, Some("hello".to_string()));
        assert!(choice.finish_reason.is_none());
    }

    // ==================== ChatCompletionDelta Tests ====================

    #[test]
    fn test_delta_empty() {
        let delta = ChatCompletionDelta {
            role: None,
            content: None,
            tool_calls: None,
            ..Default::default()
        };

        assert!(delta.role.is_none());
        assert!(delta.content.is_none());
        assert!(delta.tool_calls.is_none());
    }

    #[test]
    fn test_delta_with_role() {
        let delta = ChatCompletionDelta {
            role: Some(MessageRole::Assistant),
            content: None,
            tool_calls: None,
            ..Default::default()
        };

        assert_eq!(delta.role, Some(MessageRole::Assistant));
    }

    #[test]
    fn test_delta_with_content() {
        let delta = ChatCompletionDelta {
            role: None,
            content: Some("Hello world".to_string()),
            tool_calls: None,
            ..Default::default()
        };

        assert_eq!(delta.content, Some("Hello world".to_string()));
    }

    #[test]
    fn test_delta_serialize() {
        let delta = ChatCompletionDelta {
            role: Some(MessageRole::User),
            content: Some("test".to_string()),
            tool_calls: None,
            ..Default::default()
        };

        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains("\"content\":\"test\""));
    }

    #[test]
    fn test_delta_serialize_omits_null_optional_fields() {
        let delta = ChatCompletionDelta {
            content: Some("visible".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&delta).unwrap();
        assert_eq!(json, "{\"content\":\"visible\"}");
        assert!(!json.contains("null"));
    }

    #[test]
    fn test_delta_serialize_reasoning_and_tool_metadata() {
        let delta = ChatCompletionDelta {
            thinking: Some(ThinkingDelta::new("thinking...")),
            reasoning_content: Some("thinking...".to_string()),
            tool_call_id: Some("call_123".to_string()),
            refusal: Some("cannot comply".to_string()),
            function_call: Some(FunctionCallDelta {
                name: Some("legacy_fn".to_string()),
                arguments: Some("{}".to_string()),
            }),
            ..Default::default()
        };

        let json = serde_json::to_value(&delta).unwrap();
        assert_eq!(json["reasoning_content"], "thinking...");
        assert_eq!(json["thinking"]["content"], "thinking...");
        assert_eq!(json["tool_call_id"], "call_123");
        assert_eq!(json["refusal"], "cannot comply");
        assert_eq!(json["function_call"]["name"], "legacy_fn");
    }

    #[test]
    fn test_delta_deserialize() {
        let json = r#"{"role": "assistant", "content": "Hi"}"#;
        let delta: ChatCompletionDelta = serde_json::from_str(json).unwrap();
        assert_eq!(delta.role, Some(MessageRole::Assistant));
        assert_eq!(delta.content, Some("Hi".to_string()));
    }

    // ==================== ToolCallDelta Tests ====================

    #[test]
    fn test_tool_call_delta_empty() {
        let tool_call = ToolCallDelta {
            index: 0,
            id: None,
            tool_type: None,
            function: None,
        };

        assert_eq!(tool_call.index, 0);
        assert!(tool_call.id.is_none());
        assert!(tool_call.function.is_none());
    }

    #[test]
    fn test_tool_call_delta_first_chunk() {
        let tool_call = ToolCallDelta {
            index: 0,
            id: Some("call_123".to_string()),
            tool_type: Some("function".to_string()),
            function: Some(FunctionCallDelta {
                name: Some("get_weather".to_string()),
                arguments: None,
            }),
        };

        assert_eq!(tool_call.id, Some("call_123".to_string()));
        assert_eq!(tool_call.tool_type, Some("function".to_string()));
        assert!(tool_call.function.is_some());
    }

    #[test]
    fn test_tool_call_delta_serialize() {
        let tool_call = ToolCallDelta {
            index: 0,
            id: Some("call_abc".to_string()),
            tool_type: Some("function".to_string()),
            function: None,
        };

        let json = serde_json::to_string(&tool_call).unwrap();
        assert!(json.contains("call_abc"));
        // Check that type is renamed correctly
        assert!(json.contains("\"type\":"));
    }

    #[test]
    fn test_tool_call_delta_deserialize() {
        let json = r#"{
            "index": 0,
            "id": "call_xyz",
            "type": "function",
            "function": {"name": "test_func"}
        }"#;

        let tool_call: ToolCallDelta = serde_json::from_str(json).unwrap();
        assert_eq!(tool_call.index, 0);
        assert_eq!(tool_call.id, Some("call_xyz".to_string()));
        assert_eq!(tool_call.tool_type, Some("function".to_string()));
    }

    // ==================== FunctionCallDelta Tests ====================

    #[test]
    fn test_function_call_delta_empty() {
        let func = FunctionCallDelta {
            name: None,
            arguments: None,
        };

        assert!(func.name.is_none());
        assert!(func.arguments.is_none());
    }

    #[test]
    fn test_function_call_delta_with_name() {
        let func = FunctionCallDelta {
            name: Some("get_weather".to_string()),
            arguments: None,
        };

        assert_eq!(func.name, Some("get_weather".to_string()));
    }

    #[test]
    fn test_function_call_delta_with_arguments() {
        let func = FunctionCallDelta {
            name: None,
            arguments: Some("{\"location\":".to_string()),
        };

        assert_eq!(func.arguments, Some("{\"location\":".to_string()));
    }

    #[test]
    fn test_function_call_delta_full() {
        let func = FunctionCallDelta {
            name: Some("search".to_string()),
            arguments: Some("{\"query\": \"rust\"}".to_string()),
        };

        assert_eq!(func.name, Some("search".to_string()));
        assert_eq!(func.arguments, Some("{\"query\": \"rust\"}".to_string()));
    }

    #[test]
    fn test_function_call_delta_serialize() {
        let func = FunctionCallDelta {
            name: Some("test".to_string()),
            arguments: Some("{}".to_string()),
        };

        let json = serde_json::to_string(&func).unwrap();
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"arguments\":\"{}\""));
    }

    #[test]
    fn test_function_call_delta_deserialize() {
        let json = r#"{"name": "calculate", "arguments": "{\"x\": 1}"}"#;
        let func: FunctionCallDelta = serde_json::from_str(json).unwrap();
        assert_eq!(func.name, Some("calculate".to_string()));
        assert_eq!(func.arguments, Some("{\"x\": 1}".to_string()));
    }

    #[test]
    fn test_function_call_delta_clone() {
        let func = FunctionCallDelta {
            name: Some("test".to_string()),
            arguments: Some("args".to_string()),
        };

        let cloned = func.clone();
        assert_eq!(func.name, cloned.name);
        assert_eq!(func.arguments, cloned.arguments);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_full_streaming_chunk() {
        let chunk = ChatCompletionChunk {
            id: "chatcmpl-streaming".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1700000000,
            model: "gpt-4".to_string(),
            system_fingerprint: Some("fp_abc123".to_string()),
            choices: vec![ChatCompletionChunkChoice {
                index: 0,
                delta: ChatCompletionDelta {
                    role: Some(MessageRole::Assistant),
                    content: Some("Hello!".to_string()),
                    tool_calls: None,
                    ..Default::default()
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
        };

        let json = serde_json::to_string(&chunk).unwrap();
        let parsed: ChatCompletionChunk = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, chunk.id);
        assert_eq!(parsed.choices.len(), 1);
        assert_eq!(parsed.choices[0].delta.content, Some("Hello!".to_string()));
    }

    #[test]
    fn test_streaming_with_tool_calls() {
        let chunk = ChatCompletionChunk {
            id: "test".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            system_fingerprint: None,
            choices: vec![ChatCompletionChunkChoice {
                index: 0,
                delta: ChatCompletionDelta {
                    role: Some(MessageRole::Assistant),
                    content: None,
                    tool_calls: Some(vec![ToolCallDelta {
                        index: 0,
                        id: Some("call_123".to_string()),
                        tool_type: Some("function".to_string()),
                        function: Some(FunctionCallDelta {
                            name: Some("get_weather".to_string()),
                            arguments: Some("{\"city\":\"NYC\"}".to_string()),
                        }),
                    }]),
                    ..Default::default()
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
        };

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("tool_calls"));
        assert!(json.contains("get_weather"));
        assert!(json.contains("NYC"));
    }
}
