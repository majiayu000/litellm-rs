//! Gemini Streaming Module
//!
//! Uses the unified SSE parser with GeminiTransformer for Gemini's candidates/parts format.

use std::pin::Pin;

use bytes::Bytes;
use futures::Stream;
use pin_project_lite::pin_project;

use crate::core::providers::base::sse::{GeminiTransformer, UnifiedSSEStream};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::ChatChunk;

/// Gemini SSE stream type
pub type GeminiSSEStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    GeminiTransformer,
>;

pin_project! {
    /// Handle streaming response
    pub struct GeminiStream {
        #[pin]
        inner: Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>,
    }
}

impl GeminiStream {
    /// Create from response
    pub fn from_response(response: reqwest::Response, model: String) -> Self {
        let transformer = GeminiTransformer::new(model);
        let stream = UnifiedSSEStream::new(Box::pin(response.bytes_stream()), transformer);
        Self {
            inner: Box::pin(stream),
        }
    }
}

impl Stream for GeminiStream {
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.project();
        this.inner.poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::providers::base::sse::GeminiTransformer;
    use crate::core::providers::base::sse::UnifiedSSEParser;

    #[test]
    fn test_sse_parsing() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "Hello"}]}, "finishReason": null}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].choices[0].delta.content.as_ref().unwrap(),
            "Hello"
        );
    }

    #[test]
    fn test_done_parsing() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = b"data: [DONE]\n\n";
        let result = parser.process_bytes(data).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_chunk_with_finish_reason_stop() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "Done."}]}, "finishReason": "STOP"}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].choices[0].finish_reason,
            Some(crate::core::types::responses::FinishReason::Stop)
        );
    }

    #[test]
    fn test_chunk_with_finish_reason_max_tokens() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "..."}]}, "finishReason": "MAX_TOKENS"}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(
            result[0].choices[0].finish_reason,
            Some(crate::core::types::responses::FinishReason::Length)
        );
    }

    #[test]
    fn test_chunk_with_finish_reason_safety() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "..."}]}, "finishReason": "SAFETY"}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(
            result[0].choices[0].finish_reason,
            Some(crate::core::types::responses::FinishReason::ContentFilter)
        );
    }

    #[test]
    fn test_chunk_with_usage() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "Hello"}]}}], "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15}}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert!(result[0].usage.is_some());
        let usage = result[0].usage.as_ref().unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn test_multiple_parts() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "Hello"}, {"text": " world"}]}}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(
            result[0].choices[0].delta.content.as_ref().unwrap(),
            "Hello world"
        );
    }

    #[test]
    fn test_multiple_candidates() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "Response 1"}]}}, {"content": {"parts": [{"text": "Response 2"}]}}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result[0].choices.len(), 2);
        assert_eq!(
            result[0].choices[0].delta.content.as_ref().unwrap(),
            "Response 1"
        );
        assert_eq!(
            result[0].choices[1].delta.content.as_ref().unwrap(),
            "Response 2"
        );
    }

    #[test]
    fn test_empty_candidates() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": []}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_error_response() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"error": {"code": 400, "message": "Bad request"}}

"#;
        let result = parser.process_bytes(data);
        assert!(result.is_err());
    }
}
