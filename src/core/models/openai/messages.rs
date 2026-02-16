//! Message types for OpenAI-compatible API
//!
//! This module defines chat messages, roles, content types, and content parts
//! for multimodal interactions.

use serde::{Deserialize, Serialize};

use super::audio::AudioContent;
use super::tools::{FunctionCall, ToolCall};

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Message role
    pub role: MessageRole,
    /// Message content
    pub content: Option<MessageContent>,
    /// Message name (for function/tool messages)
    pub name: Option<String>,
    /// Function call (legacy)
    pub function_call: Option<FunctionCall>,
    /// Tool calls
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Tool call ID (for tool messages)
    pub tool_call_id: Option<String>,
    /// Audio content
    pub audio: Option<AudioContent>,
}

/// Message role
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message role
    System,
    /// User message role
    User,
    /// Assistant message role
    Assistant,
    /// Function call message role
    Function,
    /// Tool call message role
    Tool,
}

/// Message content (can be string or array of content parts)
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text content
    Text(String),
    /// Multi-part content (text, images, audio)
    Parts(Vec<ContentPart>),
}

/// Content part for multimodal messages
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    /// Text content part
    #[serde(rename = "text")]
    Text {
        /// Text content
        text: String,
    },
    /// Image URL content part
    #[serde(rename = "image_url")]
    ImageUrl {
        /// Image URL details
        image_url: ImageUrl,
    },
    /// Audio content part
    #[serde(rename = "audio")]
    Audio {
        /// Audio content details
        audio: AudioContent,
    },
}

/// Image URL content
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct ImageUrl {
    /// Image URL
    pub url: String,
    /// Detail level
    pub detail: Option<String>,
}

impl From<MessageRole> for crate::core::types::message::MessageRole {
    fn from(value: MessageRole) -> Self {
        match value {
            MessageRole::System => Self::System,
            MessageRole::User => Self::User,
            MessageRole::Assistant => Self::Assistant,
            MessageRole::Function => Self::Function,
            MessageRole::Tool => Self::Tool,
        }
    }
}

impl From<crate::core::types::message::MessageRole> for MessageRole {
    fn from(value: crate::core::types::message::MessageRole) -> Self {
        match value {
            crate::core::types::message::MessageRole::System => Self::System,
            crate::core::types::message::MessageRole::User => Self::User,
            crate::core::types::message::MessageRole::Assistant => Self::Assistant,
            crate::core::types::message::MessageRole::Function => Self::Function,
            crate::core::types::message::MessageRole::Tool => Self::Tool,
        }
    }
}

impl From<ContentPart> for crate::core::types::content::ContentPart {
    fn from(value: ContentPart) -> Self {
        match value {
            ContentPart::Text { text } => Self::Text { text },
            ContentPart::ImageUrl { image_url } => Self::ImageUrl {
                image_url: crate::core::types::content::ImageUrl {
                    url: image_url.url,
                    detail: image_url.detail,
                },
            },
            ContentPart::Audio { audio } => Self::Audio {
                audio: crate::core::types::content::AudioData {
                    data: audio.data,
                    format: Some(audio.format),
                },
            },
        }
    }
}

impl From<crate::core::types::content::ContentPart> for ContentPart {
    fn from(value: crate::core::types::content::ContentPart) -> Self {
        match value {
            crate::core::types::content::ContentPart::Text { text } => Self::Text { text },
            crate::core::types::content::ContentPart::ImageUrl { image_url } => Self::ImageUrl {
                image_url: ImageUrl {
                    url: image_url.url,
                    detail: image_url.detail,
                },
            },
            crate::core::types::content::ContentPart::Audio { audio } => Self::Audio {
                audio: AudioContent {
                    data: audio.data,
                    format: audio.format.unwrap_or_else(|| "unknown".to_string()),
                },
            },
            _ => Self::Text {
                text: "[unsupported content part]".to_string(),
            },
        }
    }
}

impl From<MessageContent> for crate::core::types::message::MessageContent {
    fn from(value: MessageContent) -> Self {
        match value {
            MessageContent::Text(text) => Self::Text(text),
            MessageContent::Parts(parts) => {
                Self::Parts(parts.into_iter().map(Into::into).collect())
            }
        }
    }
}

impl From<crate::core::types::message::MessageContent> for MessageContent {
    fn from(value: crate::core::types::message::MessageContent) -> Self {
        match value {
            crate::core::types::message::MessageContent::Text(text) => Self::Text(text),
            crate::core::types::message::MessageContent::Parts(parts) => {
                Self::Parts(parts.into_iter().map(Into::into).collect())
            }
        }
    }
}

impl From<ChatMessage> for crate::core::types::chat::ChatMessage {
    fn from(value: ChatMessage) -> Self {
        Self {
            role: value.role.into(),
            content: value.content.map(Into::into),
            thinking: None,
            name: value.name,
            tool_calls: value
                .tool_calls
                .map(|calls| calls.into_iter().map(Into::into).collect()),
            tool_call_id: value.tool_call_id,
            function_call: value.function_call.map(Into::into),
        }
    }
}

impl From<crate::core::types::chat::ChatMessage> for ChatMessage {
    fn from(value: crate::core::types::chat::ChatMessage) -> Self {
        Self {
            role: value.role.into(),
            content: value.content.map(Into::into),
            name: value.name,
            function_call: value.function_call.map(Into::into),
            tool_calls: value
                .tool_calls
                .map(|calls| calls.into_iter().map(Into::into).collect()),
            tool_call_id: value.tool_call_id,
            audio: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_openai_to_core_conversion() {
        let msg = ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text("hello".to_string())),
            name: Some("assistant".to_string()),
            function_call: Some(FunctionCall {
                name: "sum".to_string(),
                arguments: "{\"a\":1,\"b\":2}".to_string(),
            }),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                tool_type: "function".to_string(),
                function: FunctionCall {
                    name: "sum".to_string(),
                    arguments: "{\"a\":1,\"b\":2}".to_string(),
                },
            }]),
            tool_call_id: Some("call_1".to_string()),
            audio: None,
        };

        let core_msg: crate::core::types::chat::ChatMessage = msg.into();
        assert_eq!(
            core_msg.role,
            crate::core::types::message::MessageRole::Assistant
        );
        assert_eq!(
            core_msg.tool_calls.as_ref().map(|calls| calls.len()),
            Some(1)
        );
    }

    #[test]
    fn test_chat_message_core_to_openai_conversion() {
        let core_msg = crate::core::types::chat::ChatMessage {
            role: crate::core::types::message::MessageRole::User,
            content: Some(crate::core::types::message::MessageContent::Text(
                "hello".to_string(),
            )),
            thinking: None,
            name: Some("user".to_string()),
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
        };

        let openai_msg: ChatMessage = core_msg.into();
        assert_eq!(openai_msg.role, MessageRole::User);
        assert!(matches!(openai_msg.content, Some(MessageContent::Text(_))));
        assert!(openai_msg.audio.is_none());
    }
}
