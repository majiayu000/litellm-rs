use super::*;

fn create_test_message(role: MessageRole, content: &str) -> ChatMessage {
    ChatMessage {
        role,
        content: Some(MessageContent::Text(content.to_string())),
        thinking: None,
        audio: None,
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
    }
}

fn create_test_request() -> ChatRequest {
    ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![create_test_message(MessageRole::User, "Hello")],
        ..Default::default()
    }
}

#[test]
fn test_azure_chat_utils_validate_request_valid() {
    let request = create_test_request();
    assert!(AzureChatUtils::validate_request(&request).is_ok());
}

#[test]
fn test_azure_chat_utils_validate_request_empty_messages() {
    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        ..Default::default()
    };
    let result = AzureChatUtils::validate_request(&request);
    assert!(result.is_err());
}

#[test]
fn test_supports_functions_gpt4() {
    assert!(AzureChatUtils::supports_functions("gpt-4"));
    assert!(AzureChatUtils::supports_functions("gpt-4-32k"));
    assert!(AzureChatUtils::supports_functions("gpt-4-turbo"));
    assert!(AzureChatUtils::supports_functions("GPT-4")); // Case insensitive
}

#[test]
fn test_supports_functions_gpt35() {
    assert!(AzureChatUtils::supports_functions("gpt-35-turbo"));
    assert!(AzureChatUtils::supports_functions("gpt-35-turbo-16k"));
    assert!(AzureChatUtils::supports_functions("gpt-3.5-turbo"));
}

#[test]
fn test_supports_functions_other_models() {
    assert!(!AzureChatUtils::supports_functions("text-davinci-003"));
    assert!(!AzureChatUtils::supports_functions(
        "text-embedding-ada-002"
    ));
    assert!(!AzureChatUtils::supports_functions("dall-e-3"));
}

#[test]
fn test_supports_tools_gpt4_turbo() {
    assert!(AzureChatUtils::supports_tools("gpt-4-turbo"));
    assert!(AzureChatUtils::supports_tools("gpt-4-1106-preview"));
    assert!(AzureChatUtils::supports_tools("gpt-4-turbo-1106"));
}

#[test]
fn test_supports_tools_gpt4o() {
    assert!(AzureChatUtils::supports_tools("gpt-4o"));
    assert!(AzureChatUtils::supports_tools("gpt-4o-mini"));
}

#[test]
fn test_supports_tools_gpt35_1106() {
    assert!(AzureChatUtils::supports_tools("gpt-35-turbo-1106"));
}

#[test]
fn test_supports_tools_older_models() {
    assert!(!AzureChatUtils::supports_tools("gpt-4"));
    assert!(!AzureChatUtils::supports_tools("gpt-35-turbo"));
    assert!(!AzureChatUtils::supports_tools("gpt-4-32k"));
}

#[test]
fn test_azure_chat_handler_new() {
    let config =
        AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
    let handler = AzureChatHandler::new(config);
    assert!(handler.is_ok());
}

#[test]
fn test_transform_request_basic() {
    let config =
        AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
    let handler = AzureChatHandler::new(config).unwrap();

    let request = create_test_request();
    let result = handler.transform_request(&request);
    assert!(result.is_ok());

    let value = result.unwrap();
    assert!(value["messages"].is_array());
}

#[test]
fn test_transform_request_with_options() {
    let config =
        AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
    let handler = AzureChatHandler::new(config).unwrap();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![create_test_message(MessageRole::User, "Hello")],
        temperature: Some(0.7),
        max_tokens: Some(100),
        max_completion_tokens: Some(120),
        top_p: Some(0.9),
        frequency_penalty: Some(0.5),
        presence_penalty: Some(0.3),
        stop: Some(vec!["STOP".to_string()]),
        stream: true,
        ..Default::default()
    };

    let result = handler.transform_request(&request);
    assert!(result.is_ok());

    let value = result.unwrap();
    assert!((value["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
    assert_eq!(value["max_tokens"], 100);
    assert_eq!(value["max_completion_tokens"], 120);
    assert!((value["top_p"].as_f64().unwrap() - 0.9).abs() < 0.001);
    assert!(value["stop"].is_array());
    assert!(value["stream"].as_bool().unwrap());
}

#[test]
fn test_transform_response() {
    let config =
        AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
    let handler = AzureChatHandler::new(config).unwrap();

    let response = json!({
        "id": "chatcmpl-123",
        "created": 1234567890,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello there!"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        },
        "system_fingerprint": "fp_abc123"
    });

    let result = handler.transform_response(response, "gpt-4");
    assert!(result.is_ok());

    let chat_response = result.unwrap();
    assert_eq!(chat_response.id, "chatcmpl-123");
    assert_eq!(chat_response.model, "gpt-4");
    assert_eq!(chat_response.choices.len(), 1);
    assert_eq!(
        chat_response.choices[0].message.role,
        MessageRole::Assistant
    );
    assert_eq!(
        chat_response.choices[0].finish_reason,
        Some(FinishReason::Stop)
    );
    assert!(chat_response.usage.is_some());
    let usage = chat_response.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 15);
    assert_eq!(
        chat_response.system_fingerprint,
        Some("fp_abc123".to_string())
    );
}

#[test]
fn test_transform_response_finish_reasons() {
    let config =
        AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
    let handler = AzureChatHandler::new(config).unwrap();

    let finish_reasons = vec![
        ("stop", FinishReason::Stop),
        ("length", FinishReason::Length),
        ("tool_calls", FinishReason::ToolCalls),
        ("content_filter", FinishReason::ContentFilter),
        ("function_call", FinishReason::FunctionCall),
    ];

    for (reason_str, expected_reason) in finish_reasons {
        let response = json!({
            "id": "test",
            "created": 1234567890,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Test"
                },
                "finish_reason": reason_str
            }]
        });

        let result = handler.transform_response(response, "gpt-4");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().choices[0].finish_reason,
            Some(expected_reason)
        );
    }
}

