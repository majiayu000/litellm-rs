use super::*;
use crate::core::models::openai::{
    ChatMessage, Function, FunctionCall, MessageContent, MessageRole, ToolChoiceFunction,
    ToolChoiceFunctionSpec,
};
use crate::core::types::responses::{
    ChatChunk, ChatDelta, ChatStreamChoice, LogProbs, TokenLogProb,
};
use crate::core::types::thinking::{ThinkingDelta, ThinkingUsage};

#[test]
fn guardrail_recheck_is_only_needed_for_visible_continuation_content() {
    use crate::core::providers::ChatMessageContinuation;
    use crate::core::types::anthropic_continuation::{
        AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent,
    };

    assert!(!ChatMessageContinuation::new().has_visible_thinking());

    let visible = ChatMessageContinuation::new().with_anthropic_thinking(
        AnthropicThinkingContent::new(vec![AnthropicThinkingBlock::Thinking {
            thinking: "visible guardrail phrase".to_string(),
            signature: AnthropicSignature::try_from("opaque signature").unwrap(),
        }]),
    );
    assert!(visible.has_visible_thinking());
}

#[test]
fn guardrail_input_projection_includes_only_visible_continuation_thinking() {
    use crate::core::providers::ChatMessageContinuation;
    use crate::core::types::anthropic_continuation::{
        AnthropicRedactedData, AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent,
    };

    let request = ChatCompletionRequest {
        messages: vec![ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text("answer".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }],
        ..Default::default()
    };
    let continuation = ChatMessageContinuation::new().with_anthropic_thinking(
        AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking: "visible input phrase".to_string(),
                signature: AnthropicSignature::try_from("opaque signature").unwrap(),
            },
            AnthropicThinkingBlock::RedactedThinking {
                data: AnthropicRedactedData::try_from("opaque redacted payload").unwrap(),
            },
        ]),
    );

    let projected =
        guardrail_request_with_continuation(&request, std::slice::from_ref(&continuation))
            .expect("valid assistant continuation should project");
    let MessageContent::Text(content) = projected.messages[0].content.as_ref().unwrap() else {
        panic!("projection should remain text");
    };
    assert!(content.contains("answer"));
    assert!(content.contains("visible input phrase"));
    assert!(!content.contains("opaque signature"));
    assert!(!content.contains("opaque redacted payload"));

    assert!(guardrail_request_with_continuation(&request, &[]).is_err());
    let mut wrong_role = request;
    wrong_role.messages[0].role = MessageRole::User;
    assert!(guardrail_request_with_continuation(&wrong_role, &[continuation]).is_err());
}

#[test]
fn guardrail_masking_discards_stale_anthropic_text_ranges() {
    use crate::core::providers::{AnthropicContentBlockOrder, ChatMessageContinuation};
    use crate::core::types::anthropic_continuation::{
        AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent,
    };

    let original = ChatCompletionRequest {
        messages: vec![ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text("Email user@example.com".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }],
        ..Default::default()
    };
    let mut projected = original.clone();
    projected.messages[0].content = Some(MessageContent::Text("Email [MASKED]".to_string()));
    let continuation = ChatMessageContinuation::new()
        .with_anthropic_thinking(AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking: "visible reasoning".to_string(),
                signature: AnthropicSignature::try_from("opaque signature").unwrap(),
            },
        ]))
        .with_anthropic_block_order(vec![
            AnthropicContentBlockOrder::Thinking { index: 0 },
            AnthropicContentBlockOrder::Text { start: 0, end: 22 },
        ]);

    let sanitized = continuation_after_input_projection(&original, &projected, vec![continuation])
        .expect("changed content should sanitize continuation metadata");

    assert!(sanitized[0].anthropic_block_order().is_none());
    assert!(sanitized[0].anthropic_thinking().is_some());
}

