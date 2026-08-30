use super::*;
use crate::core::types::content::ContentPart;
use crate::core::types::tools::{
    FunctionCall, FunctionChoice, FunctionDefinition, Tool, ToolCall, ToolChoice, ToolType,
};

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

#[test]
fn test_transform_chat_response_with_usage() {
    let transformer = GeminiTransformer::new();
    let response = json!({
        "candidates": [{
            "content": {"parts": [{"text": "Response"}]},
            "finishReason": "STOP"
        }],
        "usageMetadata": {
            "promptTokenCount": 100,
            "toolUsePromptTokenCount": 10,
            "candidatesTokenCount": 50,
            "thoughtsTokenCount": 5,
            "cachedContentTokenCount": 20,
            "totalTokenCount": 165
        }
    });
    let model = VertexAIModel::GeminiPro;

    let result = transformer
        .transform_chat_response(response, &model)
        .unwrap();
    let usage = result.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 110);
    assert_eq!(usage.completion_tokens, 55);
    assert_eq!(usage.total_tokens, 165);
    assert_eq!(
        usage
            .prompt_tokens_details
            .as_ref()
            .and_then(|details| details.cache_read_tokens),
        Some(20)
    );
    assert!(usage.completion_tokens_details.is_none());
}

fn vertex_response_with_usage(usage: Value) -> ChatResponse {
    GeminiTransformer::new()
        .transform_chat_response(
            json!({
                "candidates": [{"content": {"parts": [{"text": "ok"}]}}],
                "usageMetadata": usage
            }),
            &VertexAIModel::GeminiPro,
        )
        .unwrap()
}

#[test]
fn test_vertex_usage_metadata_fails_closed_and_saturates() {
    for bad in [
        json!({"promptTokenCount": 2, "candidatesTokenCount": 1, "totalTokenCount": 4}),
        json!({"promptTokenCount": "2", "candidatesTokenCount": 1, "totalTokenCount": 3}),
        json!({"promptTokenCount": 2, "totalTokenCount": 2}),
        json!({"promptTokenCount": 0, "candidatesTokenCount": 0, "totalTokenCount": 0}),
        json!({"promptTokenCount": 2, "candidatesTokenCount": 1, "cachedContentTokenCount": 3, "totalTokenCount": 3}),
    ] {
        assert!(vertex_response_with_usage(bad).usage.is_none());
    }
    let usage = vertex_response_with_usage(json!({
        "promptTokenCount": u64::MAX, "candidatesTokenCount": 0,
        "totalTokenCount": u64::MAX
    }))
    .usage
    .unwrap();
    assert_eq!(
        (usage.prompt_tokens, usage.total_tokens),
        (u32::MAX, u32::MAX)
    );
}

#[test]
fn test_vertex_transform_chat_response_preserves_tool_calls() {
    let transformer = GeminiTransformer::new();
    let response = json!({
        "candidates": [{
            "index": 3,
            "content": {"parts": [
                {"text": "checking"},
                {"functionCall": {
                    "id": "call_weather_1",
                    "name": "get_weather",
                    "args": {"city": "Paris"}
                }}
            ]},
            "finishReason": "STOP"
        }]
    });

    let result = transformer
        .transform_chat_response(response, &VertexAIModel::GeminiPro)
        .unwrap();
    let choice = &result.choices[0];
    assert_eq!(choice.index, 3);
    assert_eq!(choice.finish_reason, Some(FinishReason::ToolCalls));
    assert_eq!(
        choice.message.content.as_ref().unwrap().to_string(),
        "checking"
    );
    let call = choice.message.tool_calls.as_ref().unwrap().first().unwrap();
    assert_eq!(call.id, "call_weather_1");
    assert_eq!(call.function.name, "get_weather");
    assert_eq!(call.function.arguments, r#"{"city":"Paris"}"#);
}

#[test]
fn test_transform_chat_response_missing_candidates() {
    let transformer = GeminiTransformer::new();
    let response = json!({});
    let model = VertexAIModel::GeminiPro;

    let result = transformer.transform_chat_response(response, &model);
    assert!(result.is_err());
}

#[test]
fn test_transform_chat_response_empty_candidates() {
    let transformer = GeminiTransformer::new();
    let response = json!({"candidates": []});
    let model = VertexAIModel::GeminiPro;

    let result = transformer.transform_chat_response(response, &model);
    assert!(result.is_err());
}

#[test]
fn test_message_content_to_parts_text() {
    let transformer = GeminiTransformer::new();
    let content = MessageContent::Text("Hello world".to_string());

    let result = transformer.message_content_to_parts(&content);
    assert!(result.is_ok());
    let parts = result.unwrap();
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        Part::Text { text } => assert_eq!(text, "Hello world"),
        _ => panic!("Expected text part"),
    }
}