#[test]
fn test_transform_response_roles() {
    let config =
        AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
    let handler = AzureChatHandler::new(config).unwrap();

    let roles = vec![
        ("system", MessageRole::System),
        ("user", MessageRole::User),
        ("assistant", MessageRole::Assistant),
        ("function", MessageRole::Function),
        ("tool", MessageRole::Tool),
    ];

    for (role_str, expected_role) in roles {
        let response = json!({
            "id": "test",
            "created": 1234567890,
            "choices": [{
                "index": 0,
                "message": {
                    "role": role_str,
                    "content": "Test"
                },
                "finish_reason": "stop"
            }]
        });

        let result = handler.transform_response(response, "gpt-4");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().choices[0].message.role, expected_role);
    }
}

#[test]
fn test_transform_streaming_chunk() {
    let chunk = json!({
        "id": "chatcmpl-123",
        "created": 1234567890,
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": "Hello"
            },
            "finish_reason": null
        }],
        "system_fingerprint": "fp_abc123"
    });

    let result = AzureChatHandler::transform_streaming_chunk(chunk, "gpt-4");
    assert!(result.is_ok());

    let chat_chunk = result.unwrap();
    assert_eq!(chat_chunk.id, "chatcmpl-123");
    assert_eq!(chat_chunk.model, "gpt-4");
    assert_eq!(chat_chunk.choices.len(), 1);
    assert_eq!(
        chat_chunk.choices[0].delta.content,
        Some("Hello".to_string())
    );
    assert_eq!(
        chat_chunk.choices[0].delta.role,
        Some(MessageRole::Assistant)
    );
}

#[test]
fn test_transform_streaming_chunk_finish() {
    let chunk = json!({
        "id": "chatcmpl-123",
        "created": 1234567890,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop"
        }]
    });

    let result = AzureChatHandler::transform_streaming_chunk(chunk, "gpt-4");
    assert!(result.is_ok());

    let chat_chunk = result.unwrap();
    assert_eq!(
        chat_chunk.choices[0].finish_reason,
        Some(FinishReason::Stop)
    );
}

#[test]
fn test_transform_streaming_chunk_empty_choices() {
    let chunk = json!({
        "id": "chatcmpl-123",
        "created": 1234567890
    });

    let result = AzureChatHandler::transform_streaming_chunk(chunk, "gpt-4");
    assert!(result.is_ok());

    let chat_chunk = result.unwrap();
    assert!(chat_chunk.choices.is_empty());
}

fn response_with_usage(usage: Value) -> ChatResponse {
    let config =
        AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
    let handler = AzureChatHandler::new(config).unwrap();
    handler
        .transform_response(
            json!({
                "id": "usage-test", "created": 1,
                "choices": [{"message": {"role": "assistant", "content": "ok"}}],
                "usage": usage
            }),
            "gpt-4",
        )
        .unwrap()
}

#[test]
fn test_usage_fails_closed_without_losing_legal_zero_or_range() {
    for bad in [
        json!({"prompt_tokens": 2, "total_tokens": 3}),
        json!({"prompt_tokens": "2", "completion_tokens": 1, "total_tokens": 3}),
        json!({"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 4}),
        json!({"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}),
    ] {
        assert!(response_with_usage(bad).usage.is_none());
    }
    let usage = response_with_usage(json!({
        "prompt_tokens": u64::from(u32::MAX) + 1,
        "completion_tokens": 0,
        "total_tokens": u64::from(u32::MAX) + 1
    }))
    .usage
    .unwrap();
    assert_eq!(
        (usage.prompt_tokens, usage.completion_tokens),
        (u32::MAX, 0)
    );
    assert_eq!(usage.total_tokens, u32::MAX);
}
