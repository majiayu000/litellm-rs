use super::*;
use crate::core::models::openai::{ChatMessage, MessageContent, MessageRole};
use crate::core::types::responses::{
    ChatChunk, ChatDelta, ChatStreamChoice, LogProbs, TokenLogProb,
};
use crate::core::types::thinking::{ThinkingDelta, ThinkingUsage};

#[test]
fn test_convert_finish_reason() {
    assert_eq!(
        convert_finish_reason(types::responses::FinishReason::Stop),
        "stop"
    );
    assert_eq!(
        convert_finish_reason(types::responses::FinishReason::Length),
        "length"
    );
    assert_eq!(
        convert_finish_reason(types::responses::FinishReason::ToolCalls),
        "tool_calls"
    );
    assert_eq!(
        convert_finish_reason(types::responses::FinishReason::StopSequence),
        "stop_sequence"
    );
    assert_eq!(
        convert_finish_reason(types::responses::FinishReason::Refusal),
        "refusal"
    );
    assert_eq!(
        convert_finish_reason(types::responses::FinishReason::PauseTurn),
        "pause_turn"
    );
}

#[test]
fn test_format_sse_error_produces_openai_format() {
    let bytes = format_sse_error("something went wrong", "server_error", "internal_error");
    let text = String::from_utf8(bytes.to_vec()).unwrap();

    // Should contain an error event followed by a DONE event
    assert!(text.contains("data: {"));
    assert!(text.contains("data: [DONE]"));

    // Extract the JSON from the first data line
    let first_data = text
        .lines()
        .find(|l| l.starts_with("data: {"))
        .unwrap()
        .strip_prefix("data: ")
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(first_data).unwrap();
    assert_eq!(parsed["error"]["message"], "something went wrong");
    assert_eq!(parsed["error"]["type"], "server_error");
    assert_eq!(parsed["error"]["code"], "internal_error");
}

#[test]
fn test_sse_error_classification_auth() {
    let err = ProviderError::Authentication {
        provider: "openai",
        message: "bad key".to_string(),
    };
    let (t, c) = sse_error_classification(&err);
    assert_eq!(t, "invalid_request_error");
    assert_eq!(c, "authentication_error");
}

#[test]
fn test_sse_error_classification_rate_limit() {
    let err = ProviderError::RateLimit {
        provider: "openai",
        message: "too many".to_string(),
        retry_after: None,
        rpm_limit: None,
        tpm_limit: None,
        current_usage: None,
    };
    let (t, c) = sse_error_classification(&err);
    assert_eq!(t, "rate_limit_error");
    assert_eq!(c, "rate_limit_exceeded");
}

#[test]
fn test_sse_error_classification_timeout() {
    let err = ProviderError::Timeout {
        provider: "openai",
        message: "timed out".to_string(),
    };
    let (t, c) = sse_error_classification(&err);
    assert_eq!(t, "server_error");
    assert_eq!(c, "timeout");
}

#[test]
fn test_sse_error_classification_fallback() {
    let err = ProviderError::Network {
        provider: "openai",
        message: "dns failed".to_string(),
    };
    let (t, c) = sse_error_classification(&err);
    assert_eq!(t, "server_error");
    assert_eq!(c, "internal_error");
}

