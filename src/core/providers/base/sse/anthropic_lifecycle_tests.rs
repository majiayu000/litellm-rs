use std::collections::HashMap;

use bytes::Bytes;
use futures::{StreamExt, stream};

use super::{AnthropicTransformer, SSETransformer};
use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEStream};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::anthropic_continuation::AnthropicThinkingBlock;
use crate::core::types::responses::ChatChunk;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn sse_event(value: serde_json::Value) -> String {
    format!("data: {value}\n\n")
}

fn transform(
    transformer: &AnthropicTransformer,
    event: serde_json::Value,
) -> Result<Option<ChatChunk>, ProviderError> {
    transformer.transform_chunk(&event.to_string())
}

fn assert_lifecycle_error(error: ProviderError, index: u64, detail: &str) {
    assert_lifecycle_error_contexts(error, index, &[detail]);
}

fn assert_lifecycle_error_contexts(error: ProviderError, index: u64, details: &[&str]) {
    match error {
        ProviderError::Streaming {
            provider,
            stream_type,
            position,
            last_chunk,
            message,
        } => {
            assert_eq!(provider, "anthropic");
            assert_eq!(stream_type, "chat.thinking");
            assert_eq!(position, Some(index));
            assert!(last_chunk.is_none());
            for detail in details {
                assert!(message.contains(detail), "unexpected error: {message}");
            }
        }
        error => panic!("unexpected error variant: {error}"),
    }
}

#[tokio::test]
async fn transform_error_is_terminal_and_later_events_cannot_repair_stream() {
    let events = [
        serde_json::json!({"type":"content_block_start","index":3,"content_block":{"type":"thinking","thinking":""}}),
        serde_json::json!({"type":"content_block_delta","index":3,"delta":{"type":"signature_delta","signature":""}}),
        serde_json::json!({"type":"content_block_delta","index":3,"delta":{"type":"signature_delta","signature":"valid"}}),
        serde_json::json!({"type":"content_block_stop","index":3}),
    ]
    .map(|event| Ok::<Bytes, reqwest::Error>(Bytes::from(sse_event(event))));
    let mut output = UnifiedSSEStream::new(stream::iter(events), AnthropicTransformer::new("test"));

    assert!(output.next().await.unwrap().is_ok());
    assert_lifecycle_error(
        output.next().await.unwrap().unwrap_err(),
        3,
        "empty signature",
    );
    assert!(
        output.next().await.is_none(),
        "failed stream must stay terminal"
    );
}

#[tokio::test]
async fn pending_invalid_event_at_eof_preserves_parse_and_lifecycle_errors() {
    let body = format!(
        "{}data: {{",
        sse_event(
            serde_json::json!({"type":"content_block_start","index":4,"content_block":{"type":"thinking","thinking":""}})
        )
    );
    let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, AnthropicTransformer::new("test"));

    assert!(output.next().await.unwrap().is_ok());
    assert_lifecycle_error_contexts(
        output.next().await.unwrap().unwrap_err(),
        4,
        &[
            "ResponseParsing",
            "Failed to parse Anthropic SSE",
            "missing its signature",
        ],
    );
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn pending_invalid_event_at_transport_error_preserves_all_contexts() {
    let prefix = format!(
        "{}data: {{",
        sse_event(
            serde_json::json!({"type":"content_block_start","index":5,"content_block":{"type":"thinking","thinking":""}})
        )
    );
    let response = broken_chunked_response(prefix).await;
    let mut output = UnifiedSSEStream::new(
        Box::pin(response.bytes_stream()),
        AnthropicTransformer::new("test"),
    );

    assert!(output.next().await.unwrap().is_ok());
    assert_lifecycle_error_contexts(
        output.next().await.unwrap().unwrap_err(),
        5,
        &[
            "Stream error",
            "ResponseParsing",
            "Failed to parse Anthropic SSE",
            "missing its signature",
        ],
    );
    assert!(output.next().await.is_none());
}

fn completed_blocks(transformer: &AnthropicTransformer) -> Vec<(u32, AnthropicThinkingBlock)> {
    let state = transformer
        .thinking_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.completed.clone()
}

