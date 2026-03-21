//! Message types for chat completions

use serde::{Deserialize, Serialize};

/// Message role enum
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message
    System,
    /// Developer message (replaces System for OpenAI o-series models)
    Developer,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool message
    Tool,
    /// Function message (backward compatibility)
    Function,
}

impl MessageRole {
    /// Check if the message role is effectively empty
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::Developer => write!(f, "developer"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::Tool => write!(f, "tool"),
            MessageRole::Function => write!(f, "function"),
        }
    }
}

/// Message content (supports multimodal)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text content
    Text(String),
    /// Multi-part content (supports text, images, audio, etc.)
    Parts(Vec<super::content::ContentPart>),
}

impl std::fmt::Display for MessageContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageContent::Text(text) => write!(f, "{}", text),
            MessageContent::Parts(parts) => {
                use super::content::ContentPart;
                let texts: Vec<String> = parts
                    .iter()
                    .filter_map(|part| match part {
                        ContentPart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect();
                write!(f, "{}", texts.join(" "))
            }
        }
    }
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_role_display() {
        assert_eq!(MessageRole::System.to_string(), "system");
        assert_eq!(MessageRole::User.to_string(), "user");
        assert_eq!(MessageRole::Assistant.to_string(), "assistant");
        assert_eq!(MessageRole::Tool.to_string(), "tool");
        assert_eq!(MessageRole::Function.to_string(), "function");
    }

    #[test]
    fn test_message_role_serialization() {
        let role = MessageRole::User;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"user\"");

        let deserialized: MessageRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, MessageRole::User);
    }

    #[test]
    fn test_message_role_is_empty() {
        assert!(!MessageRole::System.is_empty());
        assert!(!MessageRole::User.is_empty());
        assert!(!MessageRole::Assistant.is_empty());
    }

    #[test]
    fn test_message_content_text() {
        let content = MessageContent::Text("Hello".to_string());
        assert_eq!(content.to_string(), "Hello");
    }

    #[test]
    fn test_message_content_from_string() {
        let content: MessageContent = "Hello".into();
        assert_eq!(content.to_string(), "Hello");

        let content: MessageContent = String::from("World").into();
        assert_eq!(content.to_string(), "World");
    }

    #[test]
    fn test_message_content_serialization() {
        let content = MessageContent::Text("Test message".to_string());
        let json = serde_json::to_string(&content).unwrap();
        assert_eq!(json, "\"Test message\"");
    }
}
