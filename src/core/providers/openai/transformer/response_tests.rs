use super::*;
use crate::core::types::responses::FinishReason;

#[test]
fn test_transform_basic_response() {
    let response = OpenAIChatResponse {
        id: "chatcmpl-123".to_string(),
        object: "chat.completion".to_string(),
        created: 1677652288,
        model: "gpt-4".to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: OpenAIMessage {
                role: "assistant".to_string(),
                content: Some(serde_json::json!("Hello!")),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
                reasoning: None,
                reasoning_details: None,
                reasoning_content: None,
                audio: None,
            },
            finish_reason: Some("stop".to_string()),
            logprobs: None,
        }],
        usage: Some(OpenAIUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        }),
        system_fingerprint: Some("fp_123".to_string()),
    };

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    assert_eq!(result.id, "chatcmpl-123");
    assert_eq!(result.model, "gpt-4");
    assert_eq!(result.choices.len(), 1);
    assert!(matches!(
        result.choices.first().unwrap().finish_reason,
        Some(FinishReason::Stop)
    ));
}

#[test]
fn test_transform_response_preserves_top_level_audio() {
    let response: OpenAIChatResponse = serde_json::from_str(
        r#"{
                "id": "chatcmpl-audio",
                "object": "chat.completion",
                "created": 1677652288,
                "model": "gpt-4o-audio",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "audio": {
                            "id": "audio_123",
                            "expires_at": 1677655888,
                            "data": "base64-response-audio",
                            "transcript": "spoken response"
                        }
                    },
                    "finish_reason": "stop"
                }]
            }"#,
    )
    .unwrap();

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    let audio = result.choices[0]
        .message
        .audio
        .as_ref()
        .expect("top-level audio should be preserved");
    assert_eq!(audio.data, "base64-response-audio");
    assert_eq!(audio.format, None);
}

#[test]
fn test_transform_response_with_usage_details() {
    let response = OpenAIChatResponse {
        id: "chatcmpl-123".to_string(),
        object: "chat.completion".to_string(),
        created: 1677652288,
        model: "gpt-4".to_string(),
        choices: vec![],
        usage: Some(OpenAIUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            prompt_tokens_details: Some(OpenAITokenDetails {
                cached_tokens: Some(20),
                audio_tokens: Some(5),
                reasoning_tokens: None,
            }),
            completion_tokens_details: Some(OpenAITokenDetails {
                cached_tokens: None,
                audio_tokens: Some(10),
                reasoning_tokens: Some(15),
            }),
        }),
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    let usage = result.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(
        usage.prompt_tokens_details.as_ref().unwrap().cached_tokens,
        Some(20)
    );
    assert_eq!(
        usage
            .completion_tokens_details
            .as_ref()
            .unwrap()
            .reasoning_tokens,
        Some(15)
    );
}

#[test]
fn test_transform_response_role_mapping() {
    let roles = vec!["system", "user", "assistant", "tool", "function", "unknown"];

    for role in roles {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: role.to_string(),
                    content: Some(serde_json::json!("test")),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                    reasoning: None,
                    reasoning_details: None,
                    reasoning_content: None,
                    audio: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response);
        assert!(result.is_ok());
    }
}

#[test]
fn test_transform_finish_reasons() {
    let reasons = vec![
        ("stop", FinishReason::Stop),
        ("length", FinishReason::Length),
        ("function_call", FinishReason::FunctionCall),
        ("tool_calls", FinishReason::ToolCalls),
        ("content_filter", FinishReason::ContentFilter),
        ("unknown", FinishReason::Stop), // Default fallback
    ];

    for (reason_str, expected) in reasons {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                    reasoning: None,
                    reasoning_details: None,
                    reasoning_content: None,
                    audio: None,
                },
                finish_reason: Some(reason_str.to_string()),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        assert_eq!(
            result.choices.first().unwrap().finish_reason,
            Some(expected)
        );
    }
}

#[test]
fn test_transform_response_with_tool_calls() {
    let response = OpenAIChatResponse {
        id: "test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "gpt-4".to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: OpenAIMessage {
                role: "assistant".to_string(),
                content: None,
                name: None,
                tool_calls: Some(vec![OpenAIToolCall {
                    id: "call_abc".to_string(),
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionCall {
                        name: "get_weather".to_string(),
                        arguments: r#"{"location":"NYC"}"#.to_string(),
                    },
                }]),
                tool_call_id: None,
                function_call: None,
                reasoning: None,
                reasoning_details: None,
                reasoning_content: None,
                audio: None,
            },
            finish_reason: Some("tool_calls".to_string()),
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    let tool_calls = result
        .choices
        .first()
        .unwrap()
        .message
        .tool_calls
        .as_ref()
        .unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls.first().unwrap().id, "call_abc");
    assert_eq!(tool_calls.first().unwrap().function.name, "get_weather");
}

