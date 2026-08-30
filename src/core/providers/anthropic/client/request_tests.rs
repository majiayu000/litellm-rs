use serde_json::json;

use super::*;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::anthropic::{AnthropicConfig, AnthropicProvider};
use crate::core::providers::{
    AnthropicContentBlockOrder, ChatContinuationRequest, ChatMessageContinuation,
};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::anthropic_continuation::{
    AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent,
};
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::content::{
    AudioData, CacheControl, ContentPart, DocumentSource, ImageSource,
};
use crate::core::types::message::{MessageContent, MessageRole};
use crate::core::types::thinking::{ThinkingConfig, ThinkingEffort};
use crate::core::types::tools::{
    FunctionCall, FunctionChoice, FunctionDefinition, ResponseFormat, Tool, ToolCall, ToolChoice,
    ToolType,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

fn anthropic_client() -> AnthropicClient {
    AnthropicClient::new(AnthropicConfig::new_test("test-key")).unwrap()
}

fn tool(name: &str) -> Tool {
    Tool {
        tool_type: ToolType::Function,
        function: FunctionDefinition {
            name: name.to_string(),
            description: Some("lookup".to_string()),
            parameters: Some(json!({"type": "object"})),
        },
    }
}

async fn continuation_capture_server() -> (String, oneshot::Receiver<String>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (request_sender, request_receiver) = oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request_bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = socket.read(&mut buffer).await.unwrap();
            assert_ne!(read, 0, "client closed before sending a complete request");
            request_bytes.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request_bytes
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request_bytes[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if request_bytes.len() >= header_end + 4 + content_length {
                break;
            }
        }
        request_sender
            .send(String::from_utf8(request_bytes).unwrap())
            .unwrap();
        let body = r#"{"id":"msg-response","model":"claude-3-opus-20240229","content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    (format!("http://{address}"), request_receiver)
}

fn signed_continuation_extension() -> ChatMessageContinuation {
    ChatMessageContinuation::new().with_anthropic_thinking(AnthropicThinkingContent::new(vec![
        AnthropicThinkingBlock::Thinking {
            thinking: "plan".to_string(),
            signature: AnthropicSignature::try_from("opaque-signature").unwrap(),
        },
    ]))
}

#[test]
fn signed_legacy_continuation_enables_thinking_when_http_request_omits_it() {
    let mut request = ChatRequest::new("claude-opus-4-8");
    request.max_tokens = Some(10_001);
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text("visible".to_string())),
        ..Default::default()
    });
    request.messages.push(ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("continue".to_string())),
        ..Default::default()
    });

    let transformed = anthropic_client()
        .transform_chat_request_with_extensions(
            &request,
            &[
                signed_continuation_extension(),
                ChatMessageContinuation::new(),
            ],
        )
        .expect("signed continuation should enable legacy thinking");

    assert_eq!(
        transformed["thinking"],
        json!({"type": "enabled", "budget_tokens": 10_000})
    );
    assert_eq!(transformed["max_tokens"], 10_001);
}

#[test]
fn signed_legacy_continuation_does_not_raise_the_validated_output_limit() {
    let mut request = ChatRequest::new("claude-opus-4-8");
    request.max_tokens = Some(10_000);
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text("visible".to_string())),
        ..Default::default()
    });

    let error = anthropic_client()
        .transform_chat_request_with_extensions(&request, &[signed_continuation_extension()])
        .expect_err("thinking replay must not raise a validated max_tokens limit");

    assert!(error.to_string().contains("max_tokens"));
    assert_eq!(request.max_tokens, Some(10_000));
}

#[tokio::test]
async fn continuation_send_path_adds_interleaved_beta_once() {
    let (base_url, captured) = continuation_capture_server().await;
    let client = AnthropicClient::new(
        AnthropicConfig::new_test("test-key")
            .with_base_url(base_url)
            .with_endpoint_access(ProviderEndpointAccess::PrivateNetwork),
    )
    .unwrap();
    let mut request = ChatRequest::new("claude-sonnet-4-20250514");
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text("visible".to_string())),
        ..Default::default()
    });
    request.thinking = Some(ThinkingConfig::new().enabled());
    request.extra_params.insert(
        "anthropic_beta".to_string(),
        json!(["interleaved-thinking-2025-05-14"]),
    );
    let envelope =
        ChatContinuationRequest::new(request, vec![signed_continuation_extension()]).unwrap();

    client
        .chat_with_continuation(envelope)
        .await
        .expect("mock Anthropic request should succeed");
    let request = captured.await.unwrap();
    let beta_lines = request
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("anthropic-beta:"))
        .collect::<Vec<_>>();
    assert_eq!(beta_lines.len(), 1, "request: {request}");
    assert_eq!(
        beta_lines[0]
            .matches("interleaved-thinking-2025-05-14")
            .count(),
        1,
        "request: {request}"
    );
}

