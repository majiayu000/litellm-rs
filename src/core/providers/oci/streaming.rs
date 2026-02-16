//! Streaming Module for OCI Generative AI
//!
//! Uses the unified SSE parser — OCI uses OpenAI-compatible SSE format.

use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEStream};

/// OCI uses OpenAI-compatible SSE format
pub type OciStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    OpenAICompatibleTransformer,
>;

/// Create an OCI streaming response
pub fn create_oci_stream(
    stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> OciStream {
    let transformer = OpenAICompatibleTransformer::new("oci");
    UnifiedSSEStream::new(Box::pin(stream), transformer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::base::sse::UnifiedSSEParser;
    use crate::core::types::message::MessageRole;
    use futures::StreamExt;

    #[test]
    fn test_sse_parsing() {
        let transformer = OpenAICompatibleTransformer::new("oci");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"test-1\",\"model\":\"cohere.command-r-plus\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n";
        let result = parser.process_bytes(test_data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].choices[0].delta.content,
            Some("Hello".to_string())
        );
    }

    #[test]
    fn test_done_message() {
        let transformer = OpenAICompatibleTransformer::new("oci");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: [DONE]\n\n";
        let result = parser.process_bytes(test_data).unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_oci_stream_basic() {
        use futures::stream;

        let test_data = vec![
            Ok(Bytes::from(
                "data: {\"id\":\"test-1\",\"model\":\"cohere.command-r-plus\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            )),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];

        let mock_stream = stream::iter(test_data);
        let mut oci_stream = create_oci_stream(mock_stream);

        let chunk1 = oci_stream.next().await;
        assert!(chunk1.is_some());
        let chunk1 = chunk1.unwrap().unwrap();
        assert_eq!(chunk1.choices[0].delta.content.as_ref().unwrap(), "Hello");

        let end = oci_stream.next().await;
        assert!(end.is_none());
    }

    #[tokio::test]
    async fn test_oci_stream_multiple_chunks() {
        use futures::stream;

        let test_data = vec![
            Ok(Bytes::from(
                "data: {\"id\":\"test-1\",\"model\":\"cohere.command-r-plus\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            )),
            Ok(Bytes::from(
                "data: {\"id\":\"test-1\",\"model\":\"cohere.command-r-plus\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            )),
            Ok(Bytes::from(
                "data: {\"id\":\"test-1\",\"model\":\"cohere.command-r-plus\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" World\"}}]}\n\n",
            )),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];

        let mock_stream = stream::iter(test_data);
        let mut oci_stream = create_oci_stream(mock_stream);

        // Role chunk
        let chunk1 = oci_stream.next().await.unwrap().unwrap();
        assert_eq!(chunk1.choices[0].delta.role, Some(MessageRole::Assistant));

        // Content chunks
        let chunk2 = oci_stream.next().await.unwrap().unwrap();
        assert_eq!(chunk2.choices[0].delta.content.as_ref().unwrap(), "Hello");

        let chunk3 = oci_stream.next().await.unwrap().unwrap();
        assert_eq!(chunk3.choices[0].delta.content.as_ref().unwrap(), " World");

        // Done
        assert!(oci_stream.next().await.is_none());
    }
}
