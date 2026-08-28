use serde_json::json;

use super::*;
use crate::core::providers::anthropic::config::AnthropicConfig;
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::content::{
    AudioData, CacheControl, ContentPart, DocumentSource, ImageSource,
};
use crate::core::types::message::{MessageContent, MessageRole};
use crate::core::types::thinking::{AnthropicThinkingBlock, ThinkingConfig, ThinkingContent};
use crate::core::types::tools::{
    FunctionCall, FunctionChoice, FunctionDefinition, Tool, ToolCall, ToolChoice, ToolType,
};

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

#[test]
fn current_claude_5_rejects_non_default_sampling_parameters() {
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        let mut temperature_request = ChatRequest::new(model).add_user_message("Hello");
        temperature_request.temperature = Some(0.5);
        let temperature_error = anthropic_client()
            .transform_chat_request(&temperature_request)
            .expect_err("Claude 5 must reject non-default temperature");
        assert!(temperature_error.to_string().contains("temperature"));

        let mut top_p_request = ChatRequest::new(model).add_user_message("Hello");
        top_p_request.top_p = Some(0.5);
        let top_p_error = anthropic_client()
            .transform_chat_request(&top_p_request)
            .expect_err("Claude 5 must reject non-default top_p");
        assert!(top_p_error.to_string().contains("top_p"));
    }
}

#[test]
fn current_claude_5_accepts_sampling_compatibility_defaults() {
    for top_p in [0.99, 1.0] {
        let mut request = ChatRequest::new("claude-opus-5").add_user_message("Hello");
        request.temperature = Some(1.0);
        request.top_p = Some(top_p);

        let transformed = anthropic_client()
            .transform_chat_request(&request)
            .expect("Anthropic accepts top_p values at or above 0.99 for Claude 5");

        assert_eq!(transformed["temperature"], json!(1.0));
        assert!(
            (transformed["top_p"]
                .as_f64()
                .expect("top_p must be numeric")
                - f64::from(top_p))
            .abs()
                < 1e-6
        );
    }
}

#[test]
fn current_claude_5_rejects_assistant_prefill() {
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        let request = ChatRequest::new(model)
            .add_user_message("Choose A or B")
            .add_assistant_message("The answer is (");

        let error = anthropic_client()
            .transform_chat_request(&request)
            .expect_err("Claude 5 must reject assistant prefills locally");

        assert!(error.to_string().contains("assistant prefill"));
    }
}

#[test]
fn current_claude_5_serializes_adaptive_thinking_and_effort() {
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        let mut request = ChatRequest::new(model).add_user_message("Solve this carefully");
        request.thinking = Some(ThinkingConfig::medium_effort());

        let transformed = anthropic_client()
            .transform_chat_request(&request)
            .expect("Claude 5 must use adaptive thinking");

        assert_eq!(transformed["thinking"]["type"], "adaptive");
        assert_eq!(transformed["thinking"]["display"], "summarized");
        assert_eq!(transformed["output_config"]["effort"], "medium");
        assert!(transformed["thinking"].get("budget_tokens").is_none());
    }

    let mut omitted = ChatRequest::new("claude-opus-5").add_user_message("Solve this");
    omitted.thinking = Some(ThinkingConfig::medium_effort().include_in_response(false));
    let transformed = anthropic_client()
        .transform_chat_request(&omitted)
        .expect("Claude 5 supports omitted adaptive thinking display");
    assert_eq!(transformed["thinking"]["display"], "omitted");
}

#[test]
fn current_claude_5_rejects_manual_thinking_budgets() {
    let mut request = ChatRequest::new("claude-sonnet-5").add_user_message("Solve this");
    request.thinking = Some(ThinkingConfig::new().enabled().with_budget(4_096));

    let error = anthropic_client()
        .transform_chat_request(&request)
        .expect_err("Claude 5 does not support manual budget-token thinking");

    assert!(error.to_string().contains("budget_tokens"));
}

#[test]
fn current_claude_5_rejects_legacy_function_fields() {
    let mut functions_request = ChatRequest::new("claude-opus-5").add_user_message("lookup");
    functions_request.functions = Some(vec![json!({"name": "lookup"})]);
    let functions_error = anthropic_client()
        .transform_chat_request(&functions_request)
        .expect_err("Claude 5 must not silently drop legacy functions");
    assert!(functions_error.to_string().contains("legacy functions"));

    let mut function_call_request = ChatRequest::new("claude-opus-5").add_user_message("lookup");
    function_call_request.function_call = Some(json!({"name": "lookup"}));
    let function_call_error = anthropic_client()
        .transform_chat_request(&function_call_request)
        .expect_err("Claude 5 must not silently drop legacy function_call");
    assert!(function_call_error.to_string().contains("legacy functions"));
}

