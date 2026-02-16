//! Cohere Streaming Handler
//!
//! Uses the unified SSE parser with CohereTransformer for v1/v2 streaming formats.

use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

use crate::core::providers::base::sse::{CohereTransformer, UnifiedSSEStream};

/// Cohere SSE stream type
pub type CohereStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    CohereTransformer,
>;

/// Create a Cohere streaming response
pub fn create_cohere_stream(
    stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
    model: &str,
    use_v2: bool,
) -> CohereStream {
    let transformer = CohereTransformer::new(model, use_v2);
    UnifiedSSEStream::new(Box::pin(stream), transformer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::base::sse::UnifiedSSEParser;
    use crate::core::types::responses::FinishReason;
    use futures::StreamExt;

    #[test]
    fn test_parse_v1_text_generation() {
        let transformer = CohereTransformer::new("command-r-plus", false);
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = b"data: {\"type\": \"text-generation\", \"text\": \"Hello, \"}\n\n";
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].choices[0].delta.content,
            Some("Hello, ".to_string())
        );
    }

    #[test]
    fn test_parse_v2_content_delta() {
        let transformer = CohereTransformer::new("command-r-plus", true);
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = b"data: {\"type\": \"content-delta\", \"delta\": {\"message\": {\"content\": {\"text\": \"World!\"}}}}\n\n";
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].choices[0].delta.content,
            Some("World!".to_string())
        );
    }

    #[test]
    fn test_parse_empty_and_done() {
        let transformer = CohereTransformer::new("command-r-plus", true);
        let mut parser = UnifiedSSEParser::new(transformer);

        let result = parser.process_bytes(b"data: [DONE]\n\n").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_v1_stream_end() {
        let transformer = CohereTransformer::new("command-r-plus", false);
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = b"data: {\"type\": \"stream-end\", \"finish_reason\": \"stop\"}\n\n";
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].choices[0].finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn test_v2_message_end_with_usage() {
        let transformer = CohereTransformer::new("command-r-plus", true);
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = b"data: {\"type\": \"message-end\", \"data\": {\"delta\": {\"finish_reason\": \"stop\", \"usage\": {\"tokens\": {\"input_tokens\": 10, \"output_tokens\": 20}}}}}\n\n";
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].choices[0].finish_reason, Some(FinishReason::Stop));
        assert!(result[0].usage.is_some());
        let usage = result[0].usage.as_ref().unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[tokio::test]
    async fn test_cohere_stream() {
        use futures::stream;

        let test_data = vec![
            Ok(Bytes::from(
                "data: {\"type\": \"content-delta\", \"delta\": {\"message\": {\"content\": {\"text\": \"Hello\"}}}}\n\n",
            )),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];

        let mock_stream = stream::iter(test_data);
        let mut stream = create_cohere_stream(mock_stream, "command-r-plus", true);

        let chunk = stream.next().await.unwrap().unwrap();
        assert_eq!(chunk.choices[0].delta.content.as_ref().unwrap(), "Hello");

        assert!(stream.next().await.is_none());
    }
}
