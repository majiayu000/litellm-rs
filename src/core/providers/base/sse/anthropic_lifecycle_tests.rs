use bytes::Bytes;
use futures::{StreamExt, stream};

use super::{AnthropicTransformer, SSETransformer};
use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEStream};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::anthropic_continuation::AnthropicThinkingBlock;
use crate::core::types::responses::{ChatChunk, FinishReason};
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

fn assert_lifecycle_error(error: ProviderError, index: u64, details: &[&str]) {
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

fn completed_blocks(transformer: &AnthropicTransformer) -> Vec<(u32, AnthropicThinkingBlock)> {
    transformer
        .thinking_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .completed
        .clone()
}

async fn broken_chunked_response(prefix: String) -> reqwest::Response {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback listener must bind");
    let address = listener.local_addr().expect("listener address");
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request must connect");
        let mut request = [0_u8; 1024];
        let bytes_read = socket.read(&mut request).await.expect("request must read");
        assert!(bytes_read > 0, "request must not be empty");
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n",
            )
            .await
            .expect("headers must write");
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
async fn raw_eof_before_signature_fails_once_then_ends() {
    let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(sse_event(
        serde_json::json!({
            "type":"content_block_start", "index":3,
            "content_block":{"type":"thinking", "thinking":""}
        }),
    )))]);
    let mut output = UnifiedSSEStream::new(source, AnthropicTransformer::new("claude-test"));

    assert!(output.next().await.expect("start chunk").is_ok());
    let error = output
        .next()
        .await
        .expect("EOF must emit an error")
        .expect_err("unsigned thinking must fail");
    assert_lifecycle_error(error, 3, &["missing its signature"]);
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn raw_eof_requires_stop_after_signed_and_redacted_blocks() {
    for (index, body, detail) in [
        (
            1,
            [
                sse_event(serde_json::json!({
                    "type":"content_block_start", "index":1,
                    "content_block":{"type":"thinking", "thinking":"thought"}
                })),
                sse_event(serde_json::json!({
                    "type":"content_block_delta", "index":1,
                    "delta":{"type":"signature_delta", "signature":"opaque"}
                })),
            ]
            .concat(),
            "content_block_stop",
        ),
        (
            2,
            sse_event(serde_json::json!({
                "type":"content_block_start", "index":2,
                "content_block":{"type":"redacted_thinking", "data":"opaque"}
            })),
            "content_block_stop",
        ),
    ] {
        let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
        let mut output = UnifiedSSEStream::new(source, AnthropicTransformer::new("claude-test"));
        let mut error = None;
        while let Some(item) = output.next().await {
            if let Err(found) = item {
                error = Some(found);
                break;
            }
        }
        assert_lifecycle_error(error.expect("EOF must fail"), index, &[detail]);
        assert!(output.next().await.is_none());
    }
}

#[tokio::test]
async fn raw_eof_after_complete_signed_block_succeeds() {
    let body = [
        sse_event(serde_json::json!({
            "type":"content_block_start", "index":0,
            "content_block":{"type":"thinking", "thinking":"thought"}
        })),
        sse_event(serde_json::json!({
            "type":"content_block_delta", "index":0,
            "delta":{"type":"signature_delta", "signature":"opaque"}
        })),
        sse_event(serde_json::json!({"type":"content_block_stop", "index":0})),
    ]
    .concat();
    let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, AnthropicTransformer::new("claude-test"));
    while let Some(item) = output.next().await {
        assert!(item.is_ok());
    }
}