#[test]
fn current_claude_5_preserves_thinking_blocks_in_tool_continuations() {
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        let client = anthropic_client();
        let response = json!({
            "id": "msg_tool_loop",
            "model": model,
            "content": [
                {"type": "thinking", "thinking": "first", "signature": "sig-first"},
                {"type": "redacted_thinking", "data": "encrypted-payload"},
                {"type": "thinking", "thinking": "second", "signature": "sig-second"},
                {"type": "tool_use", "id": "toolu_1", "name": "lookup", "input": {"id": 7}}
            ],
            "stop_reason": "tool_use"
        });
        let response = client
            .transform_chat_response(response)
            .expect("Claude 5 tool response should parse");
        let assistant = response.choices[0].message.clone();
        assert_eq!(
            assistant.thinking,
            Some(ThinkingContent::AnthropicBlocks {
                blocks: vec![
                    AnthropicThinkingBlock::Thinking {
                        thinking: "first".to_string(),
                        signature: Some("sig-first".to_string()),
                    },
                    AnthropicThinkingBlock::RedactedThinking {
                        data: "encrypted-payload".to_string(),
                    },
                    AnthropicThinkingBlock::Thinking {
                        thinking: "second".to_string(),
                        signature: Some("sig-second".to_string()),
                    },
                ],
            })
        );

        let tool_result = ChatMessage {
            role: MessageRole::Tool,
            content: Some(MessageContent::Text("done".to_string())),
            tool_call_id: Some("toolu_1".to_string()),
            ..Default::default()
        };
        let mut request = ChatRequest::new(model).with_tools(vec![tool("lookup")]);
        request.messages = vec![assistant, tool_result];

        let transformed = client
            .transform_chat_request(&request)
            .expect("Claude 5 tool continuation should preserve thinking blocks");
        let blocks = transformed["messages"][0]["content"]
            .as_array()
            .expect("assistant content must be blocks");

        assert_eq!(
            blocks[0],
            json!({"type": "thinking", "thinking": "first", "signature": "sig-first"})
        );
        assert_eq!(
            blocks[1],
            json!({"type": "redacted_thinking", "data": "encrypted-payload"})
        );
        assert_eq!(
            blocks[2],
            json!({"type": "thinking", "thinking": "second", "signature": "sig-second"})
        );
        assert_eq!(blocks[3]["type"], "tool_use");
    }
}

#[test]
fn current_claude_5_serializes_or_rejects_explicit_disabled_thinking() {
    let mut fable = ChatRequest::new("claude-fable-5").add_user_message("Hello");
    fable.thinking = Some(ThinkingConfig::default());
    let error = anthropic_client()
        .transform_chat_request(&fable)
        .expect_err("Fable 5 thinking is always on");
    assert!(error.to_string().contains("cannot disable thinking"));

    for model in ["claude-opus-5", "claude-sonnet-5"] {
        let mut request = ChatRequest::new(model).add_user_message("Hello");
        request.thinking = Some(ThinkingConfig::default());
        let transformed = anthropic_client()
            .transform_chat_request(&request)
            .expect("Opus and Sonnet 5 can explicitly disable thinking");
        assert_eq!(transformed["thinking"], json!({"type": "disabled"}));
    }
}

#[test]
fn current_claude_5_rejects_every_top_k_value() {
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        let mut request = ChatRequest::new(model).add_user_message("Hello");
        request.extra_params.insert("top_k".to_string(), json!(1));
        let error = anthropic_client()
            .transform_chat_request(&request)
            .expect_err("Claude 5 does not accept top_k");
        assert!(error.to_string().contains("top_k"));
    }
}

#[test]
fn current_claude_5_rejects_forced_tools_only_while_thinking_is_active() {
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        let mut required = ChatRequest::new(model)
            .add_user_message("lookup")
            .with_tools(vec![tool("lookup")]);
        required.tool_choice = Some(ToolChoice::String("required".to_string()));
        let error = anthropic_client()
            .transform_chat_request(&required)
            .expect_err("default Claude 5 thinking forbids forced tools");
        assert!(error.to_string().contains("tool_choice"));

        let mut specific = ChatRequest::new(model)
            .add_user_message("lookup")
            .with_tools(vec![tool("lookup")]);
        specific.tool_choice = Some(ToolChoice::Specific {
            choice_type: "function".to_string(),
            function: Some(FunctionChoice {
                name: "lookup".to_string(),
            }),
        });
        let error = anthropic_client()
            .transform_chat_request(&specific)
            .expect_err("default Claude 5 thinking forbids specific tools");
        assert!(error.to_string().contains("tool_choice"));
    }

    let mut disabled = ChatRequest::new("claude-opus-5")
        .add_user_message("lookup")
        .with_tools(vec![tool("lookup")]);
    disabled.thinking = Some(ThinkingConfig::default());
    disabled.tool_choice = Some(ToolChoice::String("required".to_string()));
    let transformed = anthropic_client()
        .transform_chat_request(&disabled)
        .expect("forced tools are valid when Opus 5 thinking is explicitly disabled");
    assert_eq!(transformed["tool_choice"]["type"], "any");

    for choice in ["auto", "none"] {
        let mut request = ChatRequest::new("claude-sonnet-5")
            .add_user_message("lookup")
            .with_tools(vec![tool("lookup")]);
        request.tool_choice = Some(ToolChoice::String(choice.to_string()));
        let transformed = anthropic_client()
            .transform_chat_request(&request)
            .expect("default Claude 5 thinking supports auto and none tool choice");
        assert_eq!(transformed["tool_choice"]["type"], choice);
    }
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
