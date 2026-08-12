use super::*;
use crate::core::providers::google_tool_loop::GoogleToolPlanner;
use crate::core::types::tools::{FunctionDefinition, Tool, ToolCall, ToolChoice, ToolType};

fn transform_parts(client: &GeminiClient, message: &ChatMessage) -> Vec<Value> {
    let mut planner = GoogleToolPlanner::new("gemini");
    client
        .transform_message_content(0, message, &mut planner)
        .unwrap()
}

fn weather_tool() -> Tool {
    Tool {
        tool_type: ToolType::Function,
        function: FunctionDefinition {
            name: "get_weather".to_string(),
            description: Some("Get weather".to_string()),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                }
            })),
        },
    }
}

fn weather_call() -> ToolCall {
    ToolCall {
        id: "call_weather_1".to_string(),
        tool_type: "function".to_string(),
        function: crate::core::types::tools::FunctionCall {
            name: "get_weather".to_string(),
            arguments: r#"{"city":"Paris"}"#.to_string(),
        },
    }
}

#[test]
fn test_client_creation() {
    let config = GeminiConfig::new_google_ai("test-key");
    let client = GeminiClient::new(config);
    assert!(client.is_ok());
}

#[test]
fn test_data_url_parsing() {
    let config = GeminiConfig::new_google_ai("test-key");
    let client = GeminiClient::new(config).unwrap();

    let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
    let result = client.parse_data_url(data_url).unwrap();

    assert!(result.is_some());
    let (mime_type, _data) = result.unwrap();
    assert_eq!(mime_type, "image/png");
}

#[test]
fn test_message_transformation() {
    let config = GeminiConfig::new_google_ai("test-key");
    let client = GeminiClient::new(config).unwrap();

    let message = ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("Hello, world!".to_string())),
        thinking: None,
        audio: None,
        name: None,
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
    };

    let parts = transform_parts(&client, &message);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0]["text"], "Hello, world!");
}

#[test]
fn test_multimodal_message() {
    let config = GeminiConfig::new_google_ai("test-key");
    let client = GeminiClient::new(config).unwrap();

    let message = ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![
            ContentPart::Text {
                text: "What's in this image?".to_string(),
            },
            ContentPart::Image {
                source: crate::core::types::content::ImageSource {
                    data: "test".to_string(),
                    media_type: "image/png".to_string(),
                },
                image_url: None,
                detail: None,
            },
        ])),
        thinking: None,
        audio: None,
        name: None,
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
    };

    let parts = transform_parts(&client, &message);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0]["text"], "What's in this image?");
    assert!(parts[1].get("inlineData").is_some());
}

#[test]
fn test_gemini_request_maps_tool_call_and_result_round_trip() {
    let client = GeminiClient::new(GeminiConfig::new_google_ai("test-key")).unwrap();
    let request = ChatRequest {
        model: "gemini-2.0-flash".to_string(),
        messages: vec![
            ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("weather?".to_string())),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: Some(MessageContent::Text("checking".to_string())),
                tool_calls: Some(vec![weather_call()]),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::Tool,
                tool_call_id: Some("call_weather_1".to_string()),
                content: Some(MessageContent::Text("sunny".to_string())),
                ..Default::default()
            },
        ],
        tools: Some(vec![weather_tool()]),
        tool_choice: Some(ToolChoice::Specific {
            choice_type: "function".to_string(),
            function: Some(crate::core::types::tools::FunctionChoice {
                name: "get_weather".to_string(),
            }),
        }),
        ..Default::default()
    };

    let body = client.transform_chat_request(&request).unwrap();
    assert_eq!(
        body["tools"][0]["functionDeclarations"][0]["name"],
        "get_weather"
    );
    assert_eq!(
        body["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"][0],
        "get_weather"
    );
    assert_eq!(body["contents"][1]["role"], "model");
    assert_eq!(
        body["contents"][1]["parts"][1]["functionCall"],
        json!({
            "id": "call_weather_1",
            "name": "get_weather",
            "args": {"city": "Paris"}
        })
    );
    assert_eq!(body["contents"][2]["role"], "user");
    assert_eq!(
        body["contents"][2]["parts"][0]["functionResponse"],
        json!({
            "name": "get_weather",
            "response": {"result": "sunny"}
        })
    );
}

#[test]
fn test_gemini_request_rejects_unknown_tool_result_id() {
    let client = GeminiClient::new(GeminiConfig::new_google_ai("test-key")).unwrap();
    let request = ChatRequest {
        model: "gemini-2.0-flash".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::Tool,
            tool_call_id: Some("missing".to_string()),
            content: Some(MessageContent::Text("sunny".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };
    let err = client.transform_chat_request(&request).unwrap_err();
    assert!(matches!(err, ProviderError::InvalidRequest { .. }));
}

#[test]
fn july_2026_request_body_omits_sampling_parameters() {
    let client = GeminiClient::new(GeminiConfig::new_google_ai("test-key")).unwrap();
    let mut request = ChatRequest {
        model: "gemini-3.6-flash".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        temperature: Some(0.7),
        top_p: Some(0.9),
        max_tokens: Some(16),
        ..Default::default()
    };

    let body = client.transform_chat_request(&request).unwrap();
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 16);
    assert!(body["generationConfig"].get("temperature").is_none());
    assert!(body["generationConfig"].get("topP").is_none());

    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text("prefill".to_string())),
        ..Default::default()
    });
    assert!(client.transform_chat_request(&request).is_err());
}