async fn broken_chunked_response(prefix: String) -> reqwest::Response {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback listener must bind");
    let address = listener.local_addr().expect("listener address");
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request must connect");
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await.expect("request must read");
        let headers = b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n";
        socket.write_all(headers).await.expect("headers must write");
        socket
            .write_all(format!("{:X}\r\n{prefix}\r\n20\r\ntruncated", prefix.len()).as_bytes())
            .await
            .expect("truncated body must write");
    });

    reqwest::get(format!("http://{address}"))
        .await
        .expect("loopback request must receive headers")
}

#[tokio::test]
async fn raw_eof_before_signature_errors_once_then_ends() {
    let body = sse_event(serde_json::json!({
        "type": "content_block_start",
        "index": 3,
        "content_block": {"type": "thinking", "thinking": ""}
    }));
    let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, AnthropicTransformer::new("claude-test"));

    let start = output
        .next()
        .await
        .expect("thinking start chunk")
        .expect("thinking start should be valid");
    assert_eq!(
        start.choices[0]
            .delta
            .thinking
            .as_ref()
            .and_then(|thinking| thinking.is_start),
        Some(true)
    );

    let error = output
        .next()
        .await
        .expect("truncated thinking must emit a terminal error")
        .expect_err("raw EOF before signature must fail");
    assert_lifecycle_error(error, 3, "missing its signature");
    assert!(
        output.next().await.is_none(),
        "error must terminate the stream"
    );
}

#[tokio::test]
async fn raw_eof_after_signature_without_stop_errors() {
    let body = [
        sse_event(serde_json::json!({
            "type":"content_block_start", "index":1,
            "content_block":{"type":"thinking", "thinking":"thought"}
        })),
        sse_event(serde_json::json!({
            "type":"content_block_delta", "index":1,
            "delta":{"type":"signature_delta", "signature":"opaque-signature"}
        })),
    ]
    .concat();
    let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, AnthropicTransformer::new("claude-test"));

    assert!(output.next().await.unwrap().is_ok());
    assert!(output.next().await.unwrap().is_ok());
    let error = output.next().await.unwrap().unwrap_err();
    assert_lifecycle_error(error, 1, "missing content_block_stop");
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn raw_eof_after_redacted_start_without_stop_errors() {
    let body = sse_event(serde_json::json!({
        "type":"content_block_start", "index":12,
        "content_block":{"type":"redacted_thinking", "data":"opaque-redacted"}
    }));
    let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, AnthropicTransformer::new("claude-test"));

    assert!(output.next().await.unwrap().is_ok());
    let error = output.next().await.unwrap().unwrap_err();
    assert_lifecycle_error(error, 12, "missing content_block_stop");
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn raw_eof_after_complete_block_succeeds() {
    let body = [
        sse_event(serde_json::json!({
            "type":"content_block_start", "index":0,
            "content_block":{"type":"thinking", "thinking":"thought"}
        })),
        sse_event(serde_json::json!({
            "type":"content_block_delta", "index":0,
            "delta":{"type":"signature_delta", "signature":"opaque-signature"}
        })),
        sse_event(serde_json::json!({"type":"content_block_stop", "index":0})),
    ]
    .concat();
    let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, AnthropicTransformer::new("claude-test"));

    for _ in 0..3 {
        assert!(output.next().await.unwrap().is_ok());
    }
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn broken_chunked_transport_with_active_block_prefers_lifecycle_error() {
    let response = broken_chunked_response(sse_event(serde_json::json!({
        "type":"content_block_start", "index":11,
        "content_block":{"type":"thinking", "thinking":"secret thought"}
    })))
    .await;
    let mut output = UnifiedSSEStream::new(
        Box::pin(response.bytes_stream()),
        AnthropicTransformer::new("claude-test"),
    );

    assert!(output.next().await.unwrap().is_ok());
    let error = output.next().await.unwrap().unwrap_err();
    assert_lifecycle_error(error, 11, "missing its signature");
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn broken_chunked_transport_after_complete_block_keeps_network_error() {
    let prefix = [
        sse_event(serde_json::json!({
            "type":"content_block_start", "index":0,
            "content_block":{"type":"thinking", "thinking":""}
        })),
        sse_event(serde_json::json!({
            "type":"content_block_delta", "index":0,
            "delta":{"type":"signature_delta", "signature":"opaque"}
        })),
        sse_event(serde_json::json!({"type":"content_block_stop", "index":0})),
    ]
    .concat();
    let response = broken_chunked_response(prefix).await;
    let mut output = UnifiedSSEStream::new(
        Box::pin(response.bytes_stream()),
        AnthropicTransformer::new("claude-test"),
    );

    for _ in 0..3 {
        assert!(output.next().await.unwrap().is_ok());
    }
    assert!(matches!(
        output.next().await.unwrap(),
        Err(ProviderError::Network { .. })
    ));
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn non_anthropic_default_finalizer_behavior_is_unchanged() {
    let body = sse_event(serde_json::json!({
        "id":"chunk-1", "object":"chat.completion.chunk", "created":1,
        "model":"gpt-test", "choices":[{"index":0,"delta":{"content":"ok"}}]
    }));
    let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, OpenAICompatibleTransformer::new("openai"));

    assert_eq!(
        output.next().await.unwrap().unwrap().choices[0]
            .delta
            .content
            .as_deref(),
        Some("ok")
    );
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn non_anthropic_pending_parse_error_remains_the_original_error() {
    let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from("data: {"))]);
    let mut output = UnifiedSSEStream::new(source, OpenAICompatibleTransformer::new("openai"));

    assert!(matches!(
        output.next().await.unwrap(),
        Err(ProviderError::ResponseParsing {
            provider: "openai",
            ..
        })
    ));
    assert!(output.next().await.is_none());
}

