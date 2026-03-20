//! Message types for OpenAI-compatible API
//!
//! This module defines chat messages, roles, content types, and content parts
//! for multimodal interactions.

use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

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
    /// Developer message role (replaces System for OpenAI o-series models)
    Developer,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text content
    Text(String),
    /// Multi-part content (text, images, audio)
    Parts(Vec<ContentPart>),
}

/// Content part for multimodal messages
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Base64 image part
    #[serde(rename = "image")]
    Image {
        /// Base64 image source
        source: ImageSource,
        /// Detail level
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
        /// URL compatibility field
        #[serde(skip_serializing_if = "Option::is_none")]
        image_url: Option<ImageUrl>,
    },
    /// Document part
    #[serde(rename = "document")]
    Document {
        /// Document source
        source: DocumentSource,
        /// Cache control
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// Tool result
    #[serde(rename = "tool_result")]
    ToolResult {
        /// Tool use ID
        tool_use_id: String,
        /// Result payload
        content: serde_json::Value,
        /// Error flag
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    /// Tool use
    #[serde(rename = "tool_use")]
    ToolUse {
        /// Tool use ID
        id: String,
        /// Tool name
        name: String,
        /// Tool input payload
        input: serde_json::Value,
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

/// Image source content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    /// MIME type
    pub media_type: String,
    /// Base64 data
    pub data: String,
}

/// Document source content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSource {
    /// MIME type
    pub media_type: String,
    /// Base64 data
    pub data: String,
}

/// Cache control metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    /// Cache type
    #[serde(rename = "type")]
    pub cache_type: String,
}

impl Hash for MessageContent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Text(text) => {
                0u8.hash(state);
                text.hash(state);
            }
            Self::Parts(parts) => {
                1u8.hash(state);
                parts.len().hash(state);
                for part in parts {
                    part.hash(state);
                }
            }
        }
    }
}

impl Hash for ContentPart {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Text { text } => {
                0u8.hash(state);
                text.hash(state);
            }
            Self::ImageUrl { image_url } => {
                1u8.hash(state);
                image_url.hash(state);
            }
            Self::Audio { audio } => {
                2u8.hash(state);
                audio.hash(state);
            }
            Self::Image {
                source,
                detail,
                image_url,
            } => {
                3u8.hash(state);
                source.media_type.hash(state);
                source.data.hash(state);
                detail.hash(state);
                image_url.hash(state);
            }
            Self::Document {
                source,
                cache_control,
            } => {
                4u8.hash(state);
                source.media_type.hash(state);
                source.data.hash(state);
                cache_control.as_ref().map(|c| &c.cache_type).hash(state);
            }
            Self::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                5u8.hash(state);
                tool_use_id.hash(state);
                serde_json::to_string(content)
                    .unwrap_or_default()
                    .hash(state);
                is_error.hash(state);
            }
            Self::ToolUse { id, name, input } => {
                6u8.hash(state);
                id.hash(state);
                name.hash(state);
                serde_json::to_string(input).unwrap_or_default().hash(state);
            }
        }
    }
}

impl From<MessageRole> for crate::core::types::message::MessageRole {
    fn from(value: MessageRole) -> Self {
        match value {
            MessageRole::System => Self::System,
            MessageRole::Developer => Self::Developer,
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
            crate::core::types::message::MessageRole::Developer => Self::Developer,
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
            ContentPart::Image {
                source,
                detail,
                image_url,
            } => Self::Image {
                source: crate::core::types::content::ImageSource {
                    media_type: source.media_type,
                    data: source.data,
                },
                detail,
                image_url: image_url.map(|url| crate::core::types::content::ImageUrl {
                    url: url.url,
                    detail: url.detail,
                }),
            },
            ContentPart::Document {
                source,
                cache_control,
            } => Self::Document {
                source: crate::core::types::content::DocumentSource {
                    media_type: source.media_type,
                    data: source.data,
                },
                cache_control: cache_control.map(|cc| crate::core::types::content::CacheControl {
                    cache_type: cc.cache_type,
                }),
            },
            ContentPart::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Self::ToolResult {
                tool_use_id,
                content,
                is_error,
            },
            ContentPart::ToolUse { id, name, input } => Self::ToolUse { id, name, input },
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
                    format: audio.format.unwrap_or_else(|| "mp3".to_string()),
                },
            },
            crate::core::types::content::ContentPart::Image {
                source,
                detail,
                image_url,
            } => Self::Image {
                source: ImageSource {
                    media_type: source.media_type,
                    data: source.data,
                },
                detail,
                image_url: image_url.map(|url| ImageUrl {
                    url: url.url,
                    detail: url.detail,
                }),
            },
            crate::core::types::content::ContentPart::Document {
                source,
                cache_control,
            } => Self::Document {
                source: DocumentSource {
                    media_type: source.media_type,
                    data: source.data,
                },
                cache_control: cache_control.map(|cc| CacheControl {
                    cache_type: cc.cache_type,
                }),
            },
            crate::core::types::content::ContentPart::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Self::ToolResult {
                tool_use_id,
                content,
                is_error,
            },
            crate::core::types::content::ContentPart::ToolUse { id, name, input } => {
                Self::ToolUse { id, name, input }
            }
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

    #[test]
    fn test_content_part_roundtrip_document() {
        let part = ContentPart::Document {
            source: DocumentSource {
                media_type: "application/pdf".to_string(),
                data: "base64pdf".to_string(),
            },
            cache_control: Some(CacheControl {
                cache_type: "ephemeral".to_string(),
            }),
        };

        let core: crate::core::types::content::ContentPart = part.clone().into();
        let back: ContentPart = core.into();
        match back {
            ContentPart::Document { source, .. } => {
                assert_eq!(source.media_type, "application/pdf");
            }
            _ => panic!("expected document"),
        }
    }

    #[test]
    fn test_content_part_roundtrip_tool_use() {
        let part = ContentPart::ToolUse {
            id: "tool-1".to_string(),
            name: "search".to_string(),
            input: serde_json::json!({"q":"hello"}),
        };

        let core: crate::core::types::content::ContentPart = part.clone().into();
        let back: ContentPart = core.into();
        match back {
            ContentPart::ToolUse { id, name, .. } => {
                assert_eq!(id, "tool-1");
                assert_eq!(name, "search");
            }
            _ => panic!("expected tool_use"),
        }
    }
}