#[test]
fn claude5_policy_is_exact_defaults_adaptive_and_reports_supported_params() {
    let client = anthropic_client();
    let provider = AnthropicProvider::new(AnthropicConfig::new_test("test-key")).unwrap();
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        let wire = client
            .transform_chat_request(&ChatRequest::new(model).add_user_message("solve"))
            .expect("exact Claude 5 IDs use protocol policy before catalog registration");
        assert_eq!(wire["thinking"], json!({"type": "adaptive"}));
        let params = provider.get_supported_openai_params(model);
        assert!(params.contains(&"reasoning_effort"));
        assert!(params.contains(&"response_format"));
        assert!(
            !params
                .iter()
                .any(|p| matches!(*p, "temperature" | "top_p" | "top_k"))
        );
        let mut formatted = ChatRequest::new(model).add_user_message("solve");
        formatted.reasoning_effort = Some("high".to_string());
        formatted.response_format = Some(ResponseFormat {
            format_type: "json_schema".to_string(),
            json_schema: Some(json!({"type": "object"})),
            response_type: None,
        });
        let formatted = client.transform_chat_request(&formatted).unwrap();
        assert_eq!(formatted["output_config"]["effort"], "high");
        assert_eq!(formatted["output_config"]["format"]["type"], "json_schema");
        assert!(formatted.get("response_format").is_none());
    }
    for model in [
        "Claude-Fable-5",
        "claude-fable-5-latest",
        "claude-opus-5-20260801",
    ] {
        assert!(
            client
                .transform_chat_request(&ChatRequest::new(model).add_user_message("solve"))
                .is_err()
        );
    }
    assert!(
        !provider
            .get_supported_openai_params("Claude-Fable-5")
            .contains(&"reasoning_effort")
    );
}

#[test]
fn claude5_disabled_thinking_obeys_model_effort_policy() {
    let client = anthropic_client();
    for effort in [None, Some("low"), Some("high")] {
        let mut opus = ChatRequest::new("claude-opus-5").add_user_message("solve");
        opus.thinking = Some(ThinkingConfig::new());
        opus.reasoning_effort = effort.map(str::to_string);
        let wire = client
            .transform_chat_request(&opus)
            .expect("Opus allows disabled at high or below");
        assert_eq!(wire["thinking"], json!({"type": "disabled"}));
    }
    for effort in ["xhigh", "max"] {
        let mut opus = ChatRequest::new("claude-opus-5").add_user_message("solve");
        opus.thinking = Some(ThinkingConfig::new());
        opus.reasoning_effort = Some(effort.to_string());
        assert!(client.transform_chat_request(&opus).is_err());
    }
    let mut sonnet = ChatRequest::new("claude-sonnet-5").add_user_message("solve");
    sonnet.thinking = Some(ThinkingConfig::new());
    sonnet.reasoning_effort = Some("max".to_string());
    assert_eq!(
        client.transform_chat_request(&sonnet).unwrap()["thinking"]["type"],
        "disabled"
    );

    let mut fable = ChatRequest::new("claude-fable-5").add_user_message("solve");
    fable.thinking = Some(ThinkingConfig::new());
    assert!(client.transform_chat_request(&fable).is_err());
}

