use super::*;
use crate::core::types::anthropic_continuation::{AnthropicThinkingBlock, ChatMessageExtensions};

// ==================== Chat Response Transformation Tests ====================

#[test]
fn test_transform_chat_response_text() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [
            {"type": "text", "text": "Hello, world!"}
        ],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 10,
            "output_tokens": 20
        }
    });

    let result = client.transform_chat_response(response).unwrap();
    assert_eq!(result.id, "msg_123");
    assert_eq!(result.model, "claude-3-opus-20240229");
    assert_eq!(result.choices.len(), 1);

    if let Some(MessageContent::Text(text)) = &result.choices.first().unwrap().message.content {
        assert_eq!(text, "Hello, world!");
    } else {
        panic!("Expected text content");
    }
}

#[test]
fn response_parser_returns_validated_secret_safe_sidecar() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let response = json!({
        "id": "msg_123",
        "model": "claude-opus-5",
        "content": [
            {"type": "thinking", "thinking": "plan", "signature": "opaque-signature"},
            {"type": "redacted_thinking", "data": "opaque-redacted"},
            {"type": "tool_use", "id": "toolu_1", "name": "lookup", "input": {}}
        ],
        "stop_reason": "tool_use"
    });

    let result = client
        .transform_chat_response_with_continuation(response, &std::collections::HashMap::new())
        .expect("valid continuation response");
    let extension = &result.choice_extensions()[0];
    let blocks = extension
        .anthropic_thinking()
        .expect("thinking sidecar")
        .blocks();
    assert!(matches!(blocks[0], AnthropicThinkingBlock::Thinking { .. }));
    assert!(matches!(
        blocks[1],
        AnthropicThinkingBlock::RedactedThinking { .. }
    ));
    let rendered = format!("{result:?}");
    assert!(!rendered.contains("opaque-signature"));
    assert!(!rendered.contains("opaque-redacted"));

    let invalid = json!({
        "id": "msg_bad",
        "model": "claude-opus-5",
        "content": [{"type": "thinking", "thinking": "plan"}]
    });
    let error = client
        .transform_chat_response_with_continuation(invalid, &std::collections::HashMap::new())
        .expect_err("missing signature must fail");
    assert!(error.to_string().contains("choice 0 block 0"));
}

#[test]
fn response_replay_preserves_multiple_thinking_tool_interleavings() {
    let client = AnthropicClient::new(AnthropicConfig::new_test("test-key")).unwrap();
    let cases = [
        vec![
            json!({"type": "thinking", "thinking": "plan-a", "signature": "sig-a"}),
            json!({"type": "tool_use", "id": "tool-a", "name": "lookup_a", "input": {"a": 1}}),
            json!({"type": "redacted_thinking", "data": "redacted-b"}),
            json!({"type": "tool_use", "id": "tool-b", "name": "lookup_b", "input": {"b": 2}}),
            json!({"type": "thinking", "thinking": "plan-c", "signature": "sig-c"}),
        ],
        vec![
            json!({"type": "thinking", "thinking": "first", "signature": "sig-first"}),
            json!({"type": "tool_use", "id": "tool-first", "name": "lookup_first", "input": {}}),
            json!({"type": "thinking", "thinking": "second", "signature": "sig-second"}),
            json!({"type": "tool_use", "id": "tool-second", "name": "lookup_second", "input": {}}),
        ],
        vec![
            json!({"type": "thinking", "thinking": "before", "signature": "sig-before"}),
            json!({"type": "text", "text": "visible 世界"}),
            json!({"type": "tool_use", "id": "tool-middle", "name": "lookup_middle", "input": {}}),
            json!({"type": "refusal", "refusal": "refused"}),
            json!({"type": "redacted_thinking", "data": "redacted-after"}),
            json!({"type": "text", "text": "tail"}),
        ],
    ];

    for original_content in cases {
        let response = json!({
            "id": "msg_interleaved",
            "model": "claude-fable-5",
            "content": original_content,
            "stop_reason": "tool_use"
        });
        let parsed = client
            .transform_chat_response_with_continuation(response, &std::collections::HashMap::new())
            .expect("interleaved response should parse");
        let (response, extensions) = parsed.into_parts();
        let serialized_extensions = serde_json::to_value(&extensions).unwrap();
        let serialized_text = serialized_extensions.to_string();
        for payload in ["visible 世界", "refused", "tool-middle", "tail"] {
            assert!(
                !serialized_text.contains(payload),
                "ordering metadata must remain index/span-only: {serialized_text}"
            );
        }
        let extensions: Vec<ChatMessageExtensions> =
            serde_json::from_value(serialized_extensions).unwrap();
        let mut request = ChatRequest::new("claude-opus-4-8");
        request.messages.push(response.choices[0].message.clone());

        let replay = client
            .transform_chat_request_with_extensions(&request, &extensions)
            .expect("typed continuation should replay");
        assert_eq!(replay["messages"][0]["content"], json!(original_content));
    }
}

