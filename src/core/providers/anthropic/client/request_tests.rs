use serde_json::json;

use super::*;
use crate::core::providers::anthropic::config::AnthropicConfig;
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::content::{
    AudioData, CacheControl, ContentPart, DocumentSource, ImageSource,
};
use crate::core::types::message::{MessageContent, MessageRole};
use crate::core::types::thinking::{ThinkingConfig, ThinkingEffort};
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
fn reasoning_effort_maps_once_to_adaptive_output_config() {
    let mut request = ChatRequest::new("claude-fable-5").add_user_message("solve");
    request.reasoning_effort = Some("high".to_string());

    let transformed = anthropic_client()
        .transform_chat_request(&request)
        .expect("exact Claude 5 reasoning must be supported");

    assert_eq!(transformed["thinking"]["type"], "adaptive");
    assert_eq!(transformed["output_config"], json!({"effort": "high"}));
    assert_eq!(
        transformed
            .as_object()
            .expect("request is an object")
            .keys()
            .filter(|key| key.as_str() == "output_config")
            .count(),
        1
    );
}

#[test]
fn claude5_extended_reasoning_efforts_map_without_downshifting() {
    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        for effort in ["xhigh", "max"] {
            let mut request = ChatRequest::new(model).add_user_message("solve");
            request.reasoning_effort = Some(effort.to_string());

            let (thinking, output_effort) = AnthropicClient::claude_5_thinking_config(&request)
                .expect("official Claude 5 effort must be accepted")
                .expect("reasoning effort enables adaptive thinking");

            assert_eq!(thinking["type"], "adaptive");
            assert_eq!(output_effort.expect("effort").as_str(), effort);
        }
    }
}

#[test]
fn fable_defaults_to_always_on_adaptive_while_registered_opus_and_sonnet_remain_optional() {
    let client = anthropic_client();

    let fable = client
        .transform_chat_request(&ChatRequest::new("claude-fable-5").add_user_message("solve"))
        .expect("Fable must default to adaptive thinking");
    assert_eq!(
        fable["thinking"],
        json!({"type": "adaptive", "display": "summarized"})
    );

    for model in ["claude-opus-5", "claude-sonnet-5"] {
        let request = ChatRequest::new(model).add_user_message("solve");
        assert!(
            AnthropicClient::claude_5_thinking_config(&request)
                .expect("registered optional-thinking Claude 5 semantics remain valid")
                .is_none()
        );
    }

    let mut disabled = ChatRequest::new("claude-fable-5").add_user_message("solve");
    disabled.thinking = Some(ThinkingConfig::new());
    assert!(client.transform_chat_request(&disabled).is_err());
}

#[test]
fn reasoning_effort_invalid_conflicting_and_non_claude5_fail_closed() {
    let client = anthropic_client();

    let mut invalid = ChatRequest::new("claude-fable-5").add_user_message("solve");
    invalid.reasoning_effort = Some("maximum".to_string());
    assert!(client.transform_chat_request(&invalid).is_err());

    let mut conflict = ChatRequest::new("claude-fable-5").add_user_message("solve");
    conflict.reasoning_effort = Some("high".to_string());
    conflict.thinking = Some(
        ThinkingConfig::new()
            .enabled()
            .with_effort(ThinkingEffort::Low),
    );
    assert!(client.transform_chat_request(&conflict).is_err());

    let mut manual = ChatRequest::new("claude-fable-5").add_user_message("solve");
    manual.reasoning_effort = Some("medium".to_string());
    manual.thinking = Some(ThinkingConfig::new().enabled().with_budget(1024));
    assert!(client.transform_chat_request(&manual).is_err());

    let mut old_model = ChatRequest::new("claude-3-opus-20240229").add_user_message("solve");
    old_model.reasoning_effort = Some("high".to_string());
    assert!(client.transform_chat_request(&old_model).is_err());
}

#[test]
fn claude5_legacy_function_calls_error_while_modern_tools_remain_valid() {
    let client = anthropic_client();
    let mut legacy = ChatRequest::new("claude-fable-5").add_user_message("weather?");
    legacy.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        function_call: Some(FunctionCall {
            name: "lookup".to_string(),
            arguments: "{}".to_string(),
        }),
        ..Default::default()
    });
    let error = client
        .transform_chat_request(&legacy)
        .expect_err("legacy function_call must fail");
    assert!(error.to_string().contains("tools/tool_choice"));

    let modern = ChatRequest::new("claude-fable-5")
        .add_user_message("weather?")
        .with_tools(vec![tool("lookup")]);
    assert!(client.transform_chat_request(&modern).is_ok());
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
fn claude_fable_5_accepts_official_prompt_cache_control_but_lookalikes_fail() {
    let mut request = ChatRequest::new("claude-fable-5").add_user_message("hello");
    request
        .extra_params
        .insert("cache_control".to_string(), json!({"type": "ephemeral"}));

    let transformed = anthropic_client()
        .transform_chat_request(&request)
        .expect("Fable 5 prompt caching is supported");
    assert_eq!(transformed["cache_control"], json!({"type": "ephemeral"}));

    request.model = "claude-fable-5-preview".to_string();
    assert!(anthropic_client().transform_chat_request(&request).is_err());
}

#[test]
fn claude_fable_5_rejects_unsupported_sampling_controls() {
    let client = anthropic_client();

    let mut temperature = ChatRequest::new("claude-fable-5").add_user_message("hello");
    temperature.temperature = Some(0.2);
    let error = client
        .transform_chat_request(&temperature)
        .expect_err("Fable must reject temperature");
    assert!(error.to_string().contains("temperature"));

    let mut top_p = ChatRequest::new("claude-fable-5").add_user_message("hello");
    top_p.top_p = Some(0.8);
    let error = client
        .transform_chat_request(&top_p)
        .expect_err("Fable must reject top_p");
    assert!(error.to_string().contains("top_p"));
}

#[test]
fn claude_fable_5_preserves_document_cache_control() {
    let mut request = ChatRequest::new("claude-fable-5");
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

    let transformed = anthropic_client()
        .transform_chat_request(&request)
        .expect("Fable 5 document caching is supported");
    assert_eq!(
        transformed["messages"][0]["content"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
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
