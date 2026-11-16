//! Response types
//!
//! Defines unified data structures for all API responses

mod audio;
mod chat;
mod completion;
mod delta;
mod embedding;
mod error;
mod image;
mod logprobs;
mod usage;

// Re-export all types for backward compatibility
pub use audio::{AudioTranscriptionResponse, SegmentInfo, WordInfo};
pub use chat::{ChatChoice, ChatChunk, ChatResponse, ChatStreamChoice};
pub use completion::{CompletionChoice, CompletionResponse};
pub use delta::{AudioDelta, ChatDelta, FunctionCallDelta, ToolCallDelta};
pub use embedding::{EmbedResponse, EmbeddingData, EmbeddingResponse, EmbeddingUsage};
pub use error::{ApiError, ErrorResponse};
pub use image::{ImageData, ImageGenerationResponse, ImageResponse};
pub use logprobs::{FinishReason, LogProbs, TokenLogProb, TopLogProb};
pub(crate) use usage::deserialize_reported_cost;
pub use usage::{CompletionTokensDetails, PromptTokensDetails, Usage};