#[test]
fn test_message_content_to_parts_multipart_text() {
    let transformer = GeminiTransformer::new();
    let content = MessageContent::Parts(vec![
        ContentPart::Text {
            text: "Part 1".to_string(),
        },
        ContentPart::Text {
            text: "Part 2".to_string(),
        },
    ]);

    let result = transformer.message_content_to_parts(&content);
    assert!(result.is_ok());
    let parts = result.unwrap();
    assert_eq!(parts.len(), 2);
}

#[test]
fn vertex_multimodal_parts_encode_audio_and_pdf_as_inline_data() {
    use crate::core::types::content::{AudioData, DocumentSource};

    let transformer = GeminiTransformer::new();
    let content = MessageContent::Parts(vec![
        ContentPart::Audio {
            audio: AudioData {
                data: "audio-base64".to_string(),
                format: Some("mp3".to_string()),
            },
        },
        ContentPart::Document {
            source: DocumentSource {
                media_type: "application/pdf".to_string(),
                data: "pdf-base64".to_string(),
            },
            cache_control: None,
        },
    ]);

    let parts = transformer
        .message_content_to_parts(&content)
        .expect("official Vertex media modalities must transform");
    let wire = serde_json::to_value(parts).expect("parts serialize");
    assert_eq!(wire[0]["inlineData"]["mimeType"], "audio/mp3");
    assert_eq!(wire[0]["inlineData"]["data"], "audio-base64");
    assert_eq!(wire[1]["inlineData"]["mimeType"], "application/pdf");
    assert_eq!(wire[1]["inlineData"]["data"], "pdf-base64");
}

#[test]
fn vertex_multimodal_parts_emit_canonical_aac_mime_type() {
    use crate::core::types::content::AudioData;

    let transformer = GeminiTransformer::new();
    for format in ["aac", "audio/aac", "audio/x-aac"] {
        let content = MessageContent::Parts(vec![ContentPart::Audio {
            audio: AudioData {
                data: "audio-base64".to_string(),
                format: Some(format.to_string()),
            },
        }]);
        let parts = transformer
            .message_content_to_parts(&content)
            .expect("AAC must transform");
        let wire = serde_json::to_value(parts).expect("parts serialize");
        assert_eq!(wire[0]["inlineData"]["mimeType"], "audio/aac", "{format}");
    }
}

fn weather_tool() -> Tool {
    Tool {
        tool_type: ToolType::Function,
        function: FunctionDefinition {
            name: "get_weather".to_string(),
            description: Some("Get weather".to_string()),
            parameters: Some(json!({
                "type": "object",
                "properties": {"city": {"type": "string"}}
            })),
        },
    }
}

fn weather_call() -> ToolCall {
    ToolCall {
        id: "call_weather_1".to_string(),
        tool_type: "function".to_string(),
        function: FunctionCall {
            name: "get_weather".to_string(),
            arguments: r#"{"city":"Paris"}"#.to_string(),
        },
    }
}