#[test]
fn test_transform_response_with_reasoning() {
    let response = OpenAIChatResponse {
        id: "test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "o1-preview".to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: OpenAIMessage {
                role: "assistant".to_string(),
                content: Some(serde_json::json!("The answer is 42")),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
                reasoning: Some("Let me think about this...".to_string()),
                reasoning_details: None,
                reasoning_content: None,
                audio: None,
            },
            finish_reason: Some("stop".to_string()),
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    assert!(result.choices.first().unwrap().message.thinking.is_some());
}

#[test]
fn test_transform_response_with_deepseek_reasoning() {
    let response = OpenAIChatResponse {
        id: "test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "deepseek-chat".to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: OpenAIMessage {
                role: "assistant".to_string(),
                content: Some(serde_json::json!("Result")),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
                reasoning: None,
                reasoning_details: None,
                reasoning_content: Some("DeepSeek thinking process...".to_string()),
                audio: None,
            },
            finish_reason: Some("stop".to_string()),
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    assert!(result.choices.first().unwrap().message.thinking.is_some());
}

#[test]
fn test_transform_response_null_content() {
    let response = OpenAIChatResponse {
        id: "test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "gpt-4".to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: OpenAIMessage {
                role: "assistant".to_string(),
                content: Some(serde_json::Value::Null),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
                reasoning: None,
                reasoning_details: None,
                reasoning_content: None,
                audio: None,
            },
            finish_reason: None,
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    assert!(result.choices.first().unwrap().message.content.is_none());
}

#[test]
fn test_transform_response_empty_content() {
    let response = OpenAIChatResponse {
        id: "test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "gpt-4".to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: OpenAIMessage {
                role: "assistant".to_string(),
                content: Some(serde_json::json!("")),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
                reasoning: None,
                reasoning_details: None,
                reasoning_content: None,
                audio: None,
            },
            finish_reason: None,
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    assert!(result.choices.first().unwrap().message.content.is_none());
}

#[test]
fn test_transform_response_with_logprobs() {
    let response = OpenAIChatResponse {
        id: "test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "gpt-4".to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: OpenAIMessage {
                role: "assistant".to_string(),
                content: Some(serde_json::json!("test")),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
                reasoning: None,
                reasoning_details: None,
                reasoning_content: None,
                audio: None,
            },
            finish_reason: Some("stop".to_string()),
            logprobs: Some(serde_json::json!({
                "content": [{
                    "token": "test",
                    "logprob": -0.5,
                    "bytes": [116, 101, 115, 116],
                    "top_logprobs": [{
                        "token": "test",
                        "logprob": -0.5,
                        "bytes": [116, 101, 115, 116]
                    }]
                }]
            })),
        }],
        usage: None,
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    assert!(result.choices.first().unwrap().logprobs.is_some());
}

#[test]
fn test_transform_response_content_array() {
    let response = OpenAIChatResponse {
        id: "test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "gpt-4".to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: OpenAIMessage {
                role: "assistant".to_string(),
                content: Some(serde_json::json!([
                    {"type": "text", "text": "Hello"}
                ])),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
                reasoning: None,
                reasoning_details: None,
                reasoning_content: None,
                audio: None,
            },
            finish_reason: None,
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    assert!(result.choices.first().unwrap().message.content.is_some());
}

#[test]
fn test_transform_response_with_function_call() {
    let response = OpenAIChatResponse {
        id: "test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "gpt-4".to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: OpenAIMessage {
                role: "assistant".to_string(),
                content: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: Some(OpenAIFunctionCall {
                    name: "get_weather".to_string(),
                    arguments: r#"{"location":"NYC"}"#.to_string(),
                }),
                reasoning: None,
                reasoning_details: None,
                reasoning_content: None,
                audio: None,
            },
            finish_reason: Some("function_call".to_string()),
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform(response).unwrap();
    let func_call = result
        .choices
        .first()
        .unwrap()
        .message
        .function_call
        .as_ref()
        .unwrap();
    assert_eq!(func_call.name, "get_weather");
}

// ==================== Stream Transformer Tests ====================

