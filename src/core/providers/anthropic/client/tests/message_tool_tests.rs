use super::*;
use crate::core::types::anthropic_continuation::{
    AnthropicRedactedData, AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent,
    ChatMessageExtensions,
};

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
fn message_carrier_preserves_signed_redacted_and_tool_order() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let mut request = ChatRequest::new("claude-opus-4-8");
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        tool_calls: Some(vec![crate::core::types::tools::ToolCall {
            id: "toolu_1".to_string(),
            tool_type: "function".to_string(),
            function: crate::core::types::tools::FunctionCall {
                name: "lookup".to_string(),
                arguments: "{}".to_string(),
            },
        }]),
        ..Default::default()
    });
    let extension =
        ChatMessageExtensions::new().with_anthropic_thinking(AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking: "plan".to_string(),
                signature: AnthropicSignature::try_from("opaque-signature").unwrap(),
            },
            AnthropicThinkingBlock::RedactedThinking {
                data: AnthropicRedactedData::try_from("opaque-redacted").unwrap(),
            },
        ]));

    let transformed = client
        .transform_chat_request_with_extensions(&request, &[extension])
        .expect("typed continuation should transform");
    let content = transformed["messages"][0]["content"]
        .as_array()
        .expect("assistant content blocks");
    assert_eq!(content[0]["type"], "thinking");
    assert_eq!(content[0]["signature"], "opaque-signature");
    assert_eq!(content[1]["type"], "redacted_thinking");
    assert_eq!(content[1]["data"], "opaque-redacted");
    assert_eq!(content[2]["type"], "tool_use");
    assert_eq!(content[2]["id"], "toolu_1");
}

#[test]
fn message_carrier_normalizes_visible_and_empty_string_content() {
    let client = AnthropicClient::new(AnthropicConfig::new_test("test-key")).unwrap();
    for (content, expected_len) in [(Some("visible answer"), 2), (None, 1)] {
        let mut request = ChatRequest::new("claude-opus-4-8");
        request.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: content.map(|text| MessageContent::Text(text.to_string())),
            ..Default::default()
        });
        let extension = ChatMessageExtensions::new().with_anthropic_thinking(
            AnthropicThinkingContent::new(vec![AnthropicThinkingBlock::Thinking {
                thinking: "plan".to_string(),
                signature: AnthropicSignature::try_from("opaque-signature").unwrap(),
            }]),
        );

        let transformed = client
            .transform_chat_request_with_extensions(&request, &[extension])
            .expect("string content must normalize to Anthropic content blocks");
        let blocks = transformed["messages"][0]["content"]
            .as_array()
            .expect("normalized content block array");
        assert_eq!(blocks.len(), expected_len);
        assert_eq!(blocks[0]["type"], "thinking");
        if content.is_some() {
            assert_eq!(
                blocks[1],
                serde_json::json!({"type": "text", "text": "visible answer"})
            );
        }
    }
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
        .transform_messages(
            messages,
            "claude-3-opus-20240229",
            Some(model_spec),
            &Default::default(),
        )
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
        .transform_messages(
            messages,
            "claude-3-opus-20240229",
            Some(model_spec),
            &Default::default(),
        )
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
    let result = client
        .transform_tool_choice(&tool_choice, &Default::default())
        .unwrap();

    assert_eq!(result["type"], "auto");
}

#[test]
fn test_transform_tool_choice_none() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let tool_choice = crate::core::types::tools::ToolChoice::String("none".to_string());
    let result = client
        .transform_tool_choice(&tool_choice, &Default::default())
        .unwrap();

    assert_eq!(result["type"], "none");
}

#[test]
fn test_transform_tool_choice_required() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();

    let tool_choice = crate::core::types::tools::ToolChoice::String("required".to_string());
    let result = client
        .transform_tool_choice(&tool_choice, &Default::default())
        .unwrap();

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
            name: "get.weather forecast".to_string(),
            description: Some("Get weather for a location".to_string()),
            parameters: Some(json!({"type": "object"})),
        },
    }];

    let result = client.transform_tools(&tools).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["name"], "get_weather_forecast");
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