#[test]
fn test_vertex_gemini_tool_loop_wire_uses_camel_case() {
    let transformer = GeminiTransformer::new();
    let request = ChatRequest {
        model: "gemini-1.5-pro".to_string(),
        messages: vec![
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
            function: Some(FunctionChoice {
                name: "get_weather".to_string(),
            }),
        }),
        ..Default::default()
    };

    let body = transformer
        .transform_chat_request(&request, &VertexAIModel::GeminiPro)
        .unwrap();
    assert_eq!(
        body["tools"][0]["functionDeclarations"][0]["name"],
        "get_weather"
    );
    assert_eq!(
        body["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"][0],
        "get_weather"
    );
    assert_eq!(
        body["contents"][0]["parts"][1]["functionCall"]["id"],
        "call_weather_1"
    );
    assert_eq!(
        body["contents"][1]["parts"][0]["functionResponse"],
        json!({"name":"get_weather","response":{"result":"sunny"}})
    );
    let wire = serde_json::to_string(&body).unwrap();
    assert!(wire.contains("functionDeclarations"));
    assert!(wire.contains("functionCallingConfig"));
    assert!(wire.contains("allowedFunctionNames"));
    assert!(wire.contains("functionCall"));
    assert!(wire.contains("functionResponse"));
    assert!(!wire.contains("function_declarations"));
    assert!(!wire.contains("function_calling_config"));
    assert!(!wire.contains("allowed_function_names"));
}

// ==================== PartnerModelTransformer Tests ====================

#[test]
fn test_partner_transformer_new() {
    let transformer = PartnerModelTransformer::new();
    assert!(format!("{:?}", transformer).contains("PartnerModelTransformer"));
}

#[test]
fn test_partner_transformer_default() {
    let transformer = PartnerModelTransformer;
    assert!(format!("{:?}", transformer).contains("PartnerModelTransformer"));
}

#[test]
fn test_transform_claude_request() {
    let transformer = PartnerModelTransformer::new();
    let request = ChatRequest {
        model: "claude-3-5-sonnet".to_string(),
        messages: vec![
            create_test_message(MessageRole::System, "You are helpful"),
            create_test_message(MessageRole::User, "Hello"),
        ],
        max_tokens: Some(1000),
        temperature: Some(0.7),
        ..Default::default()
    };
    let model = VertexAIModel::Claude35Sonnet;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body["instances"].is_array());
    let instance = &body["instances"][0];
    assert_eq!(instance["anthropic_version"], "vertex-2023-10-16");
    assert!(instance["messages"].is_array());
}

#[test]
fn test_transform_llama_request() {
    let transformer = PartnerModelTransformer::new();
    let request = ChatRequest {
        model: "llama3-70b".to_string(),
        messages: vec![create_test_message(MessageRole::User, "Hello")],
        temperature: Some(0.8),
        max_tokens: Some(500),
        ..Default::default()
    };
    let model = VertexAIModel::Llama3_70B;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body["instances"].is_array());
    assert!(body["instances"][0]["prompt"].is_string());
    assert!(body["parameters"]["temperature"].is_number());
}

#[test]
fn test_transform_jamba_request() {
    let transformer = PartnerModelTransformer::new();
    let request = ChatRequest {
        model: "jamba-1.5-large".to_string(),
        messages: vec![create_test_message(MessageRole::User, "Hello")],
        ..Default::default()
    };
    let model = VertexAIModel::Jamba15Large;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body["instances"].is_array());
    assert!(body["instances"][0]["messages"].is_array());
}

#[test]
fn test_transform_default_partner_request() {
    let transformer = PartnerModelTransformer::new();
    let request = ChatRequest {
        model: "mistral-large".to_string(),
        messages: vec![create_test_message(MessageRole::User, "Hello")],
        ..Default::default()
    };
    let model = VertexAIModel::MistralLarge;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body["instances"].is_array());
}

#[test]
fn test_messages_to_llama_prompt_user_only() {
    let transformer = PartnerModelTransformer::new();
    let messages = vec![create_test_message(MessageRole::User, "Hello")];

    let prompt = transformer.messages_to_llama_prompt(&messages);
    assert!(prompt.contains("[INST] Hello [/INST]"));
}

#[test]
fn test_messages_to_llama_prompt_with_system() {
    let transformer = PartnerModelTransformer::new();
    let messages = vec![
        create_test_message(MessageRole::System, "You are helpful"),
        create_test_message(MessageRole::User, "Hello"),
    ];

    let prompt = transformer.messages_to_llama_prompt(&messages);
    assert!(prompt.contains("<<SYS>>"));
    assert!(prompt.contains("You are helpful"));
    assert!(prompt.contains("<</SYS>>"));
}

