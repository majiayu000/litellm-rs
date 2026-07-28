use super::*;
use crate::core::providers::base::sse::UnifiedSSEParser;

fn create_test_request() -> ChatRequest {
    ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }],
        ..Default::default()
    }
}

#[test]
fn test_validate_request_success() {
    let request = create_test_request();
    assert!(AzureAIChatUtils::validate_request(&request).is_ok());
}

#[test]
fn test_validate_request_empty_messages() {
    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        ..Default::default()
    };
    let result = AzureAIChatUtils::validate_request(&request);
    assert!(result.is_err());
}

#[test]
fn test_validate_request_empty_model() {
    let mut request = create_test_request();
    request.model = String::new();
    let result = AzureAIChatUtils::validate_request(&request);
    assert!(result.is_err());
}

#[test]
fn test_validate_request_temperature_too_high() {
    let mut request = create_test_request();
    request.temperature = Some(2.5);
    let result = AzureAIChatUtils::validate_request(&request);
    assert!(result.is_err());
}

#[test]
fn test_validate_request_temperature_negative() {
    let mut request = create_test_request();
    request.temperature = Some(-0.5);
    let result = AzureAIChatUtils::validate_request(&request);
    assert!(result.is_err());
}

#[test]
fn test_validate_request_top_p_out_of_range() {
    let mut request = create_test_request();
    request.top_p = Some(1.5);
    let result = AzureAIChatUtils::validate_request(&request);
    assert!(result.is_err());
}

#[test]
fn test_transform_request_basic() {
    let request = create_test_request();
    let result = AzureAIChatUtils::transform_request(&request);
    assert!(result.is_ok());
    let value = result.unwrap();
    assert_eq!(value["model"], "gpt-4");
    assert!(value["messages"].is_array());
}

#[test]
fn test_transform_request_with_options() {
    let mut request = create_test_request();
    request.temperature = Some(0.5);
    request.max_tokens = Some(100);
    request.top_p = Some(0.9);
    request.frequency_penalty = Some(0.5);
    request.presence_penalty = Some(0.5);
    request.stop = Some(vec!["STOP".to_string()]);

    let result = AzureAIChatUtils::transform_request(&request);
    assert!(result.is_ok());
    let value = result.unwrap();
    // Use approximate comparison for floating point values
    assert!((value["temperature"].as_f64().unwrap() - 0.5).abs() < 0.001);
    assert_eq!(value["max_tokens"], 100);
    assert!((value["top_p"].as_f64().unwrap() - 0.9).abs() < 0.001);
    assert!((value["frequency_penalty"].as_f64().unwrap() - 0.5).abs() < 0.001);
    assert!((value["presence_penalty"].as_f64().unwrap() - 0.5).abs() < 0.001);
    assert!(value["stop"].is_array());
}

#[test]
fn test_transform_request_with_stream() {
    let mut request = create_test_request();
    request.stream = true;

    let result = AzureAIChatUtils::transform_request(&request);
    assert!(result.is_ok());
    let value = result.unwrap();
    assert_eq!(value["stream"], true);
}

#[test]
fn test_transform_response() {
    let response = json!({
        "id": "chatcmpl-123",
        "created": 1700000000,
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "Hello, how can I help?"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        },
        "system_fingerprint": "fp_123"
    });

    let result = AzureAIChatUtils::transform_response(response, "gpt-4");
    assert!(result.is_ok());
    let chat_response = result.unwrap();
    assert_eq!(chat_response.id, "chatcmpl-123");
    assert_eq!(chat_response.model, "gpt-4");
    assert_eq!(chat_response.choices.len(), 1);
    assert!(chat_response.usage.is_some());
    let usage = chat_response.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 20);
    assert_eq!(usage.total_tokens, 30);
}

#[test]
fn test_transform_response_finish_reasons() {
    let finish_reasons = vec![
        ("stop", FinishReason::Stop),
        ("length", FinishReason::Length),
        ("content_filter", FinishReason::ContentFilter),
        ("tool_calls", FinishReason::ToolCalls),
        ("function_call", FinishReason::FunctionCall),
    ];

    for (reason_str, expected_reason) in finish_reasons {
        let response = json!({
            "id": "test",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "test"
                },
                "finish_reason": reason_str
            }]
        });

        let result = AzureAIChatUtils::transform_response(response, "gpt-4").unwrap();
        assert_eq!(result.choices[0].finish_reason, Some(expected_reason));
    }
}

#[test]
fn test_parse_streaming_chunk_done() {
    let chunk = "data: [DONE]";
    let result = AzureAIChatUtils::parse_streaming_chunk(chunk, "gpt-4");
    assert!(result.is_ok());
    let chat_chunk = result.unwrap();
    assert_eq!(chat_chunk.id, "stream_end");
    assert!(chat_chunk.choices.is_empty());
}

