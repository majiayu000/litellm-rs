//! Anthropic Streaming Module
//!
//! Uses the unified SSE parser with AnthropicTransformer for event-based streaming.

use std::{collections::HashMap, pin::Pin};

use bytes::Bytes;
use futures::Stream;
use pin_project_lite::pin_project;

use crate::core::providers::base::sse::{AnthropicTransformer, UnifiedSSEStream};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{chat::ChatRequest, responses::ChatChunk};

use super::client::AnthropicClient;
use super::http_annotations::HttpAnnotationSender;

/// Anthropic SSE stream type
pub type AnthropicSSEStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    AnthropicTransformer,
>;

pin_project! {
    /// Anthropic streaming processor
    pub struct AnthropicStream {
        #[pin]
        inner: Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>,
    }
}

impl AnthropicStream {
    /// Create stream from response
    pub fn from_response(response: reqwest::Response, model: String) -> Self {
        Self::from_response_with_tool_name_map(response, model, HashMap::new())
    }

    pub fn from_response_with_tool_name_map(
        response: reqwest::Response,
        model: String,
        tool_name_map: HashMap<String, String>,
    ) -> Self {
        let transformer = AnthropicTransformer::new(model).with_tool_name_map(tool_name_map);
        let stream = UnifiedSSEStream::new(Box::pin(response.bytes_stream()), transformer);
        Self {
            inner: Box::pin(stream),
        }
    }
}

impl AnthropicClient {
    pub(crate) async fn chat_stream_chunks(
        &self,
        request: ChatRequest,
        http_annotation_sender: Option<HttpAnnotationSender>,
    ) -> Result<AnthropicStream, ProviderError> {
        let tool_name_map = self.anthropic_tool_name_map_for_request(&request)?;
        let model = request.model.clone();
        let response = self.chat_stream(request).await?;
        let transformer = AnthropicTransformer::new(model)
            .with_tool_name_map(tool_name_map)
            .with_http_annotation_sender(http_annotation_sender);
        let stream = UnifiedSSEStream::new(Box::pin(response.bytes_stream()), transformer);
        Ok(AnthropicStream {
            inner: Box::pin(stream),
        })
    }
}

impl Stream for AnthropicStream {
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.project();
        this.inner.poll_next(cx)
    }
}
