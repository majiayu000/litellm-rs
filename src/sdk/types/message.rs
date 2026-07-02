use serde::{Deserialize, Serialize};

use super::ToolCall;

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
}

/// Delta message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelta {
    /// Message role
    pub role: Option<Role>,
    /// Message content
    pub content: Option<String>,
    /// Tool calls
    pub tool_calls: Option<Vec<ToolCall>>,
}
