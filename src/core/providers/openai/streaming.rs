//! OpenAI Streaming Response Handler
//!
//! Uses the unified SSE parser for consistent streaming across providers.

use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEStream};

/// OpenAI uses OpenAI-compatible SSE format (naturally)
pub type OpenAIStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    OpenAICompatibleTransformer,
>;

/// Helper function to create OpenAI stream
pub fn create_openai_stream(
    stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> OpenAIStream {
    let transformer = OpenAICompatibleTransformer::new("openai");
    UnifiedSSEStream::new(Box::pin(stream), transformer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::base::sse::UnifiedSSEParser;
    use futures::StreamExt;

    // ==================== SSE Parsing Tests ====================

    #[test]
    fn test_sse_parsing() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n";

        let result = parser.process_bytes(test_data);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].id, "chatcmpl-123");
        assert_eq!(chunks[0].model, "gpt-4");
        assert_eq!(chunks[0].choices.len(), 1);
        assert_eq!(
            chunks[0].choices[0].delta.content,
            Some("Hello".to_string())
        );
    }

    #[test]
    fn test_sse_parsing_with_role() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"chatcmpl-456\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n";

        let result = parser.process_bytes(test_data);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].choices[0].delta.role.is_some());
    }

    #[test]
    fn test_sse_parsing_with_finish_reason() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"chatcmpl-789\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n";

        let result = parser.process_bytes(test_data);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].choices[0].finish_reason.is_some());
    }

    #[test]
    fn test_sse_parsing_multiple_chunks() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"A\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"created\":124,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"B\"},\"finish_reason\":null}]}\n\n";

        let result = parser.process_bytes(test_data);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].choices[0].delta.content, Some("A".to_string()));
        assert_eq!(chunks[1].choices[0].delta.content, Some("B".to_string()));
    }

    #[test]
    fn test_sse_parsing_empty_content() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"chatcmpl-empty\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\"},\"finish_reason\":null}]}\n\n";

        let result = parser.process_bytes(test_data);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].choices[0].delta.content, Some("".to_string()));
    }

    // ==================== Done Message Tests ====================

    #[test]
    fn test_done_message() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let done_data = b"data: [DONE]\n\n";
        let result = parser.process_bytes(done_data);

        assert!(result.is_ok());
        // [DONE] should not produce any chunks
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_done_message_after_data() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n";

        let result = parser.process_bytes(test_data);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Should only produce one chunk (the data), not the [DONE]
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].choices[0].delta.content, Some("Hi".to_string()));
    }

    // ==================== Incremental Parsing Tests ====================

    #[test]
    fn test_incremental_parsing() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        // Send data in parts
        let part1 = b"data: {\"id\":\"test\",\"object\":\"chat.completion.chunk\"";
        let part2 = b",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n";

        // First part should not produce a chunk
        let result1 = parser.process_bytes(part1);
        assert!(result1.is_ok());
        assert!(result1.unwrap().is_empty());

        // Second part should complete the chunk
        let result2 = parser.process_bytes(part2);
        assert!(result2.is_ok());

        let chunks = result2.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].id, "test");
        assert_eq!(chunks[0].choices[0].delta.content, Some("Hi".to_string()));
    }

    #[test]
    fn test_incremental_parsing_three_parts() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let part1 = b"data: {\"id\":\"inc\",";
        let part2 = b"\"object\":\"chat.completion.chunk\",\"created\":123,";
        let part3 = b"\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"X\"},\"finish_reason\":null}]}\n\n";

        let result1 = parser.process_bytes(part1);
        assert!(result1.is_ok());
        assert!(result1.unwrap().is_empty());

        let result2 = parser.process_bytes(part2);
        assert!(result2.is_ok());
        assert!(result2.unwrap().is_empty());

        let result3 = parser.process_bytes(part3);
        assert!(result3.is_ok());

        let chunks = result3.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].id, "inc");
    }

    #[test]
    fn test_incremental_parsing_newline_split() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        // Split right at the newline boundary
        let part1 = b"data: {\"id\":\"nl\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Y\"},\"finish_reason\":null}]}\n";
        let part2 = b"\n";

        let result1 = parser.process_bytes(part1);
        assert!(result1.is_ok());
        // May or may not produce a chunk depending on implementation

        let result2 = parser.process_bytes(part2);
        assert!(result2.is_ok());
    }

    // ==================== Transformer Tests ====================

    #[test]
    fn test_openai_compatible_transformer_creation() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        // Should not panic
        let _ = transformer;
    }

    #[test]
    fn test_openai_compatible_transformer_different_provider() {
        let transformer = OpenAICompatibleTransformer::new("azure");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"azure-123\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Test\"},\"finish_reason\":null}]}\n\n";

        let result = parser.process_bytes(test_data);
        assert!(result.is_ok());
    }

    // ==================== Stream Wrapper Tests ====================

    #[tokio::test]
    async fn test_stream_wrapper() {
        use futures::stream;

        // Create a mock byte stream
        let data = vec![
            Ok(Bytes::from(
                "data: {\"id\":\"test\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":\"stop\"}]}\n\n",
            )),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];

        let mock_stream = stream::iter(data);
        let mut openai_stream = create_openai_stream(mock_stream);

        // Should produce one chunk
        let first_chunk = openai_stream.next().await;
        assert!(first_chunk.is_some());

        if let Some(Ok(chunk)) = first_chunk {
            assert_eq!(chunk.id, "test");
            assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
        }

        // Stream should end after [DONE]
        let second_chunk = openai_stream.next().await;
        assert!(second_chunk.is_none());
    }

    #[tokio::test]
    async fn test_stream_wrapper_multiple_chunks() {
        use futures::stream;

        let data = vec![
            Ok(Bytes::from(
                "data: {\"id\":\"chunk1\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"A\"},\"finish_reason\":null}]}\n\n",
            )),
            Ok(Bytes::from(
                "data: {\"id\":\"chunk2\",\"object\":\"chat.completion.chunk\",\"created\":124,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"B\"},\"finish_reason\":\"stop\"}]}\n\n",
            )),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];

        let mock_stream = stream::iter(data);
        let mut openai_stream = create_openai_stream(mock_stream);

        let chunk1 = openai_stream.next().await;
        assert!(chunk1.is_some());
        if let Some(Ok(c)) = chunk1 {
            assert_eq!(c.id, "chunk1");
        }

        let chunk2 = openai_stream.next().await;
        assert!(chunk2.is_some());
        if let Some(Ok(c)) = chunk2 {
            assert_eq!(c.id, "chunk2");
        }

        let done = openai_stream.next().await;
        assert!(done.is_none());
    }

    #[tokio::test]
    async fn test_stream_wrapper_empty() {
        use futures::stream;

        let data: Vec<Result<Bytes, reqwest::Error>> = vec![];

        let mock_stream = stream::iter(data);
        let mut openai_stream = create_openai_stream(mock_stream);

        let result = openai_stream.next().await;
        assert!(result.unwrap().is_err());
        assert!(openai_stream.next().await.is_none());
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_empty_bytes() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let result = parser.process_bytes(b"");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let result = parser.process_bytes(b"   \n\n   ");
        assert!(result.is_ok());
    }

    #[test]
    fn test_newlines_only() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let result = parser.process_bytes(b"\n\n\n\n");
        assert!(result.is_ok());
    }

    #[test]
    fn test_unicode_content() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"unicode\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\xe4\xbd\xa0\xe5\xa5\xbd\"},\"finish_reason\":null}]}\n\n";

        let result = parser.process_bytes(test_data);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].choices[0].delta.content, Some("你好".to_string()));
    }

    #[test]
    fn test_special_characters_content() {
        let transformer = OpenAICompatibleTransformer::new("openai");
        let mut parser = UnifiedSSEParser::new(transformer);

        let test_data = b"data: {\"id\":\"special\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\\nWorld\\t!\"},\"finish_reason\":null}]}\n\n";

        let result = parser.process_bytes(test_data);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0].choices[0].delta.content,
            Some("Hello\nWorld\t!".to_string())
        );
    }
}