#[test]
fn test_convert_core_chunk_preserves_thinking_and_function_call() {
    let chunk = ChatChunk {
        id: "chunk-1".to_string(),
        object: "chat.completion.chunk".to_string(),
        created: 123,
        model: "thinking-model".to_string(),
        choices: vec![ChatStreamChoice {
            index: 0,
            delta: ChatDelta {
                role: Some(types::message::MessageRole::Assistant),
                thinking: Some(ThinkingDelta::new("reasoning")),
                function_call: Some(types::responses::FunctionCallDelta {
                    name: Some("legacy_tool".to_string()),
                    arguments: Some("{}".to_string()),
                }),
                ..Default::default()
            },
            finish_reason: None,
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    };

    let converted = convert_core_chunk_to_streaming(chunk).unwrap();
    let delta = &converted.choices[0].delta;
    assert_eq!(delta.reasoning_content.as_deref(), Some("reasoning"));
    assert_eq!(
        delta.thinking.as_ref().and_then(|t| t.content.as_deref()),
        Some("reasoning")
    );
    assert_eq!(
        delta.function_call.as_ref().and_then(|f| f.name.as_deref()),
        Some("legacy_tool")
    );
}

#[test]
fn test_convert_core_chunk_preserves_audio_delta() {
    let chunk = ChatChunk {
        id: "chunk-audio".to_string(),
        object: "chat.completion.chunk".to_string(),
        created: 123,
        model: "audio-model".to_string(),
        choices: vec![ChatStreamChoice {
            index: 0,
            delta: ChatDelta {
                audio: Some(types::responses::AudioDelta {
                    data: Some("base64-audio-delta".to_string()),
                    transcript: Some("hello from audio".to_string()),
                    format: Some("wav".to_string()),
                }),
                ..Default::default()
            },
            finish_reason: None,
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    };

    let converted = convert_core_chunk_to_streaming(chunk).unwrap();
    let Some(audio) = converted.choices[0].delta.audio.as_ref() else {
        panic!("audio delta should be preserved");
    };
    assert_eq!(audio.data.as_deref(), Some("base64-audio-delta"));
    assert_eq!(audio.transcript.as_deref(), Some("hello from audio"));
    assert_eq!(audio.format.as_deref(), Some("wav"));
}

#[test]
fn test_convert_core_chunk_preserves_stream_logprobs() {
    let chunk = ChatChunk {
        id: "chunk-1".to_string(),
        object: "chat.completion.chunk".to_string(),
        created: 123,
        model: "logprob-model".to_string(),
        choices: vec![ChatStreamChoice {
            index: 0,
            delta: ChatDelta::default(),
            finish_reason: None,
            logprobs: Some(LogProbs {
                content: vec![TokenLogProb {
                    token: "hello".to_string(),
                    logprob: -0.25,
                    bytes: None,
                    top_logprobs: None,
                }],
                refusal: None,
            }),
        }],
        usage: None,
        system_fingerprint: None,
    };

    let converted = convert_core_chunk_to_streaming(chunk).unwrap();
    let logprobs = converted.choices[0].logprobs.as_ref().unwrap();
    assert_eq!(logprobs["content"][0]["token"], "hello");
    assert_eq!(logprobs["content"][0]["logprob"], -0.25);
}

#[test]
fn test_convert_usage_preserves_thinking_usage() {
    let converted = convert_usage(types::responses::Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: Some(
            ThinkingUsage::new(42)
                .with_budget(1000)
                .with_provider("anthropic"),
        ),
    });

    assert_eq!(
        converted
            .thinking_usage
            .as_ref()
            .and_then(|usage| usage.thinking_tokens),
        Some(42)
    );
    assert_eq!(
        converted
            .completion_tokens_details
            .as_ref()
            .and_then(|details| details.reasoning_tokens),
        Some(42)
    );
}

#[test]
fn test_convert_usage_merges_thinking_tokens_into_existing_details() {
    let converted = convert_usage(types::responses::Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        prompt_tokens_details: None,
        completion_tokens_details: Some(types::responses::CompletionTokensDetails {
            reasoning_tokens: None,
            audio_tokens: Some(3),
        }),
        thinking_usage: Some(ThinkingUsage::new(42)),
    });

    let details = converted.completion_tokens_details.unwrap();
    assert_eq!(details.reasoning_tokens, Some(42));
    assert_eq!(details.audio_tokens, Some(3));
}

#[test]
fn test_build_core_chat_request_minimal() {
    let request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }],
        ..Default::default()
    };

    let core_request = build_core_chat_request(request, "gpt-4".to_string(), false).unwrap();
    assert_eq!(core_request.model, "gpt-4");
    assert_eq!(core_request.messages.len(), 1);
}

#[test]
fn test_build_core_chat_request_preserves_boundary_fields() {
    let mut extra_body = std::collections::HashMap::new();
    extra_body.insert("provider_knob".to_string(), serde_json::json!("kept"));
    let request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }],
        parallel_tool_calls: Some(false),
        response_format: Some(crate::core::models::openai::ResponseFormat {
            format_type: "json_schema".to_string(),
            json_schema: Some(serde_json::json!({"type": "object"})),
            response_type: Some("json_schema".to_string()),
        }),
        seed: Some(123),
        prediction: Some(serde_json::json!({"type": "content", "content": "expected"})),
        safety_settings: Some(serde_json::json!([{"category": "test"}])),
        cache_control: Some(serde_json::json!({"type": "ephemeral"})),
        extra_body,
        ..Default::default()
    };

    let core_request = build_core_chat_request(request, "gpt-4".to_string(), false).unwrap();
    assert_eq!(core_request.parallel_tool_calls, Some(false));
    assert_eq!(core_request.seed, Some(123));
    assert_eq!(
        core_request
            .response_format
            .as_ref()
            .and_then(|format| format.response_type.as_deref()),
        Some("json_schema")
    );
    assert_eq!(core_request.extra_params["provider_knob"], "kept");
    assert_eq!(
        core_request.extra_params["prediction"]["content"],
        "expected"
    );
    assert_eq!(
        core_request.extra_params["safety_settings"][0]["category"],
        "test"
    );
    assert_eq!(
        core_request.extra_params["cache_control"]["type"],
        "ephemeral"
    );
}

#[test]
fn test_build_core_chat_request_rejects_seed_overflow() {
    let request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }],
        seed: Some(i32::MAX as i64 + 1),
        ..Default::default()
    };

    let err = build_core_chat_request(request, "gpt-4".to_string(), false).unwrap_err();
    assert!(format!("{err}").contains("seed"));
}
