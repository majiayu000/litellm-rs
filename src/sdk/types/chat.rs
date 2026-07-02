use serde::{Deserialize, Serialize};

use super::{Message, MessageDelta, Tool, ToolChoice, Usage};

/// Chat request
#[derive(Debug, Clone)]
pub struct SdkChatRequest {
    /// Model name
    pub model: String,
    /// Message list
    pub messages: Vec<Message>,
    /// Request options
    pub options: ChatOptions,
}

/// Chat options
#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    /// Temperature parameter
    pub temperature: Option<f32>,
    /// Maximum token count
    pub max_tokens: Option<u32>,
    /// Top-p parameter
    pub top_p: Option<f32>,
    /// Frequency penalty
    pub frequency_penalty: Option<f32>,
    /// Presence penalty
    pub presence_penalty: Option<f32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
    /// Stream response
    pub stream: bool,
    /// Tool list
    pub tools: Option<Vec<Tool>>,
    /// Tool choice
    pub tool_choice: Option<ToolChoice>,
}

/// Chat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Response ID
    pub id: String,
    /// Model name
    pub model: String,
    /// Choice list
    pub choices: Vec<ChatChoice>,
    /// Usage statistics
    pub usage: Usage,
    /// Creation timestamp
    pub created: u64,
}

/// Chat choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    /// Choice index
    pub index: u32,
    /// Message content
    pub message: Message,
    /// Finish reason
    pub finish_reason: Option<String>,
}

/// Chat chunk (streaming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunk {
    /// Response ID
    pub id: String,
    /// Model name
    pub model: String,
    /// Choice list
    pub choices: Vec<ChunkChoice>,
}

/// Streaming choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    /// Choice index
    pub index: u32,
    /// Delta message
    pub delta: MessageDelta,
    /// Finish reason
    pub finish_reason: Option<String>,
}
