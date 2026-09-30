//! Public completion streams use the canonical provider response types.
//!
//! Tool argument fragments, usage, thinking, audio, and finish reasons are
//! preserved without a second response schema or a lossy conversion.

use futures::stream::BoxStream;

pub use crate::core::types::responses::{
    ChatChunk as CompletionChunk, ChatDelta as StreamDelta, ChatStreamChoice as StreamChoice,
};

pub type CompletionStream =
    BoxStream<'static, Result<CompletionChunk, crate::utils::error::gateway_error::GatewayError>>;
