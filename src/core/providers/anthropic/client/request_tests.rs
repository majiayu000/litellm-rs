use serde_json::json;

use super::*;
use crate::core::providers::anthropic::config::AnthropicConfig;
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::content::{
    AudioData, CacheControl, ContentPart, DocumentSource, ImageSource,
};
use crate::core::types::message::{MessageContent, MessageRole};
use crate::core::types::thinking::{
    AnthropicThinkingBlock, AnthropicThinkingContent, ThinkingConfig, ThinkingContent,
};
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
fn current_claude_5_protocol_serializes_adaptive_disabled_and_sampling_rules() {
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        let mut request = ChatRequest::new(model).add_user_message("Solve this");
        request.thinking = Some(ThinkingConfig::medium_effort());
        let transformed = anthropic_client()
            .transform_chat_request(&request)
            .expect("exact Claude 5 protocol IDs use adaptive thinking without catalog entries");
        assert_eq!(transformed["thinking"]["type"], "adaptive");
        assert_eq!(transformed["thinking"]["display"], "summarized");
        assert_eq!(transformed["output_config"]["effort"], "medium");

        let mut top_k = ChatRequest::new(model).add_user_message("Hello");
        top_k.extra_params.insert("top_k".to_string(), json!(1));
        assert!(
            anthropic_client()
                .transform_chat_request(&top_k)
                .expect_err("Claude 5 rejects every top_k")
                .to_string()
                .contains("top_k")
        );
    }

    for top_p in [0.99, 1.0] {
        let mut compatible = ChatRequest::new("claude-opus-5").add_user_message("Hello");
        compatible.temperature = Some(1.0);
        compatible.top_p = Some(top_p);
        assert!(
            anthropic_client()
                .transform_chat_request(&compatible)
                .is_ok()
        );
    }

    let mut omitted = ChatRequest::new("claude-opus-5").add_user_message("Hello");
    omitted.thinking = Some(ThinkingConfig::medium_effort().include_in_response(false));
    let transformed = anthropic_client()
        .transform_chat_request(&omitted)
        .expect("adaptive thinking supports omitted display");
    assert_eq!(transformed["thinking"]["display"], "omitted");

    let mut fable_disabled = ChatRequest::new("claude-fable-5").add_user_message("Hello");
    fable_disabled.thinking = Some(ThinkingConfig::default());
    assert!(
        anthropic_client()
            .transform_chat_request(&fable_disabled)
            .expect_err("Fable thinking is always on")
            .to_string()
            .contains("cannot disable thinking")
    );

    for model in ["claude-opus-5", "claude-sonnet-5"] {
        let mut disabled = ChatRequest::new(model).add_user_message("Hello");
        disabled.thinking = Some(ThinkingConfig::default());
        let transformed = anthropic_client()
            .transform_chat_request(&disabled)
            .expect("Opus and Sonnet can disable thinking");
        assert_eq!(transformed["thinking"], json!({"type": "disabled"}));
    }
}

#[test]
fn current_claude_5_protocol_fails_closed_on_unsupported_request_shapes() {
    let mut temperature = ChatRequest::new("claude-opus-5").add_user_message("Hello");
    temperature.temperature = Some(0.5);
    assert!(
        anthropic_client()
            .transform_chat_request(&temperature)
            .expect_err("non-default temperature must fail")
            .to_string()
            .contains("temperature")
    );

    let mut top_p = ChatRequest::new("claude-opus-5").add_user_message("Hello");
    top_p.top_p = Some(0.5);
    assert!(
        anthropic_client()
            .transform_chat_request(&top_p)
            .expect_err("non-compatible top_p must fail")
            .to_string()
            .contains("top_p")
    );

    let prefill = ChatRequest::new("claude-opus-5")
        .add_user_message("Choose")
        .add_assistant_message("The answer is");
    assert!(
        anthropic_client()
            .transform_chat_request(&prefill)
            .expect_err("assistant prefill must fail")
            .to_string()
            .contains("assistant prefill")
    );

    let mut legacy = ChatRequest::new("claude-opus-5").add_user_message("lookup");
    legacy.functions = Some(vec![json!({"name":"lookup"})]);
    assert!(
        anthropic_client()
            .transform_chat_request(&legacy)
            .expect_err("legacy function fields must fail")
            .to_string()
            .contains("legacy functions")
    );

    let mut budget = ChatRequest::new("claude-opus-5").add_user_message("Solve");
    budget.thinking = Some(ThinkingConfig::new().enabled().with_budget(4_096));
    assert!(
        anthropic_client()
            .transform_chat_request(&budget)
            .expect_err("adaptive thinking does not accept manual budgets")
            .to_string()
            .contains("budget_tokens")
    );
}