#[test]
fn guardrail_output_masking_discards_stale_anthropic_text_ranges() {
    use crate::core::models::openai::ChatChoice;
    use crate::core::providers::{AnthropicContentBlockOrder, ChatMessageContinuation};

    let message = |content: &str| ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text(content.to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let response = |content: &str| ChatCompletionResponse {
        id: "chatcmpl-test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "test-model".to_string(),
        system_fingerprint: None,
        choices: vec![ChatChoice {
            index: 0,
            message: message(content),
            logprobs: None,
            finish_reason: Some("stop".to_string()),
        }],
        usage: None,
    };
    let continuation = ChatMessageContinuation::new()
        .with_anthropic_block_order(vec![AnthropicContentBlockOrder::Text { start: 0, end: 22 }]);

    let sanitized = continuation_after_output_projection(
        &response("Email user@example.com"),
        &response("Email [MASKED]"),
        vec![continuation],
    )
    .expect("changed output content should sanitize continuation metadata");

    assert!(sanitized[0].anthropic_block_order().is_none());
}

#[test]
fn guardrail_projection_includes_visible_thinking_without_opaque_continuation_data() {
    use crate::core::providers::ChatMessageContinuation;
    use crate::core::types::anthropic_continuation::{
        AnthropicRedactedData, AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent,
    };

    let response = ChatCompletionResponse {
        id: "chatcmpl-test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "claude-opus-4-8".to_string(),
        system_fingerprint: None,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: MessageRole::Assistant,
                content: Some(MessageContent::Text("answer".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
            logprobs: None,
            finish_reason: Some("stop".to_string()),
        }],
        usage: None,
    };
    let continuation = ChatMessageContinuation::new().with_anthropic_thinking(
        AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking: "visible guardrail phrase".to_string(),
                signature: AnthropicSignature::try_from("opaque signature").unwrap(),
            },
            AnthropicThinkingBlock::RedactedThinking {
                data: AnthropicRedactedData::try_from("opaque redacted payload").unwrap(),
            },
        ]),
    );

    let projected = guardrail_response_with_continuation(&response, &[continuation])
        .expect("matching response continuation must project");
    let MessageContent::Text(content) = projected.choices[0]
        .message
        .content
        .as_ref()
        .expect("projected content")
    else {
        panic!("projection should remain text");
    };
    assert!(content.contains("answer"));
    assert!(content.contains("visible guardrail phrase"));
    assert!(!content.contains("opaque signature"));
    assert!(!content.contains("opaque redacted payload"));
}