#[test]
fn ordered_continuation_sidecars_are_isolated_per_message() {
    let client = AnthropicClient::new(AnthropicConfig::new_test("test-key")).unwrap();
    let originals = [
        vec![
            json!({"type": "thinking", "thinking": "a", "signature": "sig-a"}),
            json!({"type": "text", "text": "first"}),
            json!({"type": "tool_use", "id": "tool-a", "name": "lookup_a", "input": {}}),
        ],
        vec![
            json!({"type": "tool_use", "id": "tool-b", "name": "lookup_b", "input": {}}),
            json!({"type": "text", "text": "second"}),
            json!({"type": "thinking", "thinking": "b", "signature": "sig-b"}),
        ],
    ];
    let mut request = ChatRequest::new("claude-opus-4-8");
    let mut extensions = Vec::new();

    for (index, original) in originals.iter().enumerate() {
        let parsed = client
            .transform_chat_response_with_continuation(
                json!({
                    "id": format!("msg-{index}"),
                    "model": "claude-fable-5",
                    "content": original,
                    "stop_reason": "tool_use"
                }),
                &std::collections::HashMap::new(),
            )
            .expect("isolated continuation response should parse");
        let (response, choice_extensions) = parsed.into_parts();
        let mut choice_extensions: Vec<ChatMessageExtensions> =
            serde_json::from_value(serde_json::to_value(choice_extensions).unwrap()).unwrap();
        request.messages.push(response.choices[0].message.clone());
        extensions.push(choice_extensions.remove(0));
    }

    let replay = client
        .transform_chat_request_with_extensions(&request, &extensions)
        .expect("each message must use only its own ordering sidecar");
    assert_eq!(replay["messages"][0]["content"], json!(originals[0]));
    assert_eq!(replay["messages"][1]["content"], json!(originals[1]));
}

#[test]
fn ordered_continuation_metadata_fails_closed_on_span_drift() {
    let client = AnthropicClient::new(AnthropicConfig::new_test("test-key")).unwrap();
    let parsed = client
        .transform_chat_response_with_continuation(
            json!({
                "id": "msg-span",
                "model": "claude-fable-5",
                "content": [
                    {"type": "thinking", "thinking": "plan", "signature": "sig"},
                    {"type": "text", "text": "世界"}
                ]
            }),
            &std::collections::HashMap::new(),
        )
        .unwrap();
    let (response, extensions) = parsed.into_parts();
    let mut request = ChatRequest::new("claude-opus-4-8");
    request.messages.push(response.choices[0].message.clone());
    let serialized = serde_json::to_value(extensions).unwrap();

    let mut invalid_utf8 = serialized.clone();
    invalid_utf8[0]["anthropic_block_order"][1]["end"] = json!(1);
    let extensions: Vec<ChatMessageExtensions> = serde_json::from_value(invalid_utf8).unwrap();
    let error = client
        .transform_chat_request_with_extensions(&request, &extensions)
        .expect_err("a visible span cannot split a UTF-8 code point");
    assert!(error.to_string().contains("valid UTF-8 range"));

    let mut missing_visible = serialized;
    missing_visible[0]["anthropic_block_order"]
        .as_array_mut()
        .unwrap()
        .pop();
    let extensions: Vec<ChatMessageExtensions> = serde_json::from_value(missing_visible).unwrap();
    let error = client
        .transform_chat_request_with_extensions(&request, &extensions)
        .expect_err("the order must cover the complete canonical visible payload");
    assert!(error.to_string().contains("does not cover every supported"));
}

#[test]
fn test_transform_chat_response_usage() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [{"type": "text", "text": "Hi"}],
        "usage": {
            "input_tokens": 100,
            "output_tokens": 50
        }
    });

    let result = client.transform_chat_response(response).unwrap();
    let usage = result.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);
}

