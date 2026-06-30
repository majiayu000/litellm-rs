use super::*;
use crate::core::providers::anthropic::config::AnthropicConfig;
use crate::core::types::message::MessageContent;
use crate::core::types::thinking::ThinkingContent;

// ==================== Client Creation Tests ====================

#[test]
fn test_client_creation() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config);
    assert!(client.is_ok());
}

#[test]
fn test_client_creation_with_custom_config() {
    let mut config = AnthropicConfig::new_test("test-key");
    config.request_timeout = 120;
    config.connect_timeout = 30;
    let client = AnthropicClient::new(config);
    assert!(client.is_ok());
}

// ==================== Header Building Tests ====================

/// Helper to check if a header key exists in Vec<HeaderPair>
fn has_header(headers: &[HeaderPair], key: &str) -> bool {
    headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(key))
}

/// Helper to get a header value from Vec<HeaderPair>
fn get_header<'a>(headers: &'a [HeaderPair], key: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.as_ref())
}

#[test]
fn test_header_building() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let headers = client.get_request_headers();

    // Anthropic uses x-api-key header instead of Authorization
    assert!(has_header(&headers, "x-api-key"));
    assert!(has_header(&headers, "anthropic-version"));
    assert!(has_header(&headers, "Content-Type"));
    assert!(has_header(&headers, "User-Agent"));
}

#[test]
fn test_header_content_type() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let headers = client.get_request_headers();

    assert_eq!(
        get_header(&headers, "Content-Type").unwrap(),
        "application/json"
    );
}

#[test]
fn test_header_user_agent() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let headers = client.get_request_headers();

    assert_eq!(
        get_header(&headers, "User-Agent").unwrap(),
        "LiteLLM-Rust/1.0"
    );
}

#[test]
fn test_header_with_custom_headers() {
    let mut config = AnthropicConfig::new_test("test-key");
    config
        .custom_headers
        .insert("X-Custom-Header".to_string(), "custom-value".to_string());
    let client = AnthropicClient::new(config).unwrap();
    let headers = client.get_request_headers();

    assert!(has_header(&headers, "X-Custom-Header"));
}

// ==================== Error Mapping Tests ====================

#[test]
fn test_map_http_error_400() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(400, "invalid request");

    // Should return an API error for 400
    let error_string = format!("{}", error);
    assert!(error_string.contains("400") || error_string.contains("request"));
}

#[test]
fn test_map_http_error_401() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(401, "unauthorized");

    // Should return an authentication error
    let error_string = format!("{}", error);
    assert!(error_string.to_lowercase().contains("auth") || error_string.contains("key"));
}

#[test]
fn test_map_http_error_403() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(403, "forbidden");

    // Should return an authentication error
    let error_string = format!("{}", error);
    assert!(
        error_string.to_lowercase().contains("forbidden")
            || error_string.to_lowercase().contains("permission")
            || error_string.to_lowercase().contains("auth")
    );
}

#[test]
fn test_map_http_error_404() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(404, "not found");

    let error_string = format!("{}", error);
    assert!(error_string.contains("404") || error_string.contains("not found"));
}

#[test]
fn test_map_http_error_429() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(429, "rate limited");

    // Should return a rate limit error
    let error_string = format!("{}", error);
    assert!(
        error_string.to_lowercase().contains("rate")
            || error_string.to_lowercase().contains("limit")
    );
}

#[test]
fn test_map_http_error_500() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(500, "server error");

    let error_string = format!("{}", error);
    assert!(error_string.contains("500") || error_string.to_lowercase().contains("server"));
}

// ==================== Retry-After Extraction Tests ====================

#[test]
fn test_extract_retry_after_from_root() {
    let body = r#"{"retry_after": 60}"#;
    let retry = parse_retry_after_from_body(body);
    assert_eq!(retry, Some(60));
}

#[test]
fn test_extract_retry_after_from_error() {
    let body = r#"{"error": {"retry_after": 30}}"#;
    let retry = parse_retry_after_from_body(body);
    assert_eq!(retry, Some(30));
}

#[test]
fn test_extract_retry_after_missing() {
    let body = r#"{"message": "no retry info"}"#;
    let retry = parse_retry_after_from_body(body);
    assert!(retry.is_none());
}

#[test]
fn test_extract_retry_after_invalid_json() {
    let body = "not json";
    let retry = parse_retry_after_from_body(body);
    assert!(retry.is_none());
}

// ==================== System Message Separation Tests ====================

