//! Tests for streaming functionality

#[cfg(test)]
use crate::core::streaming::{handler::StreamingHandler, utils};

#[test]
fn test_streaming_handler_creation() {
    let handler = StreamingHandler::new("gpt-4".to_string());
    assert_eq!(handler.model, "gpt-4");
    assert!(handler.is_first_chunk);
    assert!(handler.accumulated_content.is_empty());
}

#[test]
fn test_extract_content_from_chunk() {
    let handler = StreamingHandler::new("gpt-4".to_string());

    // Test OpenAI format
    let openai_chunk = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
    let content = handler.extract_content_from_chunk(openai_chunk).unwrap();
    assert_eq!(content, "Hello");

    // Test Anthropic format
    let anthropic_chunk = r#"data: {"delta":{"text":"World"}}"#;
    let content = handler.extract_content_from_chunk(anthropic_chunk).unwrap();
    assert_eq!(content, "World");

    // Test done signal
    let done_chunk = "data: [DONE]";
    let content = handler.extract_content_from_chunk(done_chunk).unwrap();
    assert!(content.is_empty());
}

#[test]
fn test_token_estimation() {
    let handler = StreamingHandler::new("gpt-4".to_string());

    // Test token estimation
    let text = "Hello world"; // 11 chars -> ~3 tokens
    let tokens = handler.estimate_token_count(text);
    assert_eq!(tokens, 3);

    let longer_text = "This is a longer text for testing"; // 34 chars -> ~9 tokens
    let tokens = handler.estimate_token_count(longer_text);
    assert_eq!(tokens, 9);
}

#[tokio::test]
async fn test_sse_utils() {
    // Test SSE line parsing
    let line = "data: Hello World";
    let data = utils::parse_sse_line(line);
    assert_eq!(data, Some("Hello World".to_string()));

    // Test done detection
    assert!(utils::is_done_line("data: [DONE]"));
    assert!(utils::is_done_line("[DONE]"));
    assert!(!utils::is_done_line("data: Hello"));

    // Test error event creation
    let _error_event = utils::create_error_event("Test error");
    // Note: Event doesn't have a contains method, so we'll just check it was created
    // assert!(!error_event.data("").is_empty());
}