#[test]
fn test_transform_stream_chunk() {
    let chunk = OpenAIStreamChunk {
        id: "chatcmpl-123".to_string(),
        object: "chat.completion.chunk".to_string(),
        created: 1677652288,
        model: "gpt-4".to_string(),
        choices: vec![OpenAIStreamChoice {
            index: 0,
            delta: OpenAIDelta {
                role: Some("assistant".to_string()),
                content: Some("Hello".to_string()),
                reasoning: None,
                reasoning_content: None,
                audio: None,
                tool_calls: None,
                function_call: None,
            },
            finish_reason: None,
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform_stream_chunk(chunk).unwrap();
    assert_eq!(result.id, "chatcmpl-123");
    assert_eq!(result.choices.len(), 1);
    assert_eq!(
        result.choices.first().unwrap().delta.content,
        Some("Hello".to_string())
    );
}

#[test]
fn test_transform_stream_chunk_with_finish() {
    let chunk = OpenAIStreamChunk {
        id: "chatcmpl-123".to_string(),
        object: "chat.completion.chunk".to_string(),
        created: 1677652288,
        model: "gpt-4".to_string(),
        choices: vec![OpenAIStreamChoice {
            index: 0,
            delta: OpenAIDelta {
                role: None,
                content: None,
                reasoning: None,
                reasoning_content: None,
                audio: None,
                tool_calls: None,
                function_call: None,
            },
            finish_reason: Some("stop".to_string()),
            logprobs: None,
        }],
        usage: Some(OpenAIUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        }),
        system_fingerprint: None,
    };

    let result = OpenAIResponseTransformer::transform_stream_chunk(chunk).unwrap();
    assert!(matches!(
        result.choices.first().unwrap().finish_reason,
        Some(FinishReason::Stop)
    ));
    assert!(result.usage.is_some());
}

#[test]
fn test_transform_delta_roles() {
    let roles = vec!["system", "user", "assistant", "tool", "function", "unknown"];

    for role in roles {
        let delta = OpenAIDelta {
            role: Some(role.to_string()),
            content: None,
            reasoning: None,
            reasoning_content: None,
            audio: None,
            tool_calls: None,
            function_call: None,
        };

        let result = OpenAIResponseTransformer::transform_delta(delta);
        assert!(result.is_ok());
    }
}

#[test]
fn test_transform_delta_propagates_tool_and_function_calls() {
    let delta = OpenAIDelta {
        role: None,
        content: None,
        reasoning: None,
        reasoning_content: None,
        audio: None,
        tool_calls: Some(vec![OpenAIToolCallDelta {
            index: 0,
            id: Some("call_abc".to_string()),
            tool_type: Some("function".to_string()),
            function: Some(OpenAIFunctionCallDelta {
                name: Some("get_weather".to_string()),
                arguments: Some(r#"{"c":1}"#.to_string()),
            }),
        }]),
        function_call: Some(OpenAIFunctionCallDelta {
            name: Some("legacy_fn".to_string()),
            arguments: Some(r#"{"x":1}"#.to_string()),
        }),
    };
    let out = OpenAIResponseTransformer::transform_delta(delta).unwrap();
    let tc = out.tool_calls.as_ref().unwrap();
    assert_eq!(tc.len(), 1);
    assert_eq!(tc[0].id.as_deref(), Some("call_abc"));
    assert_eq!(
        tc[0].function.as_ref().unwrap().name.as_deref(),
        Some("get_weather")
    );
    assert_eq!(
        out.function_call.as_ref().unwrap().name.as_deref(),
        Some("legacy_fn")
    );
}

#[test]
fn test_transform_delta_preserves_audio_metadata_and_reasoning_content() {
    let delta = OpenAIDelta {
        role: None,
        content: None,
        reasoning: None,
        reasoning_content: Some("reasoning delta".to_string()),
        audio: Some(OpenAIMessageAudio {
            id: Some("audio-123".to_string()),
            expires_at: Some(1_717_171_717),
            data: Some("base64-audio".to_string()),
            transcript: Some("hello".to_string()),
            format: Some("wav".to_string()),
        }),
        tool_calls: None,
        function_call: None,
    };

    let out = match OpenAIResponseTransformer::transform_delta(delta) {
        Ok(out) => out,
        Err(error) => panic!("delta transformation should succeed: {error}"),
    };
    assert_eq!(out.thinking_content(), Some("reasoning delta"));
    let Some(audio) = out.audio.as_ref() else {
        panic!("audio delta should be present");
    };
    assert_eq!(audio.id.as_deref(), Some("audio-123"));
    assert_eq!(audio.expires_at, Some(1_717_171_717));
    assert_eq!(audio.data.as_deref(), Some("base64-audio"));
    assert_eq!(audio.transcript.as_deref(), Some("hello"));
    assert_eq!(audio.format.as_deref(), Some("wav"));
}

#[test]
fn test_transform_delta_preserves_openai_reasoning() {
    let delta = OpenAIDelta {
        role: None,
        content: None,
        reasoning: Some("openai reasoning delta".to_string()),
        reasoning_content: None,
        audio: None,
        tool_calls: None,
        function_call: None,
    };

    let out = match OpenAIResponseTransformer::transform_delta(delta) {
        Ok(out) => out,
        Err(error) => panic!("delta transformation should succeed: {error}"),
    };
    assert_eq!(out.thinking_content(), Some("openai reasoning delta"));
}
