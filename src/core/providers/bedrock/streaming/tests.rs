use super::*;
use crate::core::providers::bedrock::model_config::{BedrockApiType, BedrockModelFamily};

// ==================== HeaderValue Tests ====================

#[test]
fn test_header_value_string() {
    let value = HeaderValue::String("test".to_string());
    assert!(matches!(value, HeaderValue::String(_)));
}

#[test]
fn test_header_value_byte_array() {
    let value = HeaderValue::ByteArray(vec![1, 2, 3]);
    assert!(matches!(value, HeaderValue::ByteArray(_)));
}

#[test]
fn test_header_value_boolean() {
    let value = HeaderValue::Boolean(true);
    assert!(matches!(value, HeaderValue::Boolean(true)));
}

#[test]
fn test_header_value_numeric_types() {
    let _ = HeaderValue::Byte(1);
    let _ = HeaderValue::Short(256);
    let _ = HeaderValue::Integer(65536);
    let _ = HeaderValue::Long(1_000_000_000);
    let _ = HeaderValue::Timestamp(1234567890);
}

#[test]
fn test_header_value_uuid() {
    let value = HeaderValue::UUID("550e8400-e29b-41d4-a716-446655440000".to_string());
    assert!(matches!(value, HeaderValue::UUID(_)));
}

// ==================== EventStreamHeader Tests ====================

#[test]
fn test_event_stream_header() {
    let header = EventStreamHeader {
        name: ":message-type".to_string(),
        value: HeaderValue::String("event".to_string()),
    };
    assert_eq!(header.name, ":message-type");
}

// ==================== EventStreamMessage Tests ====================

#[test]
fn test_event_stream_message() {
    let message = EventStreamMessage {
        headers: vec![EventStreamHeader {
            name: ":event-type".to_string(),
            value: HeaderValue::String("chunk".to_string()),
        }],
        payload: Bytes::from(r#"{"text": "hello"}"#),
    };
    assert_eq!(message.headers.len(), 1);
    assert!(!message.payload.is_empty());
}

// ==================== parse_event_message Tests ====================

#[test]
fn test_parse_event_message_too_short() {
    let data = vec![0, 0, 0, 0, 0, 0, 0, 0]; // Only 8 bytes
    let result = BedrockStream::parse_event_message(&data);
    assert!(result.is_err());
}

#[test]
fn test_parse_event_message_incomplete() {
    // total_length says 100 but we only have 20 bytes
    let mut data = vec![0u8; 20];
    data[0..4].copy_from_slice(&100u32.to_be_bytes()); // total_length = 100
    data[4..8].copy_from_slice(&0u32.to_be_bytes()); // headers_length = 0

    let result = BedrockStream::parse_event_message(&data);
    assert!(result.is_err());
}

#[test]
fn test_parse_event_message_minimal() {
    // Minimum valid message:
    // - 4 bytes: total_length (16 bytes min for prelude + 4 for CRC = 20 if no headers/payload)
    // - 4 bytes: headers_length
    // - 4 bytes: prelude CRC
    // - (headers if any)
    // - (payload if any)
    // - 4 bytes: message CRC
    //
    // For a minimal message with no headers and no payload:
    // total_length = 12 (prelude) + 4 (message CRC) = 16
    let total_length: u32 = 16;
    let headers_length: u32 = 0;
    let prelude_crc: u32 = 0;
    let message_crc: u32 = 0;

    let mut data = Vec::new();
    data.extend_from_slice(&total_length.to_be_bytes());
    data.extend_from_slice(&headers_length.to_be_bytes());
    data.extend_from_slice(&prelude_crc.to_be_bytes());
    data.extend_from_slice(&message_crc.to_be_bytes());

    let result = BedrockStream::parse_event_message(&data);
    assert!(result.is_ok());

    let message = result.unwrap();
    assert!(message.headers.is_empty());
    // Payload is from headers_end (12 + 0 = 12) to total_length - 4 (16 - 4 = 12)
    // So payload start == payload end, meaning empty payload
    assert!(message.payload.is_empty());
}

// ==================== Claude Chunk Parsing Tests ====================

fn create_test_stream_claude() -> BedrockStream {
    let stream = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
    BedrockStream::new(
        stream,
        BedrockModelFamily::Claude,
        BedrockApiType::InvokeStream,
    )
}

fn create_test_stream_converse_claude() -> BedrockStream {
    let stream = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
    BedrockStream::new(stream, BedrockModelFamily::Claude, BedrockApiType::Converse)
}

#[test]
fn test_parse_claude_content_block_delta() {
    let stream = create_test_stream_claude();
    let json = serde_json::json!({
        "type": "content_block_delta",
        "delta": {
            "text": "Hello, world!"
        }
    });

    let result = stream.parse_claude_chunk(&json);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());

    let chunk = chunk.unwrap();
    assert_eq!(chunk.choices.len(), 1);
    assert_eq!(
        chunk.choices[0].delta.content,
        Some("Hello, world!".to_string())
    );
}