#[tokio::test]
async fn upstream_error_finalizes_active_state_but_preserves_network_error_when_complete() {
    let active = broken_chunked_response(sse_event(serde_json::json!({
        "type":"content_block_start", "index":11,
        "content_block":{"type":"thinking", "thinking":"private"}
    })))
    .await;
    let mut output = UnifiedSSEStream::new(
        Box::pin(active.bytes_stream()),
        AnthropicTransformer::new("claude-test"),
    );
    assert!(output.next().await.expect("start chunk").is_ok());
    let error = output
        .next()
        .await
        .expect("transport error")
        .expect_err("active lifecycle must fail");
    assert_lifecycle_error(error, 11, &["missing its signature"]);

    let complete = broken_chunked_response(
        [
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
        .concat(),
    )
    .await;
    let mut output = UnifiedSSEStream::new(
        Box::pin(complete.bytes_stream()),
        AnthropicTransformer::new("claude-test"),
    );
    for _ in 0..3 {
        assert!(output.next().await.expect("valid lifecycle chunk").is_ok());
    }
    assert!(matches!(
        output.next().await.expect("transport error"),
        Err(ProviderError::Network { .. })
    ));
}

#[tokio::test]
async fn framed_parse_error_includes_active_lifecycle_context() {
    let events = [
        Ok::<Bytes, reqwest::Error>(Bytes::from(sse_event(serde_json::json!({
            "type":"content_block_start", "index":7,
            "content_block":{"type":"thinking", "thinking":""}
        })))),
        Ok(Bytes::from("data: {\"type\":\"content_block_delta\"\n\n")),
    ];
    let mut output = UnifiedSSEStream::new(stream::iter(events), AnthropicTransformer::new("test"));
    assert!(output.next().await.expect("start chunk").is_ok());
    let error = output
        .next()
        .await
        .expect("parse error")
        .expect_err("malformed event must fail");
    assert_lifecycle_error(
        error,
        7,
        &[
            "ResponseParsing",
            "Failed to parse Anthropic SSE",
            "signature",
        ],
    );
}

#[tokio::test]
async fn non_anthropic_default_finalizer_is_unchanged() {
    let body = sse_event(serde_json::json!({
        "id":"chunk-1", "object":"chat.completion.chunk", "created":1,
        "model":"gpt-test", "choices":[{"index":0,"delta":{"content":"ok"}}]
    }));
    let source = stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, OpenAICompatibleTransformer::new("openai"));
    let chunk = output
        .next()
        .await
        .expect("chunk")
        .expect("valid OpenAI chunk");
    assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("ok"));
    assert!(output.next().await.is_none());
}

#[test]
fn signatures_are_required_and_split_deltas_materialize_losslessly() {
    let missing = AnthropicTransformer::new("claude-test");
    transform(
        &missing,
        serde_json::json!({
            "type":"content_block_start", "index":4,
            "content_block":{"type":"thinking", "thinking":"thought"}
        }),
    )
    .expect("start must be valid");
    let error = transform(
        &missing,
        serde_json::json!({"type":"content_block_stop", "index":4}),
    )
    .expect_err("missing signature must fail");
    assert_lifecycle_error(error, 4, &["missing its signature"]);

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
        transform(&transformer, event).expect("valid signed lifecycle");
    }
    match completed_blocks(&transformer).as_slice() {
        [
            (
                7,
                AnthropicThinkingBlock::Thinking {
                    thinking,
                    signature,
                },
            ),
        ] => {
            assert_eq!(thinking, "first second");
            assert_eq!(signature.expose(), "opaque-signature");
        }
        blocks => panic!("unexpected completed blocks: {blocks:?}"),
    }
}

#[test]
fn omitted_and_redacted_blocks_preserve_validated_payloads() {
    let transformer = AnthropicTransformer::new("claude-test");
    for event in [
        serde_json::json!({
            "type":"content_block_start", "index":1,
            "content_block":{"type":"thinking", "thinking":""}
        }),
        serde_json::json!({
            "type":"content_block_delta", "index":1,
            "delta":{"type":"signature_delta", "signature":"opaque"}
        }),
        serde_json::json!({"type":"content_block_stop", "index":1}),
        serde_json::json!({
            "type":"content_block_start", "index":2,
            "content_block":{"type":"redacted_thinking", "data":"redacted"}
        }),
        serde_json::json!({"type":"content_block_stop", "index":2}),
    ] {
        transform(&transformer, event).expect("valid block lifecycle");
    }
    let blocks = completed_blocks(&transformer);
    assert!(matches!(
        &blocks[0],
        (1, AnthropicThinkingBlock::Thinking { thinking, signature })
            if thinking.is_empty() && signature.expose() == "opaque"
    ));
    assert!(matches!(
        &blocks[1],
        (2, AnthropicThinkingBlock::RedactedThinking { data })
            if data.expose() == "redacted"
    ));

    let error = transform(
        &AnthropicTransformer::new("claude-test"),
        serde_json::json!({
            "type":"content_block_start", "index":9,
            "content_block":{"type":"redacted_thinking", "data":""}
        }),
    )
    .expect_err("empty redacted data must fail");
    assert_lifecycle_error(error, 9, &["empty data"]);
}