#[test]
fn claude5_rejects_manual_thinking_non_default_sampling_and_prefill() {
    let client = anthropic_client();
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        let mut compatible = ChatRequest::new(model).add_user_message("solve");
        compatible.temperature = Some(1.0);
        compatible.top_p = Some(0.99);
        assert!(client.transform_chat_request(&compatible).is_ok());

        for (field, value) in [
            ("temperature", json!(0.5)),
            ("top_p", json!(0.5)),
            ("top_k", json!(5)),
        ] {
            let mut invalid = ChatRequest::new(model).add_user_message("solve");
            match field {
                "temperature" => invalid.temperature = value.as_f64().map(|v| v as f32),
                "top_p" => invalid.top_p = value.as_f64().map(|v| v as f32),
                _ => {
                    invalid.extra_params.insert(field.to_string(), value);
                }
            }
            assert!(
                client.transform_chat_request(&invalid).is_err(),
                "{model} accepted {field}"
            );
        }
        let mut manual = ChatRequest::new(model).add_user_message("solve");
        manual.thinking = Some(ThinkingConfig::new().enabled().with_budget(1024));
        assert!(client.transform_chat_request(&manual).is_err());
        let prefill = ChatRequest::new(model)
            .add_user_message("solve")
            .add_assistant_message("answer");
        assert!(client.transform_chat_request(&prefill).is_err());
    }
    let mut invalid = ChatRequest::new("claude-opus-5").add_user_message("solve");
    invalid.reasoning_effort = Some("maximum".to_string());
    assert!(client.transform_chat_request(&invalid).is_err());

    let mut conflict = ChatRequest::new("claude-opus-5").add_user_message("solve");
    conflict.reasoning_effort = Some("high".to_string());
    conflict.thinking = Some(
        ThinkingConfig::new()
            .enabled()
            .with_effort(ThinkingEffort::Low),
    );
    assert!(client.transform_chat_request(&conflict).is_err());

    let mut old_model = ChatRequest::new("claude-3-opus-20240229").add_user_message("solve");
    old_model.reasoning_effort = Some("high".to_string());
    assert!(client.transform_chat_request(&old_model).is_err());
}

#[test]
fn claude5_rejects_extension_only_terminal_assistant_prefill() {
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        let mut request = ChatRequest::new(model).add_user_message("solve");
        request.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            ..Default::default()
        });
        let error = anthropic_client()
            .transform_chat_request_with_extensions(
                &request,
                &[
                    ChatMessageContinuation::new(),
                    signed_continuation_extension(),
                ],
            )
            .expect_err("sidecar-only terminal assistant payload is still a prefill");
        assert!(error.to_string().contains("assistant prefill"));
    }

    let mut replay = ChatRequest::new("claude-opus-5");
    replay.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text("answer".to_string())),
        ..Default::default()
    });
    replay = replay.add_user_message("continue");
    let wire = anthropic_client()
        .transform_chat_request_with_extensions(
            &replay,
            &[
                signed_continuation_extension(),
                ChatMessageContinuation::new(),
            ],
        )
        .unwrap();
    assert_eq!(wire["thinking"], json!({"type": "adaptive"}));
    assert!(wire["thinking"].get("budget_tokens").is_none());
}

#[test]
fn claude5_legacy_function_calls_error_while_modern_tools_remain_valid() {
    let mut legacy = ChatRequest::new("claude-opus-5").add_user_message("weather?");
    legacy.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        function_call: Some(FunctionCall {
            name: "lookup".to_string(),
            arguments: "{}".to_string(),
        }),
        ..Default::default()
    });
    let error = AnthropicClient::validate_claude_5_legacy_functions(&legacy)
        .expect_err("legacy function_call must fail");
    assert!(error.to_string().contains("tools/tool_choice"));

    let modern = ChatRequest::new("claude-opus-5")
        .add_user_message("weather?")
        .with_tools(vec![tool("lookup")]);
    assert!(AnthropicClient::validate_claude_5_legacy_functions(&modern).is_ok());
}

#[test]
fn issue_761_anthropic_transform_tools_sanitizes_invalid_function_names() {
    let result = anthropic_client()
        .transform_tools(&[tool("actions/download-job-logs.for-workflow-run")])
        .unwrap();

    assert_eq!(
        result[0]["name"],
        "actions_download-job-logs_for-workflow-run"
    );
}

#[test]
fn issue_761_anthropic_transform_request_sanitizes_specific_tool_choice() {
    let mut request = ChatRequest::new("claude-3-opus-20240229")
        .add_user_message("weather?")
        .with_tools(vec![tool("weather.lookup")]);
    request.tool_choice = Some(ToolChoice::Specific {
        choice_type: "function".to_string(),
        function: Some(FunctionChoice {
            name: "weather.lookup".to_string(),
        }),
    });

    let transformed = anthropic_client().transform_chat_request(&request).unwrap();

    assert_eq!(transformed["tools"][0]["name"], "weather_lookup");
    assert_eq!(transformed["tool_choice"]["name"], "weather_lookup");
}

