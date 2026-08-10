//! Compatibility facade for the canonical router runtime.

use super::{CompletionOptions, CompletionResponse, CompletionStream, Message, Router};

use crate::core::router::{RuntimeBinding, default_runtime};
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;

mod router_impl;

/// Completion compatibility facade backed only by [`crate::core::router::UnifiedRouter`].
///
/// New integrations should retain a [`RuntimeBinding`] and use [`Self::from_runtime`].
/// The type remains public for the 0.6 compatibility window.
pub struct DefaultRouter {
    runtime_binding: Option<RuntimeBinding>,
}

impl DefaultRouter {
    /// Create a facade that binds the process-default runtime for each operation.
    pub async fn new() -> Result<Self> {
        Ok(Self {
            runtime_binding: None,
        })
    }

    /// Create a completion facade backed by an explicit canonical runtime.
    pub fn from_runtime(runtime: RuntimeBinding) -> Self {
        Self {
            runtime_binding: Some(runtime),
        }
    }
}

/// Fallback router retained for source compatibility.
pub struct ErrorRouter {
    error: String,
}

impl ErrorRouter {
    /// Create a facade that always returns the supplied initialization error.
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }
}

#[async_trait]
impl Router for ErrorRouter {
    async fn complete(
        &self,
        _model: &str,
        _messages: Vec<Message>,
        _options: CompletionOptions,
    ) -> Result<CompletionResponse> {
        Err(GatewayError::internal(format!(
            "Router initialization failed: {}",
            self.error
        )))
    }

    async fn complete_stream(
        &self,
        _model: &str,
        _messages: Vec<Message>,
        _options: CompletionOptions,
    ) -> Result<CompletionStream> {
        Err(GatewayError::internal(format!(
            "Router initialization failed: {}",
            self.error
        )))
    }
}

/// Core completion function backed by the process-default canonical runtime.
pub async fn completion(
    model: &str,
    messages: Vec<Message>,
    options: Option<CompletionOptions>,
) -> Result<CompletionResponse> {
    let handle = default_runtime().map_err(GatewayError::from)?;
    router_impl::complete_with_runtime_handle(&handle, model, messages, options.unwrap_or_default())
        .await
}

/// Async compatibility alias for [`completion`].
pub async fn acompletion(
    model: &str,
    messages: Vec<Message>,
    options: Option<CompletionOptions>,
) -> Result<CompletionResponse> {
    completion(model, messages, options).await
}

/// Streaming completion backed by the process-default canonical runtime.
pub async fn completion_stream(
    model: &str,
    messages: Vec<Message>,
    options: Option<CompletionOptions>,
) -> Result<CompletionStream> {
    let handle = default_runtime().map_err(GatewayError::from)?;
    router_impl::complete_stream_with_runtime_handle(
        &handle,
        model,
        messages,
        options.unwrap_or_default(),
    )
    .await
}

/// Convert the canonical provider chunk into the compatibility stream shape.
fn convert_chat_chunk_to_completion_chunk(
    chunk: crate::core::types::responses::ChatChunk,
) -> super::stream::CompletionChunk {
    super::stream::CompletionChunk {
        id: chunk.id,
        object: chunk.object,
        created: chunk.created,
        model: chunk.model,
        choices: chunk
            .choices
            .into_iter()
            .map(|choice| super::stream::StreamChoice {
                index: choice.index,
                delta: super::stream::StreamDelta {
                    role: choice.delta.role.map(|role| role.to_string()),
                    content: choice.delta.content,
                    tool_calls: None,
                },
                finish_reason: choice.finish_reason,
            })
            .collect(),
    }
}