#[test]
fn test_gemini_finish_reason_tool_calls() {
    let config = GeminiConfig::new_google_ai("test-key");
    let client = GeminiClient::new(config).unwrap();
    let request = ChatRequest {
        model: "gemini-2.0-flash".to_string(),
        ..Default::default()
    };

    let response = json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "name": "get_weather",
                        "args": {"city": "Paris"}
                    }
                }]
            },
            "finishReason": "STOP"
        }]
    });

    let result = client.transform_chat_response(response, &request).unwrap();
    let choice = result.choices.first().unwrap();
    assert_eq!(
        choice.finish_reason,
        Some(crate::core::types::responses::FinishReason::ToolCalls)
    );
    let tool_call = choice
        .message
        .tool_calls
        .as_ref()
        .and_then(|calls| calls.first())
        .unwrap();
    assert_eq!(tool_call.function.name, "get_weather");
    assert_eq!(tool_call.function.arguments, r#"{"city":"Paris"}"#);
}

#[test]
fn test_gemini_usage_preserves_cache_and_thought_tokens() {
    let config = GeminiConfig::new_google_ai("test-key");
    let client = GeminiClient::new(config).unwrap();
    let request = ChatRequest {
        model: "gemini-2.0-flash".to_string(),
        ..Default::default()
    };

    let response = json!({
        "candidates": [{
            "content": {
                "parts": [{"text": "Hello"}]
            },
            "finishReason": "STOP"
        }],
        "usageMetadata": {
            "promptTokenCount": 100,
            "toolUsePromptTokenCount": 10,
            "candidatesTokenCount": 25,
            "totalTokenCount": 140,
            "cachedContentTokenCount": 12,
            "thoughtsTokenCount": 15
        }
    });

    let result = client.transform_chat_response(response, &request).unwrap();
    let usage = result.usage.unwrap();
    let prompt_details = usage.prompt_tokens_details.unwrap();
    assert_eq!(usage.prompt_tokens, 110);
    assert_eq!(usage.completion_tokens, 40);
    assert_eq!(usage.total_tokens, 150);
    assert_eq!(prompt_details.cached_tokens, Some(12));
    assert_eq!(prompt_details.cache_read_tokens, Some(12));
    assert!(usage.completion_tokens_details.is_none());
    assert_eq!(
        usage
            .thinking_usage
            .as_ref()
            .and_then(|thinking| thinking.thinking_tokens),
        Some(15)
    );
}

#[test]
fn test_gemini_usage_fails_closed_and_saturates_after_raw_total() {
    let client = GeminiClient::new(GeminiConfig::new_google_ai("test-key")).unwrap();
    let request = ChatRequest {
        model: "gemini-2.0-flash".to_string(),
        ..Default::default()
    };
    let transform = |client: &GeminiClient, usage| {
        client
            .transform_chat_response(
                json!({
                    "candidates": [{"content": {"parts": [{"text": "ok"}]}}],
                    "usageMetadata": usage
                }),
                &request,
            )
            .unwrap()
            .usage
    };
    for bad in [
        json!({"promptTokenCount": 2, "candidatesTokenCount": 1, "totalTokenCount": 4}),
        json!({"promptTokenCount": 2, "candidatesTokenCount": "1", "totalTokenCount": 3}),
        json!({"promptTokenCount": 0, "candidatesTokenCount": 0, "totalTokenCount": 0}),
        json!({"promptTokenCount": 2, "candidatesTokenCount": 1, "cachedContentTokenCount": 3, "totalTokenCount": 3}),
    ] {
        assert!(transform(&client, bad).is_none());
    }
    let usage = transform(
        &client,
        json!({
            "promptTokenCount": u64::MAX, "candidatesTokenCount": 0,
            "totalTokenCount": u64::MAX
        }),
    )
    .unwrap();
    assert_eq!(
        (usage.prompt_tokens, usage.total_tokens),
        (u32::MAX, u32::MAX)
    );
    let vertex = GeminiClient::new(GeminiConfig::new_vertex_ai("project", "location")).unwrap();
    let mut vertex_usage = json!({
        "promptTokenCount": 2, "toolUsePromptTokenCount": 1,
        "candidatesTokenCount": 1, "thoughtsTokenCount": 1, "totalTokenCount": 5
    });
    let usage = transform(&vertex, vertex_usage.clone()).unwrap();
    assert_eq!((usage.prompt_tokens, usage.completion_tokens), (3, 2));
    assert!(usage.thinking_usage.is_none());
    vertex_usage["totalTokenCount"] = json!(4);
    assert!(transform(&vertex, vertex_usage).is_none());
}
