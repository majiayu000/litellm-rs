//! OpenAI-compatible API models
//!
//! This module defines all the data structures that are compatible with OpenAI's API.
//! It is organized into sub-modules for better maintainability:
//!
//! - `messages` - Message types, roles, and content
//! - `requests` - Request structures for various API endpoints
//! - `tools` - Tool and function calling definitions
//! - `audio` - Audio-related types for multimodal interactions
//! - `responses` - Response structures including streaming variants
//! - `helpers` - Helper implementations and Display traits

pub mod audio;
pub mod helpers;
pub mod messages;
pub mod requests;
pub mod responses;
pub mod responses_api;
pub mod tools;

// Re-export all public types for backward compatibility
pub use audio::{AudioContent, AudioDelta, AudioParams};
pub use messages::{
    CacheControl, ChatMessage, ContentPart, DocumentSource, ImageSource, ImageUrl, MessageContent,
    MessageRole,
};
pub use requests::{
    ChatCompletionRequest, CompletionRequest, EmbeddingRequest, ImageGenerationRequest,
    ResponseFormat, StreamOptions,
};
pub use responses::{
    ChatChoice, ChatChoiceDelta, ChatCompletionChoice, ChatCompletionChunk, ChatCompletionResponse,
    ChatMessageDelta, CompletionChoice, CompletionResponse, CompletionTokensDetails,
    ContentLogprob, EmbeddingObject, EmbeddingResponse, EmbeddingUsage, ImageGenerationResponse,
    ImageObject, Logprobs, Model, ModelListResponse, PromptTokensDetails, TopLogprob, Usage,
};
pub use tools::{
    Function, FunctionCall, FunctionCallDelta, Tool, ToolCall, ToolCallDelta, ToolChoice,
    ToolChoiceFunction, ToolChoiceFunctionSpec,
};