#[test]
fn issue_761_anthropic_transform_request_rejects_sanitized_tool_choice_alias() {
    let mut request = ChatRequest::new("claude-3-opus-20240229")
        .add_user_message("weather?")
        .with_tools(vec![tool("weather_lookup")]);
    request.tool_choice = Some(ToolChoice::Specific {
        choice_type: "function".to_string(),
        function: Some(FunctionChoice {
            name: "weather.lookup".to_string(),
        }),
    });

    let result = anthropic_client().transform_chat_request(&request);
    let message = result
        .err()
        .map_or_else(String::new, |error| error.to_string());

    assert!(message.contains("Tool choice"));
    assert!(message.contains("weather.lookup"));
    assert!(message.contains("weather_lookup"));
}

#[test]
fn issue_761_anthropic_transform_request_sanitizes_assistant_tool_call_history() {
    let mut request = ChatRequest::new("claude-3-opus-20240229");
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text("checking".to_string())),
        tool_calls: Some(vec![ToolCall {
            id: "toolu_123".to_string(),
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: "weather.lookup".to_string(),
                arguments: r#"{"city":"Paris"}"#.to_string(),
            },
        }]),
        ..Default::default()
    });

    let transformed = anthropic_client().transform_chat_request(&request).unwrap();
    let content = transformed["messages"][0]["content"].as_array().unwrap();

    assert_eq!(content[1]["name"], "weather_lookup");
}

#[test]
fn issue_761_anthropic_transform_request_rejects_sanitized_history_alias() {
    let mut request = ChatRequest::new("claude-3-opus-20240229")
        .add_user_message("weather?")
        .with_tools(vec![tool("weather_lookup")]);
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        tool_calls: Some(vec![ToolCall {
            id: "toolu_123".to_string(),
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: "weather.lookup".to_string(),
                arguments: r#"{"city":"Paris"}"#.to_string(),
            },
        }]),
        ..Default::default()
    });

    let result = anthropic_client().transform_chat_request(&request);
    let message = result
        .err()
        .map_or_else(String::new, |error| error.to_string());

    assert!(message.contains("Tool call"));
    assert!(message.contains("weather.lookup"));
    assert!(message.contains("weather_lookup"));
}

#[test]
fn issue_761_anthropic_transform_tools_rejects_sanitized_name_collisions() {
    let error = anthropic_client()
        .transform_tools(&[tool("weather.lookup"), tool("weather/lookup")])
        .expect_err("colliding sanitized names must fail closed");
    let message = error.to_string();

    assert!(message.contains("weather.lookup"));
    assert!(message.contains("weather/lookup"));
    assert!(message.contains("weather_lookup"));
}

#[test]
fn issue_761_anthropic_transform_response_restores_original_tool_names()
-> Result<(), crate::core::providers::unified_provider::ProviderError> {
    let client = anthropic_client();
    let request = ChatRequest::new("claude-3-opus-20240229")
        .add_user_message("weather?")
        .with_tools(vec![tool("weather.lookup")]);
    let tool_name_map = client.anthropic_tool_name_map_for_request(&request)?;
    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [{
            "type": "tool_use",
            "id": "toolu_123",
            "name": "weather_lookup",
            "input": {"city": "Paris"}
        }],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 10, "output_tokens": 5}
    });

    let result = client.transform_chat_response_with_tool_name_map(response, &tool_name_map)?;
    let name = result.choices[0]
        .message
        .tool_calls
        .as_ref()
        .and_then(|tool_calls| tool_calls.first())
        .map(|tool_call| tool_call.function.name.as_str());

    assert_eq!(name, Some("weather.lookup"));
    Ok(())
}

#[test]
fn issue_764_maps_user_and_top_level_cache_control()
-> Result<(), crate::core::providers::unified_provider::ProviderError> {
    let mut request = ChatRequest::new("claude-3-opus-20240229").add_user_message("hello");
    request.user = Some("user-123".to_string());
    request
        .extra_params
        .insert("cache_control".to_string(), json!({"type": "ephemeral"}));

    let transformed = anthropic_client().transform_chat_request(&request)?;

    assert_eq!(transformed["metadata"]["user_id"], "user-123");
    assert_eq!(transformed["cache_control"], json!({"type": "ephemeral"}));
    Ok(())
}

