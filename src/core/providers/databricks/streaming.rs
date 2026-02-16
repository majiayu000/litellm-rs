//! Databricks Streaming Support
//!
//! Uses the unified SSE parser with DatabricksTransformer for array content support.

use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

use crate::core::providers::base::sse::{DatabricksTransformer, UnifiedSSEStream};

/// Databricks SSE stream type
pub type DatabricksStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    DatabricksTransformer,
>;

/// Create a streaming response from Databricks SSE stream
pub fn create_databricks_stream(
    stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> DatabricksStream {
    let transformer = DatabricksTransformer;
    UnifiedSSEStream::new(Box::pin(stream), transformer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::base::sse::UnifiedSSEParser;
    use crate::core::types::message::MessageRole;
    use crate::core::types::responses::FinishReason;
    use futures::StreamExt;

    #[test]
    fn test_parse_sse_done() {
        let transformer = DatabricksTransformer;
        let mut parser = UnifiedSSEParser::new(transformer);

        let result = parser.process_bytes(b"data: [DONE]\n\n").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_sse_valid() {
        let transformer = DatabricksTransformer;
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = b"data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"dbrx\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n";
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "chatcmpl-123");
        assert_eq!(
            result[0].choices[0].delta.content,
            Some("Hello".to_string())
        );
    }

    #[test]
    fn test_parse_with_role() {
        let transformer = DatabricksTransformer;
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = b"data: {\"id\":\"test\",\"created\":0,\"model\":\"test\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n";
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(
            result[0].choices[0].delta.role,
            Some(MessageRole::Assistant)
        );
        assert_eq!(result[0].choices[0].delta.content, Some("Hi".to_string()));
    }

    #[test]
    fn test_parse_with_finish_reason() {
        let transformer = DatabricksTransformer;
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = b"data: {\"id\":\"test\",\"created\":0,\"model\":\"test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n";
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result[0].choices[0].finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn test_parse_with_array_content() {
        let transformer = DatabricksTransformer;
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = b"data: {\"id\":\"chunk-123\",\"created\":1700000000,\"model\":\"claude-3-opus\",\"choices\":[{\"index\":0,\"delta\":{\"content\":[{\"type\":\"text\",\"text\":\"Hello \"},{\"type\":\"text\",\"text\":\"world\"}]}}]}\n\n";
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(
            result[0].choices[0].delta.content,
            Some("Hello world".to_string())
        );
    }

    #[tokio::test]
    async fn test_databricks_stream() {
        use futures::stream;

        let test_data = vec![
            Ok(Bytes::from(
                "data: {\"id\":\"test-1\",\"created\":0,\"model\":\"dbrx\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            )),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];

        let mock_stream = stream::iter(test_data);
        let mut stream = create_databricks_stream(mock_stream);

        let chunk = stream.next().await.unwrap().unwrap();
        assert_eq!(chunk.choices[0].delta.content.as_ref().unwrap(), "Hello");

        assert!(stream.next().await.is_none());
    }
}