#[test]
fn test_separate_system_messages_no_system() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("Hello".to_string())),
        name: None,
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
        thinking: None,
        audio: None,
    }];

    let (system, user_msgs) = client.separate_system_messages(&messages).unwrap();
    assert!(system.is_none());
    assert_eq!(user_msgs.len(), 1);
}

#[test]
fn test_separate_system_messages_with_system() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let messages = vec![
        ChatMessage {
            role: MessageRole::System,
            content: Some(MessageContent::Text(
                "You are a helpful assistant.".to_string(),
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
            thinking: None,
            audio: None,
        },
        ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
            thinking: None,
            audio: None,
        },
    ];

    let (system, user_msgs) = client.separate_system_messages(&messages).unwrap();
    assert_eq!(system, Some("You are a helpful assistant.".to_string()));
    assert_eq!(user_msgs.len(), 1);
}

#[test]
fn test_separate_system_messages_multiple_system() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let messages = vec![
        ChatMessage {
            role: MessageRole::System,
            content: Some(MessageContent::Text("Rule 1".to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
            thinking: None,
            audio: None,
        },
        ChatMessage {
            role: MessageRole::System,
            content: Some(MessageContent::Text("Rule 2".to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
            thinking: None,
            audio: None,
        },
    ];

    let (system, _) = client.separate_system_messages(&messages).unwrap();
    assert_eq!(system, Some("Rule 1\nRule 2".to_string()));
}

#[test]
fn test_anthropic_transform_messages_preserves_assistant_text_with_tool_use() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let model_spec = get_anthropic_registry()
        .get_model_spec("claude-3-opus-20240229")
        .unwrap();

    let messages = vec![ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text("I'll check the weather.".to_string())),
        name: None,
        tool_calls: Some(vec![crate::core::types::tools::ToolCall {
            id: "toolu_123".to_string(),
            tool_type: "function".to_string(),
            function: crate::core::types::tools::FunctionCall {
                name: "get_weather".to_string(),
                arguments: r#"{"location":"San Francisco"}"#.to_string(),
            },
        }]),
        tool_call_id: None,
        function_call: None,
        thinking: None,
        audio: None,
    }];

    let transformed = client
        .transform_messages(messages, Some(model_spec))
        .unwrap();
    assert_eq!(transformed[0]["role"], "assistant");
    let content = transformed[0]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "I'll check the weather.");
    assert_eq!(content[1]["type"], "tool_use");
    assert_eq!(content[1]["id"], "toolu_123");
    assert_eq!(content[1]["name"], "get_weather");
    assert_eq!(content[1]["input"]["location"], "San Francisco");
}

#[test]
fn test_anthropic_transform_messages_tool_role_to_tool_result() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let model_spec = get_anthropic_registry()
        .get_model_spec("claude-3-opus-20240229")
        .unwrap();

    let messages = vec![ChatMessage {
        role: MessageRole::Tool,
        content: Some(MessageContent::Text(r#"{"temperature":"68F"}"#.to_string())),
        name: None,
        tool_calls: None,
        tool_call_id: Some("toolu_123".to_string()),
        function_call: None,
        thinking: None,
        audio: None,
    }];

    let transformed = client
        .transform_messages(messages, Some(model_spec))
        .unwrap();
    assert_eq!(transformed[0]["role"], "user");
    let content = transformed[0]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "tool_result");
    assert_eq!(content[0]["tool_use_id"], "toolu_123");
    assert_eq!(content[0]["content"], r#"{"temperature":"68F"}"#);
}

// ==================== Tool Choice Transformation Tests ====================

#[test]
fn test_transform_tool_choice_auto() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let tool_choice = crate::core::types::tools::ToolChoice::String("auto".to_string());
    let result = client.transform_tool_choice(&tool_choice).unwrap();

    assert_eq!(result["type"], "auto");
}

#[test]
fn test_transform_tool_choice_none() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let tool_choice = crate::core::types::tools::ToolChoice::String("none".to_string());
    let result = client.transform_tool_choice(&tool_choice).unwrap();

    assert_eq!(result["type"], "none");
}

#[test]
fn test_transform_tool_choice_required() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let tool_choice = crate::core::types::tools::ToolChoice::String("required".to_string());
    let result = client.transform_tool_choice(&tool_choice).unwrap();

    assert_eq!(result["type"], "any");
}

// ==================== Tool Transformation Tests ====================

#[test]
fn test_transform_tools() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let tools = vec![crate::core::types::tools::Tool {
        tool_type: crate::core::types::tools::ToolType::Function,
        function: crate::core::types::tools::FunctionDefinition {
            name: "get_weather".to_string(),
            description: Some("Get weather for a location".to_string()),
            parameters: Some(json!({"type": "object"})),
        },
    }];

    let result = client.transform_tools(&tools).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["name"], "get_weather");
    assert_eq!(result[0]["description"], "Get weather for a location");
}

