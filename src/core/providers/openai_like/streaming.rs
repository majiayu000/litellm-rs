//! OpenAI-Like Streaming Response Handler
//!
//! Uses the unified SSE parser for consistent streaming.

use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEStream};

/// OpenAI-like uses OpenAI-compatible SSE format
pub type OpenAILikeStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    OpenAICompatibleTransformer,
>;

/// Helper function to create OpenAI-like stream
pub fn create_openai_like_stream(
    stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> OpenAILikeStream {
    let transformer = OpenAICompatibleTransformer::new("openai_like");
    UnifiedSSEStream::new(Box::pin(stream), transformer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::base::sse::UnifiedSSEParser;
    use futures::StreamExt;

    #[test]
    fn test_sse_parsing() {
        let transformer = OpenAICompatibleTransformer::new("openai_like");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"custom-model\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n";

        let result = parser.process_bytes(test_data);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].id, "chatcmpl-123");
        assert_eq!(chunks[0].model, "custom-model");
    }

    #[test]
    fn test_done_message() {
        let transformer = OpenAICompatibleTransformer::new("openai_like");
        let mut parser = UnifiedSSEParser::new(transformer);

        let done_data = b"data: [DONE]\n\n";
        let result = parser.process_bytes(done_data);

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_stream_wrapper() {
        use futures::stream;

        let data = vec![
            Ok(Bytes::from(
                "data: {\"id\":\"test\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"local-model\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"},\"finish_reason\":\"stop\"}]}\n\n",
            )),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];

        let mock_stream = stream::iter(data);
        let mut openai_like_stream = create_openai_like_stream(mock_stream);

        let first_chunk = openai_like_stream.next().await;
        assert!(first_chunk.is_some());

        if let Some(Ok(chunk)) = first_chunk {
            assert_eq!(chunk.id, "test");
            assert_eq!(chunk.choices[0].delta.content, Some("Hi".to_string()));
        }

        let second_chunk = openai_like_stream.next().await;
        assert!(second_chunk.is_none());
    }
}