#[test]
fn missing_or_empty_signature_fails_closed() {
    let transformer = AnthropicTransformer::new("claude-test");
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_start", "index":4,
            "content_block":{"type":"thinking", "thinking":"thought"}
        }),
    )
    .unwrap();

    let empty = transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_delta", "index":4,
            "delta":{"type":"signature_delta", "signature":""}
        }),
    )
    .unwrap_err();
    assert_lifecycle_error(empty, 4, "empty signature");

    let stop = transform(
        &transformer,
        serde_json::json!({"type":"content_block_stop", "index":4}),
    )
    .unwrap_err();
    assert_lifecycle_error(stop, 4, "missing its signature");
}

#[test]
fn split_deltas_materialize_one_lossless_typed_block() {
    let transformer = AnthropicTransformer::new("claude-test");
    for event in [
        serde_json::json!({
            "type":"content_block_start", "index":7,
            "content_block":{"type":"thinking", "thinking":"first "}
        }),
        serde_json::json!({
            "type":"content_block_delta", "index":7,
            "delta":{"type":"thinking_delta", "thinking":"second"}
        }),
        serde_json::json!({
            "type":"content_block_delta", "index":7,
            "delta":{"type":"signature_delta", "signature":"opaque-"}
        }),
        serde_json::json!({
            "type":"content_block_delta", "index":7,
            "delta":{"type":"signature_delta", "signature":"signature"}
        }),
        serde_json::json!({"type":"content_block_stop", "index":7}),
    ] {
        transform(&transformer, event).unwrap();
    }

    let completed = completed_blocks(&transformer);
    match completed.as_slice() {
        [
            (
                index,
                AnthropicThinkingBlock::Thinking {
                    thinking,
                    signature,
                },
            ),
        ] => {
            assert_eq!(*index, 7);
            assert_eq!(thinking, "first second");
            assert_eq!(signature.expose(), "opaque-signature");
        }
        blocks => panic!("unexpected completed blocks: {blocks:?}"),
    }
}

#[test]
fn omitted_display_signature_only_builds_empty_text_block() {
    let transformer = AnthropicTransformer::new("claude-test");
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_start", "index":6,
            "content_block":{"type":"thinking", "thinking":""}
        }),
    )
    .unwrap();
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_delta", "index":6,
            "delta":{"type":"signature_delta", "signature":"opaque"}
        }),
    )
    .unwrap();
    transform(
        &transformer,
        serde_json::json!({"type":"content_block_stop", "index":6}),
    )
    .unwrap();

    match completed_blocks(&transformer).as_slice() {
        [
            (
                6,
                AnthropicThinkingBlock::Thinking {
                    thinking,
                    signature,
                },
            ),
        ] => {
            assert!(thinking.is_empty());
            assert_eq!(signature.expose(), "opaque");
        }
        blocks => panic!("unexpected completed blocks: {blocks:?}"),
    }
}