#[test]
fn test_transform_tools_empty() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let tools: Vec<crate::core::types::tools::Tool> = vec![];
    let result = client.transform_tools(&tools).unwrap();
    assert!(result.is_empty());
}

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
                "thinking": "Second thought."
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
            signature: Some("sig_123".to_string()),
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

// ==================== Unsupported Parameter Tests ====================

#[test]
fn test_transform_chat_request_rejects_n_greater_than_one() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let mut request = ChatRequest::new("claude-opus-4-6").add_user_message("Hello");
    request.n = Some(3);

    let error = client.transform_chat_request(&request).unwrap_err();
    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    assert!(
        format!("{}", error).contains("only supports n=1"),
        "unexpected error message"
    );
}

#[test]
fn test_transform_chat_request_rejects_n_zero() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    // n=0 is as unsatisfiable as n>1: the API always returns one candidate.
    let mut request = ChatRequest::new("claude-opus-4-6").add_user_message("Hello");
    request.n = Some(0);

    let error = client.transform_chat_request(&request).unwrap_err();
    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
}

#[test]
fn test_transform_chat_request_allows_n_equal_one_and_ignores_unsupported_params() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    // n=1 plus unsupported-but-ignorable params must not fail the request
    let mut request = ChatRequest::new("claude-opus-4-6").add_user_message("Hello");
    request.n = Some(1);
    request.frequency_penalty = Some(0.5);
    request.presence_penalty = Some(0.5);
    request.seed = Some(42);

    let result = client.transform_chat_request(&request).unwrap();
    assert_eq!(result["model"], "claude-opus-4-6");
    // Ignored params must not leak into the Anthropic request body
    assert!(result.get("frequency_penalty").is_none());
    assert!(result.get("seed").is_none());
}

#[test]
fn test_transform_chat_request_allows_unknown_model_when_configured() {
    let config = AnthropicConfig::new_test("test-key")
        .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
        .with_allow_unknown_models(true)
        .with_configured_models(vec!["mimo-v2.5".to_string()]);
    let client = match AnthropicClient::new(config) {
        Ok(client) => client,
        Err(err) => panic!("client should build: {err}"),
    };

    let request = ChatRequest::new("mimo-v2.5").add_user_message("Hello");

    let result = match client.transform_chat_request(&request) {
        Ok(result) => result,
        Err(err) => panic!("configured compatible model should accept image input: {err}"),
    };
    assert_eq!(result["model"], "mimo-v2.5");
    assert_eq!(result["max_tokens"], 4096);
}

#[test]
fn test_transform_chat_request_allows_configured_unknown_model_image_input() {
    let config = AnthropicConfig::new_test("test-key")
        .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
        .with_allow_unknown_models(true)
        .with_configured_models(vec!["mimo-v2.5".to_string()])
        .with_configured_multimodal_models(vec!["mimo-v2.5".to_string()]);
    let client = AnthropicClient::new(config).unwrap();

    let request = ChatRequest::new("mimo-v2.5").add_message(
        crate::core::types::message::MessageRole::User,
        crate::core::types::message::MessageContent::Parts(vec![
            crate::core::types::content::ContentPart::Text {
                text: "Describe this image".to_string(),
            },
            crate::core::types::content::ContentPart::ImageUrl {
                image_url: crate::core::types::content::ImageUrl {
                    url: "data:image/png;base64,ZmFrZQ==".to_string(),
                    detail: None,
                },
            },
        ]),
    );

    let result = client.transform_chat_request(&request).unwrap();
    assert_eq!(result["model"], "mimo-v2.5");
    assert_eq!(result["messages"][0]["content"][1]["type"], "image");
}

#[test]
fn test_transform_chat_request_rejects_unknown_model_by_default() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let request = ChatRequest::new("mimo-v2.5").add_user_message("Hello");

    let error = client.transform_chat_request(&request).unwrap_err();
    assert!(format!("{error}").contains("Unsupported model: mimo-v2.5"));
}

#[test]
fn test_transform_chat_response_preserves_cache_details_without_double_counting() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let response = json!({
        "id": "msg_123",
        "model": "claude-3-opus-20240229",
        "content": [{"type": "text", "text": "Hi"}],
        "usage": {
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_creation_input_tokens": 30,
            "cache_read_input_tokens": 20
        }
    });

    let usage = client
        .transform_chat_response(response)
        .unwrap()
        .usage
        .unwrap();
    assert_eq!(usage.prompt_tokens, 150);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 200);
}
