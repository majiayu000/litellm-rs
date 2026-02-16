use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContent {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: ToolFunction,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: String,
}

pub struct RequestUtils;

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== MessageContent Tests ====================

    #[test]
    fn test_message_content_creation() {
        let msg = MessageContent {
            role: "user".to_string(),
            content: "Hello, how are you?".to_string(),
        };

        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello, how are you?");
    }

    #[test]
    fn test_message_content_system_role() {
        let msg = MessageContent {
            role: "system".to_string(),
            content: "You are a helpful assistant.".to_string(),
        };

        assert_eq!(msg.role, "system");
    }

    #[test]
    fn test_message_content_assistant_role() {
        let msg = MessageContent {
            role: "assistant".to_string(),
            content: "I'm doing well, thank you!".to_string(),
        };

        assert_eq!(msg.role, "assistant");
    }

    #[test]
    fn test_message_content_clone() {
        let msg = MessageContent {
            role: "user".to_string(),
            content: "Test message".to_string(),
        };

        let cloned = msg.clone();
        assert_eq!(cloned.role, msg.role);
        assert_eq!(cloned.content, msg.content);
    }

    #[test]
    fn test_message_content_debug() {
        let msg = MessageContent {
            role: "user".to_string(),
            content: "Debug test".to_string(),
        };

        let debug_str = format!("{:?}", msg);
        assert!(debug_str.contains("MessageContent"));
        assert!(debug_str.contains("user"));
    }

    #[test]
    fn test_message_content_serialization() {
        let msg = MessageContent {
            role: "user".to_string(),
            content: "Serialize test".to_string(),
        };

        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "Serialize test");
    }

    #[test]
    fn test_message_content_deserialization() {
        let json = r#"{"role": "assistant", "content": "Hello!"}"#;
        let msg: MessageContent = serde_json::from_str(json).unwrap();

        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "Hello!");
    }

    // ==================== ToolFunction Tests ====================

    #[test]
    fn test_tool_function_creation() {
        let func = ToolFunction {
            name: "get_weather".to_string(),
            arguments: r#"{"location": "New York"}"#.to_string(),
        };

        assert_eq!(func.name, "get_weather");
        assert!(func.arguments.contains("New York"));
    }

    #[test]
    fn test_tool_function_clone() {
        let func = ToolFunction {
            name: "search".to_string(),
            arguments: r#"{"query": "rust programming"}"#.to_string(),
        };

        let cloned = func.clone();
        assert_eq!(cloned.name, func.name);
        assert_eq!(cloned.arguments, func.arguments);
    }

    #[test]
    fn test_tool_function_debug() {
        let func = ToolFunction {
            name: "test_func".to_string(),
            arguments: "{}".to_string(),
        };

        let debug_str = format!("{:?}", func);
        assert!(debug_str.contains("ToolFunction"));
        assert!(debug_str.contains("test_func"));
    }

    #[test]
    fn test_tool_function_serialization() {
        let func = ToolFunction {
            name: "calculate".to_string(),
            arguments: r#"{"a": 1, "b": 2}"#.to_string(),
        };

        let json = serde_json::to_value(&func).unwrap();
        assert_eq!(json["name"], "calculate");
    }

    // ==================== ToolCall Tests ====================

    #[test]
    fn test_tool_call_creation() {
        let call = ToolCall {
            id: "call_123".to_string(),
            function: ToolFunction {
                name: "get_data".to_string(),
                arguments: "{}".to_string(),
            },
            r#type: "function".to_string(),
        };

        assert_eq!(call.id, "call_123");
        assert_eq!(call.r#type, "function");
        assert_eq!(call.function.name, "get_data");
    }

    #[test]
    fn test_tool_call_clone() {
        let call = ToolCall {
            id: "call_456".to_string(),
            function: ToolFunction {
                name: "process".to_string(),
                arguments: r#"{"input": "test"}"#.to_string(),
            },
            r#type: "function".to_string(),
        };

        let cloned = call.clone();
        assert_eq!(cloned.id, call.id);
        assert_eq!(cloned.function.name, call.function.name);
    }

    #[test]
    fn test_tool_call_debug() {
        let call = ToolCall {
            id: "debug_call".to_string(),
            function: ToolFunction {
                name: "debug_func".to_string(),
                arguments: "{}".to_string(),
            },
            r#type: "function".to_string(),
        };

        let debug_str = format!("{:?}", call);
        assert!(debug_str.contains("ToolCall"));
        assert!(debug_str.contains("debug_call"));
    }

    #[test]
    fn test_tool_call_serialization() {
        let call = ToolCall {
            id: "call_789".to_string(),
            function: ToolFunction {
                name: "api_call".to_string(),
                arguments: r#"{"url": "https://example.com"}"#.to_string(),
            },
            r#type: "function".to_string(),
        };

        let json = serde_json::to_value(&call).unwrap();
        assert_eq!(json["id"], "call_789");
        assert_eq!(json["type"], "function");
    }

    #[test]
    fn test_tool_call_workflow() {
        let tool_call = ToolCall {
            id: "call_weather_123".to_string(),
            function: ToolFunction {
                name: "get_current_weather".to_string(),
                arguments: serde_json::json!({
                    "location": "San Francisco, CA",
                    "unit": "celsius"
                })
                .to_string(),
            },
            r#type: "function".to_string(),
        };

        // Parse arguments back to verify
        let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments).unwrap();
        assert_eq!(args["location"], "San Francisco, CA");
        assert_eq!(args["unit"], "celsius");
    }
}