#[test]
fn redacted_data_is_lossless_and_empty_data_is_rejected() {
    let transformer = AnthropicTransformer::new("claude-test");
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_start", "index":2,
            "content_block":{"type":"redacted_thinking", "data":"opaque-redacted-data"}
        }),
    )
    .unwrap();
    transform(
        &transformer,
        serde_json::json!({"type":"content_block_stop", "index":2}),
    )
    .unwrap();
    match completed_blocks(&transformer).as_slice() {
        [(index, AnthropicThinkingBlock::RedactedThinking { data })] => {
            assert_eq!(*index, 2);
            assert_eq!(data.expose(), "opaque-redacted-data");
        }
        blocks => panic!("unexpected completed blocks: {blocks:?}"),
    }

    let error = transform(
        &AnthropicTransformer::new("claude-test"),
        serde_json::json!({
            "type":"content_block_start", "index":9,
            "content_block":{"type":"redacted_thinking", "data":""}
        }),
    )
    .unwrap_err();
    assert_lifecycle_error(error, 9, "empty data");
}

#[test]
fn block_indexes_and_kinds_must_match() {
    let transformer = AnthropicTransformer::new("claude-test");
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_start", "index":5,
            "content_block":{"type":"redacted_thinking", "data":"opaque"}
        }),
    )
    .unwrap();

    let wrong_index = transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_delta", "index":6,
            "delta":{"type":"thinking_delta", "thinking":"wrong"}
        }),
    )
    .unwrap_err();
    assert_lifecycle_error(wrong_index, 6, "inactive block");

    let wrong_kind = transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_delta", "index":5,
            "delta":{"type":"signature_delta", "signature":"wrong"}
        }),
    )
    .unwrap_err();
    assert_lifecycle_error(wrong_kind, 5, "redacted_thinking block");
}

#[test]
fn duplicate_start_and_new_message_preserve_active_failure() {
    let transformer = AnthropicTransformer::new("claude-test");
    let start = serde_json::json!({
        "type":"content_block_start", "index":1,
        "content_block":{"type":"thinking", "thinking":"first"}
    });
    transform(&transformer, start.clone()).unwrap();
    let duplicate = transform(&transformer, start).unwrap_err();
    assert_lifecycle_error(duplicate, 1, "duplicate thinking block");

    let boundary = transform(
        &transformer,
        serde_json::json!({"type":"message_start", "message":{"id":"next"}}),
    )
    .unwrap_err();
    assert_lifecycle_error(boundary, 1, "message_start");
}

#[test]
fn message_stop_rejects_active_block_and_accepts_completed_block() {
    let transformer = AnthropicTransformer::new("claude-test");
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_start", "index":8,
            "content_block":{"type":"thinking", "thinking":"", "signature":"sig"}
        }),
    )
    .unwrap();
    let error = transform(&transformer, serde_json::json!({"type":"message_stop"})).unwrap_err();
    assert_lifecycle_error(error, 8, "missing content_block_stop");

    transform(
        &transformer,
        serde_json::json!({"type":"content_block_stop", "index":8}),
    )
    .unwrap();
    assert!(transform(&transformer, serde_json::json!({"type":"message_stop"})).is_ok());
}

#[test]
fn multiple_blocks_keep_upstream_completion_order_and_ignore_tool_blocks() {
    let transformer = AnthropicTransformer::new("claude-test");
    for event in [
        serde_json::json!({
            "type":"content_block_start", "index":2,
            "content_block":{"type":"thinking", "thinking":"two", "signature":"sig-two"}
        }),
        serde_json::json!({
            "type":"content_block_start", "index":3,
            "content_block":{"type":"tool_use", "id":"tool-1", "name":"lookup", "input":{}}
        }),
        serde_json::json!({"type":"content_block_stop", "index":3}),
        serde_json::json!({"type":"content_block_stop", "index":2}),
        serde_json::json!({
            "type":"content_block_start", "index":4,
            "content_block":{"type":"redacted_thinking", "data":"redacted-four"}
        }),
        serde_json::json!({"type":"content_block_stop", "index":4}),
    ] {
        transform(&transformer, event).unwrap();
    }

    let completed = completed_blocks(&transformer);
    assert_eq!(completed.len(), 2);
    assert!(matches!(
        completed[0],
        (2, AnthropicThinkingBlock::Thinking { .. })
    ));
    assert!(matches!(
        completed[1],
        (4, AnthropicThinkingBlock::RedactedThinking { .. })
    ));
}