#[test]
fn issue_764_preserves_document_cache_control()
-> Result<(), crate::core::providers::unified_provider::ProviderError> {
    let mut request = ChatRequest::new("claude-3-opus-20240229");
    request.messages.push(ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![ContentPart::Document {
            source: DocumentSource {
                media_type: "application/pdf".to_string(),
                data: "JVBERi0=".to_string(),
            },
            cache_control: Some(CacheControl {
                cache_type: "ephemeral".to_string(),
            }),
        }])),
        ..Default::default()
    });

    let transformed = anthropic_client().transform_chat_request(&request)?;
    let content = &transformed["messages"][0]["content"][0];

    assert_eq!(content["type"], "document");
    assert_eq!(content["cache_control"], json!({"type": "ephemeral"}));
    Ok(())
}

#[test]
fn issue_762_rejects_unsupported_audio_content_part() {
    let mut request = ChatRequest::new("claude-3-opus-20240229");
    request.messages.push(ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![ContentPart::Audio {
            audio: AudioData {
                data: "AAAA".to_string(),
                format: Some("mp3".to_string()),
            },
        }])),
        ..Default::default()
    });

    let message = match anthropic_client().transform_chat_request(&request) {
        Ok(_) => panic!("audio parts must fail closed"),
        Err(error) => error.to_string(),
    };

    assert!(message.contains("audio"));
    assert!(message.contains("does not support"));
}

#[test]
fn issue_762_rejects_non_text_system_content_part() {
    let mut request = ChatRequest::new("claude-3-opus-20240229");
    request.messages.push(ChatMessage {
        role: MessageRole::System,
        content: Some(MessageContent::Parts(vec![ContentPart::Audio {
            audio: AudioData {
                data: "AAAA".to_string(),
                format: Some("mp3".to_string()),
            },
        }])),
        ..Default::default()
    });

    let message = match anthropic_client().transform_chat_request(&request) {
        Ok(_) => panic!("system audio parts must fail closed"),
        Err(error) => error.to_string(),
    };

    assert!(message.contains("audio"));
}

#[test]
fn issue_762_rejects_invalid_assistant_tool_call_json() {
    let mut request = ChatRequest::new("claude-3-opus-20240229");
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        tool_calls: Some(vec![ToolCall {
            id: "toolu_bad".to_string(),
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: "weather.lookup".to_string(),
                arguments: "{bad json".to_string(),
            },
        }]),
        ..Default::default()
    });

    let message = match anthropic_client().transform_chat_request(&request) {
        Ok(_) => panic!("invalid assistant tool-call JSON must fail closed"),
        Err(error) => error.to_string(),
    };

    assert!(message.contains("toolu_bad"));
    assert!(message.contains("valid JSON"));
}

#[test]
fn issue_762_preserves_tool_use_and_tool_result_content_parts()
-> Result<(), crate::core::providers::unified_provider::ProviderError> {
    let mut request = ChatRequest::new("claude-3-opus-20240229");
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Parts(vec![ContentPart::ToolUse {
            id: "toolu_123".to_string(),
            name: "weather.lookup".to_string(),
            input: json!({"city": "Paris"}),
        }])),
        ..Default::default()
    });
    request.messages.push(ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![ContentPart::ToolResult {
            tool_use_id: "toolu_123".to_string(),
            content: json!("sunny"),
            is_error: Some(false),
        }])),
        ..Default::default()
    });

    let transformed = anthropic_client().transform_chat_request(&request)?;
    let tool_use = &transformed["messages"][0]["content"][0];
    let tool_result = &transformed["messages"][1]["content"][0];

    assert_eq!(tool_use["type"], "tool_use");
    assert_eq!(tool_use["name"], "weather_lookup");
    assert_eq!(tool_use["input"], json!({"city": "Paris"}));
    assert_eq!(tool_result["type"], "tool_result");
    assert_eq!(tool_result["tool_use_id"], "toolu_123");
    assert_eq!(tool_result["is_error"], false);
    Ok(())
}

