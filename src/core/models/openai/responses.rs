//! Response types for OpenAI-compatible API
//!
//! This module defines response structures for chat completions, text completions,
//! embeddings, image generation, and model listings, including streaming variants.

use serde::{Deserialize, Serialize};

use super::audio::AudioDelta;
#[cfg(test)]
use super::messages::MessageContent;
use super::messages::{ChatMessage, MessageRole};
use super::tools::{FunctionCallDelta, ToolCallDelta};

/// Chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    /// Response ID
    pub id: String,
    /// Object type
    pub object: String,
    /// Creation timestamp
    pub created: u64,
    /// Model used
    pub model: String,
    /// System fingerprint
    pub system_fingerprint: Option<String>,
    /// Choices
    pub choices: Vec<ChatChoice>,
    /// Usage statistics
    pub usage: Option<Usage>,
}

/// Chat choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    /// Choice index
    pub index: u32,
    /// Message
    pub message: ChatMessage,
    /// Logprobs
    pub logprobs: Option<Logprobs>,
    /// Finish reason
    pub finish_reason: Option<String>,
}

/// Chat completion choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChoice {
    /// Choice index
    pub index: u32,
    /// Message content
    pub message: ChatMessage,
    /// Finish reason
    pub finish_reason: Option<String>,
    /// Log probabilities
    pub logprobs: Option<serde_json::Value>,
}

/// Logprobs information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logprobs {
    /// Content logprobs
    pub content: Option<Vec<ContentLogprob>>,
}

/// Content logprob
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentLogprob {
    /// Token
    pub token: String,
    /// Log probability
    pub logprob: f64,
    /// Bytes
    pub bytes: Option<Vec<u8>>,
    /// Top logprobs
    pub top_logprobs: Option<Vec<TopLogprob>>,
}

/// Top logprob
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopLogprob {
    /// Token
    pub token: String,
    /// Log probability
    pub logprob: f64,
    /// Bytes
    pub bytes: Option<Vec<u8>>,
}

// Usage / PromptTokensDetails / CompletionTokensDetails are re-exported from
// the canonical types tree (core::types::responses::*). The duplicates that
// used to live here had field-for-field identical layouts and were a
// downstream of the parallel-type-tree schism tracked in #519.
pub use crate::core::types::responses::{CompletionTokensDetails, PromptTokensDetails, Usage};

/// Chat completion chunk (for streaming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    /// Response ID
    pub id: String,
    /// Object type
    pub object: String,
    /// Creation timestamp
    pub created: u64,
    /// Model used
    pub model: String,
    /// System fingerprint
    pub system_fingerprint: Option<String>,
    /// Choices
    pub choices: Vec<ChatChoiceDelta>,
    /// Usage statistics (only in final chunk)
    pub usage: Option<Usage>,
}

/// Chat choice delta (for streaming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoiceDelta {
    /// Choice index
    pub index: u32,
    /// Delta message
    pub delta: ChatMessageDelta,
    /// Logprobs
    pub logprobs: Option<Logprobs>,
    /// Finish reason
    pub finish_reason: Option<String>,
}

/// Chat message delta (for streaming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageDelta {
    /// Message role (only in first chunk)
    pub role: Option<MessageRole>,
    /// Content delta
    pub content: Option<String>,
    /// Function call delta (legacy)
    pub function_call: Option<FunctionCallDelta>,
    /// Tool calls delta
    pub tool_calls: Option<Vec<ToolCallDelta>>,
    /// Audio delta
    pub audio: Option<AudioDelta>,
}

/// Text completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Response ID
    pub id: String,
    /// Object type
    pub object: String,
    /// Creation timestamp
    pub created: u64,
    /// Model used
    pub model: String,
    /// Completion choices
    pub choices: Vec<CompletionChoice>,
    /// Usage statistics
    pub usage: Option<Usage>,
}

/// Completion choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChoice {
    /// Generated text
    pub text: String,
    /// Choice index
    pub index: u32,
    /// Log probabilities
    pub logprobs: Option<serde_json::Value>,
    /// Finish reason
    pub finish_reason: Option<String>,
}

/// Embedding response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// Object type
    pub object: String,
    /// Embedding data
    pub data: Vec<EmbeddingObject>,
    /// Model used
    pub model: String,
    /// Usage statistics
    pub usage: EmbeddingUsage,
}

/// Embedding object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingObject {
    /// Object type
    pub object: String,
    /// Embedding vector
    pub embedding: Vec<f64>,
    /// Index
    pub index: u32,
}

/// Embedding usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    /// Prompt tokens
    pub prompt_tokens: u32,
    /// Total tokens
    pub total_tokens: u32,
}

/// Image generation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationResponse {
    /// Creation timestamp
    pub created: u64,
    /// Generated images
    pub data: Vec<ImageObject>,
}

/// Image object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageObject {
    /// Image URL
    pub url: Option<String>,
    /// Base64 encoded image
    pub b64_json: Option<String>,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// Model ID
    pub id: String,
    /// Object type
    pub object: String,
    /// Creation timestamp
    pub created: u64,
    /// Owner
    pub owned_by: String,
}

/// Model list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelListResponse {
    /// Object type
    pub object: String,
    /// List of models
    pub data: Vec<Model>,
}

#[cfg(test)]
#[path = "responses_tests.rs"]
mod tests;