#[test]
fn normal_text_and_tool_stream_is_semantically_unchanged() {
    let transformer = AnthropicTransformer::new("claude-test");
    let text = transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_delta", "index":0,
            "delta":{"type":"text_delta", "text":"hello"}
        }),
    )
    .unwrap()
    .unwrap();
    assert_eq!(text.choices[0].delta.content.as_deref(), Some("hello"));

    let tool = transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_start", "index":4,
            "content_block":{"type":"tool_use", "id":"tool-4", "name":"lookup", "input":{}}
        }),
    )
    .unwrap()
    .unwrap();
    let call = &tool.choices[0].delta.tool_calls.as_ref().unwrap()[0];
    assert_eq!(call.index, 4);
    assert_eq!(call.id.as_deref(), Some("tool-4"));
    assert_eq!(
        call.function.as_ref().unwrap().name.as_deref(),
        Some("lookup")
    );

    let arguments = transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_delta", "index":4,
            "delta":{"type":"input_json_delta", "partial_json":"{\"city\":\"Paris\"}"}
        }),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        arguments.choices[0].delta.tool_calls.as_ref().unwrap()[0]
            .function
            .as_ref()
            .unwrap()
            .arguments
            .as_deref(),
        Some("{\"city\":\"Paris\"}")
    );

    let stop = transform(
        &transformer,
        serde_json::json!({
            "type":"message_delta", "delta":{"stop_reason":"tool_use"},
            "usage":{"input_tokens":2,"output_tokens":3}
        }),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        stop.choices[0].finish_reason,
        Some(crate::core::types::responses::FinishReason::ToolCalls)
    );
    assert!(
        transform(
            &transformer,
            serde_json::json!({"type":"content_block_stop"})
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn cloned_transformers_do_not_share_thinking_lifecycle() {
    let original = AnthropicTransformer::new("claude-test");
    transform(
        &original,
        serde_json::json!({
            "type":"content_block_start", "index":0,
            "content_block":{"type":"thinking", "thinking":"active"}
        }),
    )
    .unwrap();
    let cloned = original.clone();

    assert!(cloned.finish_stream().is_ok());
    assert_lifecycle_error(original.finish_stream().unwrap_err(), 0, "signature");
}

#[test]
fn opaque_values_are_not_exposed_by_transformer_debug() {
    let transformer = AnthropicTransformer::new("claude-test");
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_start", "index":0,
            "content_block":{"type":"thinking", "thinking":"private-thought"}
        }),
    )
    .unwrap();
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_delta", "index":0,
            "delta":{"type":"signature_delta", "signature":"private-signature"}
        }),
    )
    .unwrap();

    let debug = format!("{transformer:?}");
    assert!(!debug.contains("private-thought"));
    assert!(!debug.contains("private-signature"));
}

#[test]
fn thinking_events_require_an_exact_u32_index() {
    let transformer = AnthropicTransformer::new("claude-test");
    for event in [
        serde_json::json!({
            "type":"content_block_start",
            "content_block":{"type":"thinking", "thinking":""}
        }),
        serde_json::json!({
            "type":"content_block_start", "index":u64::from(u32::MAX) + 1,
            "content_block":{"type":"thinking", "thinking":""}
        }),
    ] {
        assert!(matches!(
            transform(&transformer, event),
            Err(ProviderError::ResponseParsing { .. })
        ));
    }
}

fn chunk_from_event(transformer: &AnthropicTransformer, event: serde_json::Value) -> ChatChunk {
    match transformer.transform_chunk(&event.to_string()) {
        Ok(Some(chunk)) => chunk,
        Ok(None) => panic!("expected Anthropic SSE event to produce a chunk"),
        Err(error) => panic!("unexpected Anthropic SSE error: {error}"),
    }
}