#[test]
fn test_messages_to_llama_prompt_conversation() {
    let transformer = PartnerModelTransformer::new();
    let messages = vec![
        create_test_message(MessageRole::User, "Hi"),
        create_test_message(MessageRole::Assistant, "Hello!"),
        create_test_message(MessageRole::User, "How are you?"),
    ];

    let prompt = transformer.messages_to_llama_prompt(&messages);
    assert!(prompt.contains("[INST] Hi [/INST]"));
    assert!(prompt.contains("Hello!"));
    assert!(prompt.contains("[INST] How are you? [/INST]"));
}

#[test]
fn test_transform_partner_response_basic() {
    let transformer = PartnerModelTransformer::new();
    let response = json!({
        "predictions": [{
            "content": "Hello! I'm Claude."
        }]
    });
    let model = VertexAIModel::Claude35Sonnet;

    let result = transformer.transform_chat_response(response, &model);
    assert!(result.is_ok());
    let chat_response = result.unwrap();
    assert_eq!(chat_response.object, "chat.completion");
    assert_eq!(chat_response.choices.len(), 1);
}

#[test]
fn test_transform_partner_response_with_text_field() {
    let transformer = PartnerModelTransformer::new();
    let response = json!({
        "predictions": [{
            "text": "Llama response"
        }]
    });
    let model = VertexAIModel::Llama3_70B;

    let result = transformer.transform_chat_response(response, &model);
    assert!(result.is_ok());
}

#[test]
fn test_transform_partner_response_missing_predictions() {
    let transformer = PartnerModelTransformer::new();
    let response = json!({});
    let model = VertexAIModel::Claude35Sonnet;

    let result = transformer.transform_chat_response(response, &model);
    assert!(result.is_err());
}

#[test]
fn test_transform_partner_response_empty_predictions() {
    let transformer = PartnerModelTransformer::new();
    let response = json!({"predictions": []});
    let model = VertexAIModel::Claude35Sonnet;

    let result = transformer.transform_chat_response(response, &model);
    assert!(result.is_err());
}

#[test]
fn test_transform_partner_response_with_metadata() {
    let transformer = PartnerModelTransformer::new();
    let response = json!({
        "predictions": [{
            "content": "Response"
        }],
        "metadata": {
            "tokenMetadata": {
                "inputTokens": {"totalTokens": 50},
                "outputTokens": {"totalTokens": 100}
            }
        }
    });
    let model = VertexAIModel::Claude35Sonnet;

    let result = transformer
        .transform_chat_response(response, &model)
        .unwrap();
    let usage = result.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 50);
    assert_eq!(usage.completion_tokens, 100);
    assert_eq!(usage.total_tokens, 150);
}

#[test]
fn test_partner_usage_metadata_is_strict_and_missing_stays_none() {
    let transform = |metadata: Option<Value>| {
        let mut response = json!({"predictions": [{"content": "ok"}]});
        if let Some(metadata) = metadata {
            response["metadata"] = metadata;
        }
        PartnerModelTransformer::new()
            .transform_chat_response(response, &VertexAIModel::Claude35Sonnet)
            .unwrap()
    };
    assert!(transform(None).usage.is_none());
    for bad in [
        json!({"tokenMetadata": {"inputTokens": {"totalTokens": 1}}}),
        json!({"tokenMetadata": {"inputTokens": {"totalTokens": "1"}, "outputTokens": {"totalTokens": 2}}}),
        json!({"tokenMetadata": {"inputTokens": {"totalTokens": 0}, "outputTokens": {"totalTokens": 0}}}),
    ] {
        assert!(transform(Some(bad)).usage.is_none());
    }
    let usage = transform(Some(json!({"tokenMetadata": {
        "inputTokens": {"totalTokens": u64::MAX},
        "outputTokens": {"totalTokens": 1}
    }})))
    .usage
    .unwrap();
    assert_eq!(usage.total_tokens, u32::MAX);
}