#[test]
fn adaptive_thinking_allows_forced_tools_but_manual_thinking_rejects_them() {
    for tool_choice in [
        ToolChoice::String("required".to_string()),
        ToolChoice::Specific {
            choice_type: "function".to_string(),
            function: Some(FunctionChoice {
                name: "lookup".to_string(),
            }),
        },
    ] {
        let mut adaptive = ChatRequest::new("claude-opus-5")
            .add_user_message("lookup")
            .with_tools(vec![tool("lookup")]);
        adaptive.thinking = Some(ThinkingConfig::medium_effort());
        adaptive.tool_choice = Some(tool_choice.clone());
        let transformed = anthropic_client()
            .transform_chat_request(&adaptive)
            .expect("adaptive thinking supports forced tool use");
        assert!(matches!(
            transformed["tool_choice"]["type"].as_str(),
            Some("any" | "tool")
        ));

        let mut manual = ChatRequest::new("claude-sonnet-4-20250514")
            .add_user_message("lookup")
            .with_tools(vec![tool("lookup")]);
        manual.thinking = Some(ThinkingConfig::new().enabled().with_budget(2_048));
        manual.tool_choice = Some(tool_choice);
        assert!(
            anthropic_client()
                .transform_chat_request(&manual)
                .expect_err("manual thinking only supports auto or none")
                .to_string()
                .contains("tool_choice")
        );
    }
}

#[test]
fn typed_thinking_history_replays_every_block_before_tool_use() {
    let history = AnthropicThinkingContent::try_from(vec![
        AnthropicThinkingBlock::Thinking {
            thinking: "first".to_string(),
            signature: "sig-first".to_string(),
        },
        AnthropicThinkingBlock::RedactedThinking {
            data: "encrypted-payload".to_string(),
        },
        AnthropicThinkingBlock::Thinking {
            thinking: "second".to_string(),
            signature: "sig-second".to_string(),
        },
    ])
    .expect("valid signed history");
    let assistant = ChatMessage {
        role: MessageRole::Assistant,
        thinking: Some(ThinkingContent::AnthropicBlocks { content: history }),
        tool_calls: Some(vec![ToolCall {
            id: "toolu_1".to_string(),
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: "lookup".to_string(),
                arguments: r#"{"id":7}"#.to_string(),
            },
        }]),
        ..Default::default()
    };
    let tool_result = ChatMessage {
        role: MessageRole::Tool,
        content: Some(MessageContent::Text("done".to_string())),
        tool_call_id: Some("toolu_1".to_string()),
        ..Default::default()
    };
    let mut request = ChatRequest::new("claude-opus-5").with_tools(vec![tool("lookup")]);
    request.messages = vec![assistant, tool_result];

    let transformed = anthropic_client()
        .transform_chat_request(&request)
        .expect("lossless thinking history is replayable");
    let blocks = transformed["messages"][0]["content"]
        .as_array()
        .expect("assistant content blocks");
    assert_eq!(
        blocks[0],
        json!({"type":"thinking","thinking":"first","signature":"sig-first"})
    );
    assert_eq!(
        blocks[1],
        json!({"type":"redacted_thinking","data":"encrypted-payload"})
    );
    assert_eq!(
        blocks[2],
        json!({"type":"thinking","thinking":"second","signature":"sig-second"})
    );
    assert_eq!(blocks[3]["type"], "tool_use");
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
