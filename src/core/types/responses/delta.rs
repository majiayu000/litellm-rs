//! Streaming delta types

use serde::{Deserialize, Serialize};

use super::super::message::MessageRole;
use super::super::thinking::ThinkingDelta;

/// Streaming audio delta content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDelta {
    /// Base64 encoded audio delta.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,

    /// Transcript text delta paired with the audio stream, when provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,

    /// Audio format, when provided by the upstream stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Streaming delta content
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatDelta {
    /// Role (usually only appears in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<MessageRole>,

    /// Content delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Thinking/reasoning delta (for thinking-enabled models)
    ///
    /// When streaming from thinking models, thinking content may arrive
    /// before or alongside the main content. Use this field to track
    /// the model's reasoning process in real-time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingDelta>,

    /// Tool call delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,

    /// Function call delta (backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCallDelta>,

    /// Audio output delta.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioDelta>,
}

impl ChatDelta {
    /// Check if this delta contains thinking content
    pub fn has_thinking(&self) -> bool {
        self.thinking.is_some()
    }

    /// Get thinking content if present
    pub fn thinking_content(&self) -> Option<&str> {
        self.thinking.as_ref().and_then(|t| t.content.as_deref())
    }
}

/// Tool call delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    /// Index
    pub index: u32,

    /// Call ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Tool type
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub tool_type: Option<String>,

    /// Function call delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