#[test]
fn test_message_delta_extracts_cache_tokens() {
    let transformer = AnthropicTransformer::new("claude-3-5-sonnet");
    let event = serde_json::json!({
        "type": "message_delta",
        "delta": {"stop_reason": "end_turn"},
        "usage": {
            "input_tokens": 12,
            "output_tokens": 50,
            "cache_creation_input_tokens": 1000,
            "cache_read_input_tokens": 2000
        }
    });
    let chunk = transformer
        .transform_chunk(&event.to_string())
        .unwrap()
        .unwrap();
    let usage = chunk.usage.as_ref().expect("usage must be present");
    assert_eq!(usage.prompt_tokens, 12);
    assert_eq!(usage.completion_tokens, 50);
    let details = usage
        .prompt_tokens_details
        .as_ref()
        .expect("cache token details must be present");
    assert_eq!(details.cache_creation_tokens, Some(1000));
    assert_eq!(details.cache_read_tokens, Some(2000));
    assert_eq!(details.cached_tokens, Some(2000));
}

#[test]
fn test_message_delta_no_cache_tokens_yields_none_details() {
    let transformer = AnthropicTransformer::new("claude-3-5-sonnet");
    let event = serde_json::json!({
        "type": "message_delta",
        "delta": {"stop_reason": "end_turn"},
        "usage": {"input_tokens": 12, "output_tokens": 50}
    });
    let chunk = transformer
        .transform_chunk(&event.to_string())
        .unwrap()
        .unwrap();
    assert!(
        chunk
            .usage
            .as_ref()
            .unwrap()
            .prompt_tokens_details
            .is_none()
    );
}

#[test]
fn test_chunks_after_message_start_keep_message_id() {
    let transformer = AnthropicTransformer::new("claude-3-5-sonnet");
    let start = chunk_from_event(
        &transformer,
        serde_json::json!({"type":"message_start","message":{"id":"msg_123"}}),
    );
    assert_eq!(start.id, "msg_123");

    let text = chunk_from_event(
        &transformer,
        serde_json::json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"hello"}}),
    );
    assert_eq!(text.id, "msg_123");

    let delta = chunk_from_event(
        &transformer,
        serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}}),
    );
    assert_eq!(delta.id, "msg_123");
    let stop = chunk_from_event(&transformer, serde_json::json!({"type":"message_stop"}));
    assert_eq!(stop.id, "msg_123");
}

#[test]
fn test_cloned_transformers_keep_independent_message_ids() {
    let base = AnthropicTransformer::new("claude-3-5-sonnet");
    let stream_a = base.clone();
    let stream_b = base.clone();
    assert_eq!(
        chunk_from_event(
            &stream_a,
            serde_json::json!({"type":"message_start","message":{"id":"msg_a"}})
        )
        .id,
        "msg_a"
    );
    assert_eq!(
        chunk_from_event(
            &stream_b,
            serde_json::json!({"type":"message_start","message":{"id":"msg_b"}})
        )
        .id,
        "msg_b"
    );
    assert_eq!(
        chunk_from_event(
            &stream_a,
            serde_json::json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"hello"}})
        )
        .id,
        "msg_a"
    );
    assert_eq!(
        chunk_from_event(&stream_b, serde_json::json!({"type":"message_stop"})).id,
        "msg_b"
    );
    assert_eq!(
        chunk_from_event(
            &stream_a,
            serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}})
        )
        .id,
        "msg_a"
    );
}

#[test]
fn issue_761_stream_restores_original_tool_names() -> Result<(), crate::ProviderError> {
    let transformer = AnthropicTransformer::new("claude-3-5-sonnet").with_tool_name_map(
        HashMap::from([("weather_lookup".to_string(), "weather.lookup".to_string())]),
    );
    let chunk = transformer.transform_chunk(
        &serde_json::json!({
            "type":"content_block_start",
            "index":0,
            "content_block":{"type":"tool_use","id":"toolu_123","name":"weather_lookup","input":{}}
        })
        .to_string(),
    )?;
    let name = chunk
        .as_ref()
        .and_then(|chunk| chunk.choices.first())
        .and_then(|choice| choice.delta.tool_calls.as_ref())
        .and_then(|tool_calls| tool_calls.first())
        .and_then(|tool_call| tool_call.function.as_ref())
        .and_then(|function| function.name.as_deref());
    assert_eq!(name, Some("weather.lookup"));
    Ok(())
}
