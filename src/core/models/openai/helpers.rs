//! Helper implementations and Display traits for OpenAI types
//!
//! This module provides Display trait implementations and other helper
//! utilities for OpenAI-compatible API types.

use std::fmt;

use super::messages::MessageRole;

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::Developer => write!(f, "developer"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::Function => write!(f, "function"),
            MessageRole::Tool => write!(f, "tool"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::openai::{
        ChatCompletionRequest, ChatMessage, ContentPart, MessageContent,
    };

    #[test]
    fn test_chat_completion_request_serialization() {
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            }],
            temperature: Some(0.7),
            ..Default::default()
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("gpt-4"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_message_content_variants() {
        let text_content = MessageContent::Text("Hello".to_string());
        let json = serde_json::to_string(&text_content).unwrap();
        assert_eq!(json, "\"Hello\"");

        let parts_content = MessageContent::Parts(vec![ContentPart::Text {
            text: "Hello".to_string(),
        }]);
        let json = serde_json::to_string(&parts_content).unwrap();
        assert!(json.contains("text"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_message_role_display() {
        assert_eq!(MessageRole::System.to_string(), "system");
        assert_eq!(MessageRole::User.to_string(), "user");
        assert_eq!(MessageRole::Assistant.to_string(), "assistant");
        assert_eq!(MessageRole::Function.to_string(), "function");
        assert_eq!(MessageRole::Tool.to_string(), "tool");
    }
}
