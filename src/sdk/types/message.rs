use serde::{Deserialize, Serialize};

use super::{ToolCall, ToolCallDelta};

/// Message role
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System message
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool message
    Tool,
}

/// Message content type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    /// Plain text content
    Text(String),
    /// Multimodal content
    Multimodal(Vec<ContentPart>),
}

/// Content part
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    /// Text content
    #[serde(rename = "text")]
    Text {
        /// Text string
        text: String,
    },
    /// Image content
    #[serde(rename = "image_url")]
    Image {
        /// Image URL information
        image_url: ImageUrl,
    },
    /// Audio content
    #[serde(rename = "audio")]
    Audio {
        /// Audio data
        audio: AudioData,
    },
}

/// Image URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// Image URL or base64 data
    pub url: String,
    /// Image detail level
    pub detail: Option<String>,
}

/// Audio data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    /// Audio data or URL
    pub data: String,
    /// Audio format
    pub format: Option<String>,
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message role
    pub role: Role,
    /// Message content
    pub content: Option<Content>,
    /// Message name
    pub name: Option<String>,
    /// Tool calls
    pub tool_calls: Option<Vec<ToolCall>>,
    /// ID of the assistant tool call answered by a tool-role message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Build a tool result associated with a preceding assistant tool call.
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(Content::Text(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// Delta message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelta {
    /// Message role
    pub role: Option<Role>,
    /// Message content
    pub content: Option<String>,
    /// Partial tool calls, correlated by index within each choice.
    /// IDs and function names normally appear only in the first delta.
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}