#[test]
fn issue_802_rejects_rich_tool_use_alias_against_declared_tools() {
    let mut request = ChatRequest::new("claude-3-opus-20240229")
        .add_user_message("weather?")
        .with_tools(vec![tool("weather_lookup")]);
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Parts(vec![ContentPart::ToolUse {
            id: "toolu_123".to_string(),
            name: "weather.lookup".to_string(),
            input: json!({"city": "Paris"}),
        }])),
        ..Default::default()
    });

    let message = match anthropic_client().transform_chat_request(&request) {
        Ok(_) => panic!("rich tool-use aliases must fail closed"),
        Err(error) => error.to_string(),
    };

    assert!(message.contains("Tool use"));
    assert!(message.contains("weather.lookup"));
    assert!(message.contains("weather_lookup"));
}

#[test]
fn issue_802_preserves_multimodal_tool_role_result_content()
-> Result<(), crate::core::providers::unified_provider::ProviderError> {
    let mut request = ChatRequest::new("claude-3-opus-20240229");
    request.messages.push(ChatMessage {
        role: MessageRole::Tool,
        tool_call_id: Some("toolu_123".to_string()),
        content: Some(MessageContent::Parts(vec![
            ContentPart::Text {
                text: "screenshot".to_string(),
            },
            ContentPart::Image {
                source: ImageSource {
                    media_type: "image/png".to_string(),
                    data: "iVBORw0KGgo=".to_string(),
                },
                detail: None,
                image_url: None,
            },
            ContentPart::Document {
                source: DocumentSource {
                    media_type: "application/pdf".to_string(),
                    data: "JVBERi0=".to_string(),
                },
                cache_control: Some(CacheControl {
                    cache_type: "ephemeral".to_string(),
                }),
            },
        ])),
        ..Default::default()
    });

    let transformed = anthropic_client().transform_chat_request(&request)?;
    let tool_result = &transformed["messages"][0]["content"][0];
    let blocks = tool_result["content"].as_array().unwrap();

    assert_eq!(tool_result["type"], "tool_result");
    assert_eq!(tool_result["tool_use_id"], "toolu_123");
    assert_eq!(blocks[0], json!({"type": "text", "text": "screenshot"}));
    assert_eq!(blocks[1]["type"], "image");
    assert_eq!(blocks[1]["source"]["media_type"], "image/png");
    assert_eq!(blocks[2]["type"], "document");
    assert_eq!(blocks[2]["cache_control"], json!({"type": "ephemeral"}));
    Ok(())
}

#[test]
fn issue_802_rejects_cache_control_on_models_without_cache_support() {
    let mut request = ChatRequest::new("claude-2.1").add_user_message("hello");
    request
        .extra_params
        .insert("cache_control".to_string(), json!({"type": "ephemeral"}));

    let message = match anthropic_client().transform_chat_request(&request) {
        Ok(_) => panic!("cache_control must fail closed for unsupported known models"),
        Err(error) => error.to_string(),
    };

    assert!(message.contains("claude-2.1"));
    assert!(message.contains("cache control"));
}

#[test]
fn provider_rejects_internal_order_without_thinking_payload() {
    let mut request = ChatRequest::new("claude-3-opus-20240229");
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text("visible".to_string())),
        ..Default::default()
    });
    let malformed = ChatMessageContinuation::new()
        .with_anthropic_block_order(vec![AnthropicContentBlockOrder::Text { start: 0, end: 7 }]);

    let error = anthropic_client()
        .transform_chat_request_with_extensions(&request, &[malformed])
        .expect_err("provider boundary must reject internally malformed order metadata");
    assert!(error.to_string().contains("non-empty Anthropic thinking"));
}

#[test]
fn issue_802_adds_extended_cache_beta_for_one_hour_cache_control() {
    let mut request = ChatRequest::new("claude-3-opus-20240229").add_user_message("hello");
    request.extra_params.insert(
        "cache_control".to_string(),
        json!({"type": "ephemeral", "ttl": "1h"}),
    );

    let headers = anthropic_client().compute_beta_headers(&request);
    let beta = headers
        .iter()
        .find(|(name, _)| name.as_ref() == "anthropic-beta")
        .map(|(_, value)| value.as_ref())
        .unwrap_or("");

    assert!(beta.contains(EXTENDED_CACHE_TTL_BETA));
}