#[test]
fn test_parse_claude_message_stop() {
    let stream = create_test_stream_claude();
    let json = serde_json::json!({
        "type": "message_stop"
    });

    let result = stream.parse_claude_chunk(&json);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());

    let chunk = chunk.unwrap();
    assert!(chunk.choices[0].finish_reason.is_some());
}

#[test]
fn test_parse_claude_unknown_event() {
    let stream = create_test_stream_claude();
    let json = serde_json::json!({
        "type": "message_start"
    });

    let result = stream.parse_claude_chunk(&json);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_parse_claude_empty_delta() {
    let stream = create_test_stream_claude();
    let json = serde_json::json!({
        "type": "content_block_delta",
        "delta": {}
    });

    let result = stream.parse_claude_chunk(&json);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap().choices[0].delta.content,
        Some("".to_string())
    );
}

// ==================== Nova Chunk Parsing Tests ====================

fn create_test_stream_nova() -> BedrockStream {
    let stream = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
    BedrockStream::new(
        stream,
        BedrockModelFamily::Nova,
        BedrockApiType::InvokeStream,
    )
}

#[test]
fn test_parse_nova_content_block_delta() {
    let stream = create_test_stream_nova();
    let json = serde_json::json!({
        "contentBlockDelta": {
            "delta": {
                "text": "Nova response"
            }
        }
    });

    let result = stream.parse_nova_chunk(&json);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap().choices[0].delta.content,
        Some("Nova response".to_string())
    );
}

#[test]
fn test_parse_nova_no_content() {
    let stream = create_test_stream_nova();
    let json = serde_json::json!({
        "messageStart": {}
    });

    let result = stream.parse_nova_chunk(&json);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ==================== Titan Chunk Parsing Tests ====================

fn create_test_stream_titan() -> BedrockStream {
    let stream = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
    BedrockStream::new(
        stream,
        BedrockModelFamily::TitanText,
        BedrockApiType::InvokeStream,
    )
}

#[test]
fn test_parse_titan_output_text() {
    let stream = create_test_stream_titan();
    let json = serde_json::json!({
        "outputText": "Titan response"
    });

    let result = stream.parse_titan_chunk(&json);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap().choices[0].delta.content,
        Some("Titan response".to_string())
    );
}

#[test]
fn test_parse_titan_with_completion_reason() {
    let stream = create_test_stream_titan();
    let json = serde_json::json!({
        "outputText": "Final text",
        "completionReason": "FINISH"
    });

    let result = stream.parse_titan_chunk(&json);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    assert!(chunk.unwrap().choices[0].finish_reason.is_some());
}

#[test]
fn test_parse_titan_no_output() {
    let stream = create_test_stream_titan();
    let json = serde_json::json!({
        "usage": {
            "inputTokens": 10
        }
    });

    let result = stream.parse_titan_chunk(&json);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ==================== Generic Chunk Parsing Tests ====================

fn create_test_stream_generic() -> BedrockStream {
    let stream = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
    BedrockStream::new(
        stream,
        BedrockModelFamily::Mistral,
        BedrockApiType::InvokeStream,
    )
}

#[test]
fn test_parse_generic_completion() {
    let stream = create_test_stream_generic();
    let json = serde_json::json!({
        "completion": "Generic completion"
    });

    let result = stream.parse_generic_chunk(&json);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap().choices[0].delta.content,
        Some("Generic completion".to_string())
    );
}

#[test]
fn test_parse_generic_generation() {
    let stream = create_test_stream_generic();
    let json = serde_json::json!({
        "generation": "Generated text"
    });

    let result = stream.parse_generic_chunk(&json);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap().choices[0].delta.content,
        Some("Generated text".to_string())
    );
}

#[test]
fn test_parse_generic_text() {
    let stream = create_test_stream_generic();
    let json = serde_json::json!({
        "text": "Simple text"
    });

    let result = stream.parse_generic_chunk(&json);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap().choices[0].delta.content,
        Some("Simple text".to_string())
    );
}

#[test]
fn test_parse_generic_openai_compatible_delta() {
    let stream = create_test_stream_generic();
    let json = serde_json::json!({
        "choices": [{
            "delta": {
                "content": "OpenAI delta"
            }
        }]
    });

    let result = stream.parse_generic_chunk(&json);
    assert!(result.is_ok());

    let chunk = result.unwrap_or_else(|err| panic!("OpenAI-compatible chunk should parse: {err}"));
    assert!(chunk.is_some());
    let chunk = chunk.unwrap_or_else(|| panic!("OpenAI-compatible chunk should emit content"));
    assert_eq!(
        chunk.choices[0].delta.content,
        Some("OpenAI delta".to_string())
    );
}