#[test]
fn test_anthropic_usage_cache_details() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [{"type": "text", "text": "Hi"}],
        "usage": {
            "input_tokens": 100,
            "cache_creation_input_tokens": 12,
            "cache_read_input_tokens": 34,
            "output_tokens": 50
        }
    });

    let result = client.transform_chat_response(response).unwrap();
    let details = result
        .usage
        .as_ref()
        .and_then(|usage| usage.prompt_tokens_details.as_ref())
        .unwrap();
    assert_eq!(result.usage.as_ref().unwrap().prompt_tokens, 146);
    assert_eq!(details.cached_tokens, Some(34));
    assert_eq!(details.cache_creation_tokens, Some(12));
    assert_eq!(details.cache_read_tokens, Some(34));
}

#[test]
fn test_anthropic_client_preserves_thinking_blocks() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [
            {
                "type": "thinking",
                "thinking": "First thought. ",
                "signature": "sig_123"
            },
            {
                "type": "thinking",
                "thinking": "Second thought.",
                "signature": "sig_456"
            },
            {"type": "text", "text": "Answer."}
        ],
        "stop_reason": "end_turn"
    });

    let result = client.transform_chat_response(response).unwrap();
    assert_eq!(
        result
            .choices
            .first()
            .unwrap()
            .message
            .thinking
            .as_ref()
            .and_then(|thinking| thinking.as_text()),
        Some("First thought. Second thought.")
    );
    assert_eq!(
        result.choices.first().unwrap().message.thinking.as_ref(),
        Some(&ThinkingContent::Text {
            text: "First thought. Second thought.".to_string(),
            signature: Some("sig_456".to_string()),
        })
    );
}

#[test]
fn test_transform_chat_response_tool_use() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [
            {
                "type": "tool_use",
                "id": "tool_1",
                "name": "get_weather",
                "input": {"location": "San Francisco"}
            }
        ],
        "stop_reason": "tool_use"
    });

    let result = client.transform_chat_response(response).unwrap();
    let tool_calls = result
        .choices
        .first()
        .unwrap()
        .message
        .tool_calls
        .as_ref()
        .unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls.first().unwrap().id, "tool_1");
    assert_eq!(tool_calls.first().unwrap().function.name, "get_weather");
}

#[test]
fn test_transform_chat_response_finish_reasons() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    // end_turn -> Stop
    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [{"type": "text", "text": "Hi"}],
        "stop_reason": "end_turn"
    });
    let result = client.transform_chat_response(response).unwrap();
    assert!(matches!(
        result.choices.first().unwrap().finish_reason,
        Some(crate::core::types::responses::FinishReason::Stop)
    ));

    // max_tokens -> Length
    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [{"type": "text", "text": "Hi"}],
        "stop_reason": "max_tokens"
    });
    let result = client.transform_chat_response(response).unwrap();
    assert!(matches!(
        result.choices.first().unwrap().finish_reason,
        Some(crate::core::types::responses::FinishReason::Length)
    ));

    // tool_use -> ToolCalls
    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [{"type": "text", "text": "Hi"}],
        "stop_reason": "tool_use"
    });
    let result = client.transform_chat_response(response).unwrap();
    assert!(matches!(
        result.choices.first().unwrap().finish_reason,
        Some(crate::core::types::responses::FinishReason::ToolCalls)
    ));

    // stop_sequence -> StopSequence
    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [{"type": "text", "text": "Hi"}],
        "stop_reason": "stop_sequence"
    });
    let result = client.transform_chat_response(response).unwrap();
    assert!(matches!(
        result.choices.first().unwrap().finish_reason,
        Some(crate::core::types::responses::FinishReason::StopSequence)
    ));

    // refusal -> Refusal
    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [{"type": "refusal", "refusal": "I cannot help with that."}],
        "stop_reason": "refusal"
    });
    let result = client.transform_chat_response(response).unwrap();
    assert!(matches!(
        result.choices.first().unwrap().finish_reason,
        Some(crate::core::types::responses::FinishReason::Refusal)
    ));

    // pause_turn -> PauseTurn
    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [{"type": "text", "text": "Hi"}],
        "stop_reason": "pause_turn"
    });
    let result = client.transform_chat_response(response).unwrap();
    assert!(matches!(
        result.choices.first().unwrap().finish_reason,
        Some(crate::core::types::responses::FinishReason::PauseTurn)
    ));
}