#[test]
fn typed_continuation_fails_closed_only_for_enabled_budget_scopes() {
    use crate::core::budget::{
        ModelLimitConfig, ProviderLimitConfig, ResetPeriod, UnifiedBudgetLimits,
    };

    let budgets = UnifiedBudgetLimits::new();
    assert!(!continuation_budget_enabled(
        &budgets,
        "anthropic",
        "claude-opus-5",
        false,
    ));
    budgets.providers.set_provider_limit(
        "anthropic",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    assert!(continuation_budget_enabled(
        &budgets,
        "anthropic",
        "claude-opus-5",
        false,
    ));
    assert!(continuation_budget_enabled(
        &UnifiedBudgetLimits::new(),
        "anthropic",
        "claude-opus-5",
        true,
    ));
    let model_budgets = UnifiedBudgetLimits::new();
    model_budgets.models.set_model_limit(
        "claude-opus-5",
        ModelLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    assert!(continuation_budget_enabled(
        &model_budgets,
        "anthropic",
        "claude-opus-5",
        false,
    ));
}

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
                content: None,
                thinking: Some(ThinkingDelta::new("reasoning")),
                tool_calls: None,
                function_call: Some(types::responses::FunctionCallDelta {
                    name: Some("legacy_tool".to_string()),
                    arguments: Some("{}".to_string()),
                }),
                audio: None,
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
                role: None,
                content: None,
                thinking: None,
                tool_calls: None,
                function_call: None,
                audio: Some(types::responses::AudioDelta {
                    id: Some("audio-123".to_string()),
                    expires_at: Some(1_717_171_717),
                    data: Some("base64-audio-delta".to_string()),
                    transcript: Some("hello from audio".to_string()),
                    format: Some("wav".to_string()),
                }),
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
    assert_eq!(audio.id.as_deref(), Some("audio-123"));
    assert_eq!(audio.expires_at, Some(1_717_171_717));
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
            delta: ChatDelta {
                role: None,
                content: None,
                thinking: None,
                tool_calls: None,
                function_call: None,
                audio: None,
            },
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

    let core_request = build_core_chat_request(&request, "gpt-4".to_string(), false).unwrap();
    assert_eq!(core_request.model, "gpt-4");
    assert_eq!(core_request.messages.len(), 1);
}

#[test]
fn test_build_core_chat_request_preserves_transport_fields() {
    let mut extra_body = std::collections::HashMap::new();
    extra_body.insert("provider_knob".to_string(), serde_json::json!("kept"));
    extra_body.insert("modalities".to_string(), serde_json::json!("wrong"));
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
        stream_options: Some(crate::core::models::openai::StreamOptions {
            include_usage: Some(true),
        }),
        tools: Some(vec![Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "lookup".to_string(),
                description: Some("Look up a record".to_string()),
                parameters: Some(serde_json::json!({"type": "object"})),
            },
        }]),
        tool_choice: Some(ToolChoice::Specific(ToolChoiceFunction {
            tool_type: "function".to_string(),
            function: ToolChoiceFunctionSpec {
                name: "lookup".to_string(),
            },
        })),
        functions: Some(vec![Function {
            name: "legacy_lookup".to_string(),
            description: None,
            parameters: Some(serde_json::json!({"type": "object"})),
        }]),
        function_call: Some(FunctionCall {
            name: "legacy_lookup".to_string(),
            arguments: "{}".to_string(),
        }),
        parallel_tool_calls: Some(false),
        response_format: Some(crate::core::models::openai::ResponseFormat {
            format_type: "json_schema".to_string(),
            json_schema: Some(serde_json::json!({"type": "object"})),
            response_type: Some("json_schema".to_string()),
        }),
        seed: Some(123),
        modalities: Some(vec!["text".to_string(), "audio".to_string()]),
        audio: Some(crate::core::models::openai::AudioParams {
            voice: "alloy".to_string(),
            format: "wav".to_string(),
        }),
        prediction: Some(serde_json::json!({"type": "content", "content": "expected"})),
        safety_settings: Some(serde_json::json!([{"category": "test"}])),
        cache_control: Some(serde_json::json!({"type": "ephemeral"})),
        store: Some(true),
        metadata: Some(std::collections::HashMap::from([(
            "trace_id".to_string(),
            "trace-123".to_string(),
        )])),
        service_tier: Some("flex".to_string()),
        extra_body,
        ..Default::default()
    };

    let core_request = build_core_chat_request(&request, "gpt-4".to_string(), false).unwrap();
    assert_eq!(core_request.parallel_tool_calls, Some(false));
    assert_eq!(core_request.seed, Some(123));
    assert_eq!(
        core_request
            .stream_options
            .as_ref()
            .and_then(|options| options.include_usage),
        Some(true)
    );
    assert_eq!(
        core_request
            .tools
            .as_ref()
            .and_then(|tools| tools.first())
            .map(|tool| tool.function.name.as_str()),
        Some("lookup")
    );
    match core_request.tool_choice.as_ref().expect("tool choice") {
        crate::core::types::tools::ToolChoice::Specific {
            choice_type,
            function,
        } => {
            assert_eq!(choice_type, "function");
            assert_eq!(function.as_ref().map(|f| f.name.as_str()), Some("lookup"));
        }
        _ => panic!("expected specific tool choice"),
    }
    assert_eq!(
        core_request
            .functions
            .as_ref()
            .and_then(|functions| functions.first())
            .and_then(|function| function.get("name")),
        Some(&serde_json::json!("legacy_lookup"))
    );
    assert_eq!(
        core_request
            .function_call
            .as_ref()
            .and_then(|call| call.get("name")),
        Some(&serde_json::json!("legacy_lookup"))
    );
    assert_eq!(
        core_request
            .response_format
            .as_ref()
            .and_then(|format| format.response_type.as_deref()),
        Some("json_schema")
    );
    assert_eq!(core_request.extra_params["provider_knob"], "kept");
    assert_eq!(core_request.extra_params["modalities"][0], "text");
    assert_eq!(core_request.extra_params["audio"]["voice"], "alloy");
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
    assert_eq!(core_request.store, Some(true));
    assert_eq!(
        core_request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("trace_id")),
        Some(&"trace-123".to_string())
    );
    assert_eq!(core_request.service_tier.as_deref(), Some("flex"));
}

#[test]
fn test_build_core_chat_request_usage_override_does_not_mutate_original() {
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
        stream_options: Some(crate::core::models::openai::StreamOptions {
            include_usage: Some(false),
        }),
        ..Default::default()
    };

    let core_request = match build_core_chat_request_with_stream_usage(
        &request,
        "gpt-4".to_string(),
        true,
        Some(true),
    ) {
        Ok(core_request) => core_request,
        Err(error) => panic!("valid stream request should build: {error}"),
    };

    assert_eq!(
        core_request
            .stream_options
            .as_ref()
            .and_then(|options| options.include_usage),
        Some(true)
    );
    assert_eq!(
        request
            .stream_options
            .as_ref()
            .and_then(|options| options.include_usage),
        Some(false)
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

    let err = build_core_chat_request(&request, "gpt-4".to_string(), false).unwrap_err();
    assert!(format!("{err}").contains("seed"));
}