#[test]
fn multiple_blocks_keep_completion_order_and_ignore_tool_blocks() {
    let transformer = AnthropicTransformer::new("claude-test");
    for event in [
        serde_json::json!({
            "type":"content_block_start", "index":2,
            "content_block":{"type":"thinking", "thinking":"two", "signature":"sig"}
        }),
        serde_json::json!({
            "type":"content_block_start", "index":3,
            "content_block":{"type":"tool_use", "id":"tool-1", "name":"lookup", "input":{}}
        }),
        serde_json::json!({"type":"content_block_stop", "index":3}),
        serde_json::json!({"type":"content_block_stop", "index":2}),
        serde_json::json!({
            "type":"content_block_start", "index":4,
            "content_block":{"type":"redacted_thinking", "data":"redacted"}
        }),
        serde_json::json!({"type":"content_block_stop", "index":4}),
    ] {
        transform(&transformer, event).expect("valid multi-block lifecycle");
    }
    let completed = completed_blocks(&transformer);
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
fn terminal_finish_rejects_incomplete_thinking_before_success() {
    let transformer = AnthropicTransformer::new("claude-test");
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_start", "index":7,
            "content_block":{"type":"thinking", "thinking":"", "signature":"sig"}
        }),
    )
    .expect("start must be valid");
    let error = transform(
        &transformer,
        serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}}),
    )
    .expect_err("finish reason cannot precede block stop");
    assert_lifecycle_error(error, 7, &["message_delta", "content_block_stop"]);

    transform(
        &transformer,
        serde_json::json!({"type":"content_block_stop", "index":7}),
    )
    .expect("block stop must succeed");
    let terminal = transform(
        &transformer,
        serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}}),
    )
    .expect("terminal event must be valid")
    .expect("terminal event must produce a chunk");
    assert_eq!(terminal.choices[0].finish_reason, Some(FinishReason::Stop));
}

#[test]
fn ordinary_text_tool_and_citation_events_remain_compatible() {
    let transformer = AnthropicTransformer::new("claude-test");
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_start", "index":0,
            "content_block":{"type":"text", "text":""}
        }),
    )
    .expect("text start must be valid");
    let text = transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_delta", "index":0,
            "delta":{"type":"text_delta", "text":"hello"}
        }),
    )
    .expect("text delta must be valid")
    .expect("text delta must produce a chunk");
    assert_eq!(text.choices[0].delta.content.as_deref(), Some("hello"));
    assert!(
        transform(
            &transformer,
            serde_json::json!({
                "type":"content_block_delta", "index":0,
                "delta":{
                    "type":"citations_delta",
                    "citation":{"type":"page_location", "cited_text":"source"}
                }
            }),
        )
        .expect("citation delta must be accepted")
        .is_none()
    );
    transform(
        &transformer,
        serde_json::json!({"type":"content_block_stop", "index":0}),
    )
    .expect("text stop must be valid");

    let tool = transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_start", "index":4,
            "content_block":{"type":"tool_use", "id":"tool-4", "name":"lookup", "input":{}}
        }),
    )
    .expect("tool start must be valid")
    .expect("tool start must produce a chunk");
    assert_eq!(
        tool.choices[0]
            .delta
            .tool_calls
            .as_ref()
            .expect("tool call")[0]
            .id
            .as_deref(),
        Some("tool-4")
    );
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
    .expect("thinking start must be valid");
    transform(
        &transformer,
        serde_json::json!({
            "type":"content_block_delta", "index":0,
            "delta":{"type":"signature_delta", "signature":"private-signature"}
        }),
    )
    .expect("signature must be valid");
    let debug = format!("{transformer:?}");
    assert!(!debug.contains("private-thought"));
    assert!(!debug.contains("private-signature"));
}
