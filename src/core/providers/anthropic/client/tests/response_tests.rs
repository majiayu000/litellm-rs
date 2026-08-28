use super::*;
use crate::core::types::anthropic_continuation::AnthropicThinkingBlock;

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
            signature: None,
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