/// Function call delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    /// Function name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Parameter delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ChatDelta Tests ====================

    #[test]
    fn test_chat_delta_creation() {
        let delta = ChatDelta {
            role: Some(MessageRole::Assistant),
            content: Some("Hello".to_string()),
            ..Default::default()
        };
        assert!(delta.role.is_some());
        assert_eq!(delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_chat_delta_empty() {
        let delta = ChatDelta::default();
        assert!(delta.role.is_none());
        assert!(delta.content.is_none());
    }

    #[test]
    fn test_chat_delta_has_thinking_false() {
        let delta = ChatDelta {
            content: Some("response".to_string()),
            ..Default::default()
        };
        assert!(!delta.has_thinking());
    }

    #[test]
    fn test_chat_delta_has_thinking_true() {
        let delta = ChatDelta {
            thinking: Some(ThinkingDelta {
                content: Some("thinking...".to_string()),
                signature: None,
                is_start: None,
                is_complete: None,
            }),
            ..Default::default()
        };
        assert!(delta.has_thinking());
    }

    #[test]
    fn test_chat_delta_thinking_content() {
        let delta = ChatDelta {
            thinking: Some(ThinkingDelta {
                content: Some("Let me think...".to_string()),
                signature: None,
                is_start: None,
                is_complete: None,
            }),
            ..Default::default()
        };
        assert_eq!(delta.thinking_content(), Some("Let me think..."));
    }

    #[test]
    fn test_chat_delta_thinking_content_none() {
        let delta = ChatDelta::default();
        assert_eq!(delta.thinking_content(), None);
    }

    #[test]
    fn test_chat_delta_serialization() {
        let delta = ChatDelta {
            role: Some(MessageRole::Assistant),
            content: Some("Hi".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains("assistant"));
        assert!(json.contains("Hi"));
    }

    #[test]
    fn test_chat_delta_serialization_minimal() {
        let delta = ChatDelta::default();
        let json = serde_json::to_string(&delta).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_chat_delta_deserialization() {
        let json = r#"{"role": "assistant", "content": "Hello!"}"#;
        let delta: ChatDelta = serde_json::from_str(json).unwrap();
        assert!(delta.role.is_some());
        assert_eq!(delta.content, Some("Hello!".to_string()));
    }

    // ==================== ToolCallDelta Tests ====================

    #[test]
    fn test_tool_call_delta_creation() {
        let delta = ToolCallDelta {
            index: 0,
            id: Some("call_123".to_string()),
            tool_type: Some("function".to_string()),
            function: None,
        };
        assert_eq!(delta.index, 0);
        assert_eq!(delta.id, Some("call_123".to_string()));
    }

    #[test]
    fn test_tool_call_delta_with_function() {
        let delta = ToolCallDelta {
            index: 0,
            id: Some("call_456".to_string()),
            tool_type: Some("function".to_string()),
            function: Some(FunctionCallDelta {
                name: Some("get_weather".to_string()),
                arguments: Some("{\"location\":".to_string()),
            }),
        };
        assert!(delta.function.is_some());
        let func = delta.function.unwrap();
        assert_eq!(func.name, Some("get_weather".to_string()));
    }

    #[test]
    fn test_tool_call_delta_serialization() {
        let delta = ToolCallDelta {
            index: 1,
            id: Some("call_789".to_string()),
            tool_type: Some("function".to_string()),
            function: Some(FunctionCallDelta {
                name: Some("search".to_string()),
                arguments: None,
            }),
        };
        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains("1"));
        assert!(json.contains("call_789"));
        assert!(json.contains("search"));
    }

    #[test]
    fn test_tool_call_delta_deserialization() {
        let json = r#"{
            "index": 0,
            "id": "call_abc",
            "type": "function",
            "function": {"name": "calculate", "arguments": "{}"}
        }"#;
        let delta: ToolCallDelta = serde_json::from_str(json).unwrap();
        assert_eq!(delta.index, 0);
        assert_eq!(delta.id, Some("call_abc".to_string()));
    }

    // ==================== FunctionCallDelta Tests ====================

    #[test]
    fn test_function_call_delta_creation() {
        let delta = FunctionCallDelta {
            name: Some("my_function".to_string()),
            arguments: Some("{\"key\": \"value\"}".to_string()),
        };
        assert_eq!(delta.name, Some("my_function".to_string()));
        assert!(delta.arguments.is_some());
    }

    #[test]
    fn test_function_call_delta_name_only() {
        let delta = FunctionCallDelta {
            name: Some("get_data".to_string()),
            arguments: None,
        };
        assert!(delta.name.is_some());
        assert!(delta.arguments.is_none());
    }

    #[test]
    fn test_function_call_delta_arguments_only() {
        let delta = FunctionCallDelta {
            name: None,
            arguments: Some("\"new_york\"".to_string()),
        };
        assert!(delta.name.is_none());
        assert!(delta.arguments.is_some());
    }

    #[test]
    fn test_function_call_delta_serialization() {
        let delta = FunctionCallDelta {
            name: Some("process".to_string()),
            arguments: Some("{\"input\": 42}".to_string()),
        };
        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains("process"));
        assert!(json.contains("input"));
    }

    #[test]
    fn test_function_call_delta_serialization_minimal() {
        let delta = FunctionCallDelta {
            name: None,
            arguments: None,
        };
        let json = serde_json::to_string(&delta).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_function_call_delta_deserialization() {
        let json = r#"{"name": "execute", "arguments": "{}"}"#;
        let delta: FunctionCallDelta = serde_json::from_str(json).unwrap();
        assert_eq!(delta.name, Some("execute".to_string()));
        assert_eq!(delta.arguments, Some("{}".to_string()));
    }

    // ==================== Clone and Debug Tests ====================

    #[test]
    fn test_chat_delta_clone() {
        let delta = ChatDelta {
            role: Some(MessageRole::User),
            content: Some("test".to_string()),
            ..Default::default()
        };
        let cloned = delta.clone();
        assert_eq!(cloned.content, Some("test".to_string()));
    }

    #[test]
    fn test_tool_call_delta_clone() {
        let delta = ToolCallDelta {
            index: 0,
            id: Some("id".to_string()),
            tool_type: None,
            function: None,
        };
        let cloned = delta.clone();
        assert_eq!(cloned.id, Some("id".to_string()));
    }

    #[test]
    fn test_function_call_delta_clone() {
        let delta = FunctionCallDelta {
            name: Some("func".to_string()),
            arguments: Some("args".to_string()),
        };
        let cloned = delta.clone();
        assert_eq!(cloned.name, Some("func".to_string()));
    }

    #[test]
    fn test_chat_delta_debug() {
        let delta = ChatDelta {
            content: Some("debug".to_string()),
            ..Default::default()
        };
        let debug = format!("{:?}", delta);
        assert!(debug.contains("ChatDelta"));
    }

    #[test]
    fn test_tool_call_delta_debug() {
        let delta = ToolCallDelta {
            index: 0,
            id: None,
            tool_type: None,
            function: None,
        };
        let debug = format!("{:?}", delta);
        assert!(debug.contains("ToolCallDelta"));
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_chat_delta_with_tool_calls() {
        let delta = ChatDelta {
            tool_calls: Some(vec![
                ToolCallDelta {
                    index: 0,
                    id: Some("call_1".to_string()),
                    tool_type: Some("function".to_string()),
                    function: Some(FunctionCallDelta {
                        name: Some("func1".to_string()),
                        arguments: None,
                    }),
                },
                ToolCallDelta {
                    index: 1,
                    id: Some("call_2".to_string()),
                    tool_type: Some("function".to_string()),
                    function: Some(FunctionCallDelta {
                        name: Some("func2".to_string()),
                        arguments: None,
                    }),
                },
            ]),
            ..Default::default()
        };
        assert_eq!(delta.tool_calls.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_chat_delta_with_function_call_backward_compat() {
        let delta = ChatDelta {
            role: Some(MessageRole::Assistant),
            function_call: Some(FunctionCallDelta {
                name: Some("old_function".to_string()),
                arguments: Some("{}".to_string()),
            }),
            ..Default::default()
        };
        assert!(delta.function_call.is_some());
    }

    #[test]
    fn test_tool_call_delta_large_index() {
        let delta = ToolCallDelta {
            index: u32::MAX,
            id: None,
            tool_type: None,
            function: None,
        };
        assert_eq!(delta.index, u32::MAX);
    }

    #[test]
    fn test_function_call_delta_empty_strings() {
        let delta = FunctionCallDelta {
            name: Some("".to_string()),
            arguments: Some("".to_string()),
        };
        assert!(delta.name.as_ref().unwrap().is_empty());
        assert!(delta.arguments.as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_chat_delta_thinking_with_empty_content() {
        let delta = ChatDelta {
            thinking: Some(ThinkingDelta {
                content: None,
                signature: None,
                is_start: None,
                is_complete: None,
            }),
            ..Default::default()
        };
        assert!(delta.has_thinking());
        assert_eq!(delta.thinking_content(), None);
    }
}