#[test]
fn test_parse_streaming_chunk_content() {
    let chunk = r#"data: {"id":"test","choices":[{"delta":{"content":"Hello"}}]}"#;
    let result = AzureAIChatUtils::parse_streaming_chunk(chunk, "gpt-4");
    assert!(result.is_ok());
    let chat_chunk = result.unwrap();
    assert_eq!(chat_chunk.model, "gpt-4");
    assert_eq!(chat_chunk.choices.len(), 1);
    assert_eq!(
        chat_chunk.choices[0].delta.content.as_ref().unwrap(),
        "Hello"
    );
}

#[test]
fn test_parse_streaming_chunk_empty() {
    let chunk = "";
    let result = AzureAIChatUtils::parse_streaming_chunk(chunk, "gpt-4");
    assert!(result.is_ok());
    let chat_chunk = result.unwrap();
    assert_eq!(chat_chunk.id, "empty");
}

#[test]
fn test_sse_parser_buffers_split_and_multiple_events() {
    let transformer = AzureAISSETransformer::new("gpt-4".to_string());
    let mut parser = UnifiedSSEParser::new(transformer);

    let first = match parser
        .process_bytes(br#"data: {"id":"chunk-1","choices":[{"delta":{"content":"Hel"#)
    {
        Ok(chunks) => chunks,
        Err(error) => panic!("partial SSE bytes should be buffered: {error}"),
    };
    assert!(first.is_empty());

    let second = match parser.process_bytes(
        br#"lo"}}]}

data: {"id":"chunk-2","choices":[{"delta":{"content":" World"}}]}

data: [DONE]

"#,
    ) {
        Ok(chunks) => chunks,
        Err(error) => panic!("complete SSE events should parse: {error}"),
    };

    assert_eq!(second.len(), 2);
    assert_eq!(second[0].choices[0].delta.content.as_deref(), Some("Hello"));
    assert_eq!(
        second[1].choices[0].delta.content.as_deref(),
        Some(" World")
    );
}

#[test]
fn test_transform_role() {
    assert_eq!(
        AzureAIChatUtils::transform_role(&MessageRole::System),
        "system"
    );
    assert_eq!(AzureAIChatUtils::transform_role(&MessageRole::User), "user");
    assert_eq!(
        AzureAIChatUtils::transform_role(&MessageRole::Assistant),
        "assistant"
    );
    assert_eq!(
        AzureAIChatUtils::transform_role(&MessageRole::Function),
        "function"
    );
    assert_eq!(AzureAIChatUtils::transform_role(&MessageRole::Tool), "tool");
}

#[test]
fn test_transform_messages_with_name() {
    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            thinking: None,
            audio: None,
            name: Some("TestUser".to_string()),
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }],
        ..Default::default()
    };

    let result = AzureAIChatUtils::transform_request(&request).unwrap();
    assert!(result["messages"][0]["name"].is_string());
    assert_eq!(result["messages"][0]["name"], "TestUser");
}

#[test]
fn test_transform_messages_with_tool_call_id() {
    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::Tool,
            content: Some(MessageContent::Text("Result".to_string())),
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: Some("call_123".to_string()),
        }],
        ..Default::default()
    };

    let result = AzureAIChatUtils::transform_request(&request).unwrap();
    assert!(result["messages"][0]["tool_call_id"].is_string());
    assert_eq!(result["messages"][0]["tool_call_id"], "call_123");
}

#[test]
fn test_transform_response_missing_usage() {
    let response = json!({
        "id": "test",
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "test"
            }
        }]
    });

    let result = AzureAIChatUtils::transform_response(response, "gpt-4").unwrap();
    assert!(result.usage.is_none());
}

#[test]
fn test_transform_response_message_roles() {
    let roles = vec!["system", "user", "assistant", "function", "tool"];

    for role in roles {
        let response = json!({
            "id": "test",
            "choices": [{
                "message": {
                    "role": role,
                    "content": "test"
                }
            }]
        });

        let result = AzureAIChatUtils::transform_response(response, "gpt-4");
        assert!(result.is_ok());
    }
}

fn response_with_usage(usage: Value) -> ChatResponse {
    AzureAIChatUtils::transform_response(
        json!({
            "id": "usage-test",
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
        json!({"prompt_tokens": 2, "completion_tokens": null, "total_tokens": 3}),
        json!({"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 4}),
        json!({"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}),
    ] {
        assert!(response_with_usage(bad).usage.is_none());
    }
    let usage = response_with_usage(json!({
        "prompt_tokens": 0,
        "completion_tokens": u64::from(u32::MAX) + 1,
        "total_tokens": u64::from(u32::MAX) + 1
    }))
    .usage
    .unwrap();
    assert_eq!(
        (usage.prompt_tokens, usage.completion_tokens),
        (0, u32::MAX)
    );
    assert_eq!(usage.total_tokens, u32::MAX);
}