#[test]
fn test_parse_generic_no_content() {
    let stream = create_test_stream_generic();
    let json = serde_json::json!({
        "metadata": {}
    });

    let result = stream.parse_generic_chunk(&json);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ==================== parse_chunk Routing Tests ====================

#[test]
fn test_parse_chunk_routes_to_claude() {
    let stream = create_test_stream_claude();
    let payload = br#"{"type": "content_block_delta", "delta": {"text": "test"}}"#;

    let result = stream.parse_chunk(payload);
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[test]
fn test_parse_chunk_routes_converse_claude_to_converse_schema() {
    let stream = create_test_stream_converse_claude();
    let payload = br#"{"contentBlockDelta": {"delta": {"text": "test"}}}"#;

    let result = stream.parse_chunk(payload);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap().choices[0].delta.content,
        Some("test".to_string())
    );
}

#[test]
fn test_parse_converse_tool_use_start_emits_tool_call_delta() {
    let stream = create_test_stream_converse_claude();
    let payload = br#"{"contentBlockStart":{"start":{"toolUse":{"toolUseId":"tool-123","name":"lookup_weather"}},"contentBlockIndex":1}}"#;

    let result = stream.parse_chunk(payload);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    let chunk = chunk.unwrap();
    let tool_calls = chunk.choices[0].delta.tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls.len(), 1);

    let tool_call = &tool_calls[0];
    assert_eq!(tool_call.index, 1);
    assert_eq!(tool_call.id.as_deref(), Some("tool-123"));
    assert_eq!(tool_call.tool_type.as_deref(), Some("function"));

    let function = tool_call.function.as_ref().unwrap();
    assert_eq!(function.name.as_deref(), Some("lookup_weather"));
    assert_eq!(function.arguments, None);
    assert_eq!(chunk.choices[0].delta.content, None);
}

#[test]
fn test_parse_converse_tool_use_delta_emits_arguments_delta() {
    let stream = create_test_stream_converse_claude();
    let payload = br#"{"contentBlockDelta":{"delta":{"toolUse":{"input":"{\"city\":\"San Francisco\"}"}},"contentBlockIndex":1}}"#;

    let result = stream.parse_chunk(payload);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    let chunk = chunk.unwrap();
    let tool_calls = chunk.choices[0].delta.tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls.len(), 1);

    let tool_call = &tool_calls[0];
    assert_eq!(tool_call.index, 1);
    assert_eq!(tool_call.id, None);
    assert_eq!(tool_call.tool_type, None);

    let function = tool_call.function.as_ref().unwrap();
    assert_eq!(function.name, None);
    assert_eq!(
        function.arguments.as_deref(),
        Some(r#"{"city":"San Francisco"}"#)
    );
    assert_eq!(chunk.choices[0].delta.content, None);
}

#[test]
fn test_parse_converse_message_stop_maps_stop_reason() {
    let stream = create_test_stream_converse_claude();
    let payload = br#"{"messageStop": {"stopReason": "tool_use"}}"#;

    let result = stream.parse_chunk(payload);
    assert!(result.is_ok());

    let chunk = result.unwrap();
    assert!(chunk.is_some());
    let chunk = chunk.unwrap();
    assert_eq!(
        chunk.choices[0].finish_reason.as_ref(),
        Some(&crate::core::types::responses::FinishReason::ToolCalls)
    );
}

#[test]
fn test_parse_chunk_routes_to_nova() {
    let stream = create_test_stream_nova();
    let payload = br#"{"contentBlockDelta": {"delta": {"text": "test"}}}"#;

    let result = stream.parse_chunk(payload);
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[test]
fn test_parse_chunk_routes_to_titan() {
    let stream = create_test_stream_titan();
    let payload = br#"{"outputText": "test"}"#;

    let result = stream.parse_chunk(payload);
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[test]
fn test_parse_chunk_invalid_json() {
    let stream = create_test_stream_claude();
    let payload = b"not valid json";

    let result = stream.parse_chunk(payload);
    assert!(result.is_err());
}

// ==================== BedrockStream Creation Tests ====================

#[test]
fn test_bedrock_stream_creation() {
    let stream = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
    let bedrock_stream = BedrockStream::new(
        stream,
        BedrockModelFamily::Claude,
        BedrockApiType::InvokeStream,
    );
    assert!(bedrock_stream.buffer.is_empty());
}

#[test]
fn test_bedrock_stream_different_models() {
    let stream1 = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
    let _ = BedrockStream::new(
        stream1,
        BedrockModelFamily::Claude,
        BedrockApiType::InvokeStream,
    );

    let stream2 = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
    let _ = BedrockStream::new(
        stream2,
        BedrockModelFamily::Nova,
        BedrockApiType::ConverseStream,
    );

    let stream3 = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
    let _ = BedrockStream::new(
        stream3,
        BedrockModelFamily::TitanText,
        BedrockApiType::InvokeStream,
    );

    let stream4 = futures::stream::empty::<Result<Bytes, reqwest::Error>>();
    let _ = BedrockStream::new(
        stream4,
        BedrockModelFamily::Mistral,
        BedrockApiType::InvokeStream,
    );
}
