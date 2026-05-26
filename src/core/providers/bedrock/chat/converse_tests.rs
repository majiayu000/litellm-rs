use super::*;
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

// ==================== Data Structure Tests ====================

#[test]
fn test_converse_message_serialization() {
    let message = ConverseMessage {
        role: "user".to_string(),
        content: vec![ContentBlock::Text {
            text: "Hello".to_string(),
        }],
    };

    let json = serde_json::to_value(&message).unwrap();
    assert_eq!(json["role"], "user");
    assert!(json["content"].is_array());
}

#[test]
fn test_system_message_with_text() {
    let msg = SystemMessage {
        text: Some("You are a helpful assistant".to_string()),
        guardrail_content: None,
    };

    let json = serde_json::to_value(&msg).unwrap();
    assert_eq!(json["text"], "You are a helpful assistant");
    assert!(json.get("guardrail_content").is_none());
}

#[test]
fn test_system_message_with_guardrail() {
    let msg = SystemMessage {
        text: None,
        guardrail_content: Some(GuardrailContent {
            text: "Safety content".to_string(),
        }),
    };

    let json = serde_json::to_value(&msg).unwrap();
    assert!(json.get("text").is_none());
    assert_eq!(json["guardrail_content"]["text"], "Safety content");
}

#[test]
fn test_content_block_text() {
    let block = ContentBlock::Text {
        text: "Hello world".to_string(),
    };

    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["text"], "Hello world");
}

#[test]
fn test_content_block_image() {
    let block = ContentBlock::Image {
        image: ImageBlock {
            format: "png".to_string(),
            source: ImageSource::Bytes {
                bytes: "base64data".to_string(),
            },
        },
    };

    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["image"]["format"], "png");
    assert_eq!(json["image"]["source"]["bytes"], "base64data");
}

#[test]
fn test_content_block_document() {
    let block = ContentBlock::Document {
        document: DocumentBlock {
            format: "pdf".to_string(),
            name: "test.pdf".to_string(),
            source: DocumentSource::Bytes {
                bytes: "pdfdata".to_string(),
            },
        },
    };

    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["document"]["format"], "pdf");
    assert_eq!(json["document"]["name"], "test.pdf");
    assert_eq!(json["document"]["source"]["bytes"], "pdfdata");
}

#[test]
fn test_tool_use_block() {
    let block = ContentBlock::ToolUse {
        tool_use: ToolUseBlock {
            tool_use_id: "tool-123".to_string(),
            name: "get_weather".to_string(),
            input: serde_json::json!({"location": "NYC"}),
        },
    };

    let json = serde_json::to_value(&block).unwrap();
    let inner = &json["toolUse"];
    assert_eq!(inner["toolUseId"], "tool-123");
    assert_eq!(inner["name"], "get_weather");
}

#[test]
fn test_tool_result_block() {
    let block = ContentBlock::ToolResult {
        tool_result: ToolResultBlock {
            tool_use_id: "tool-123".to_string(),
            content: vec![ToolResultContent::Text {
                text: "Weather is sunny".to_string(),
            }],
            status: Some("success".to_string()),
        },
    };

    let json = serde_json::to_value(&block).unwrap();
    let inner = &json["toolResult"];
    assert_eq!(inner["toolUseId"], "tool-123");
}

#[test]
fn test_inference_config_full() {
    let config = InferenceConfig {
        max_tokens: Some(1000),
        temperature: Some(0.7),
        top_p: Some(0.9),
        stop_sequences: Some(vec!["STOP".to_string()]),
    };

    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["maxTokens"], 1000);
    assert_eq!(json["temperature"], 0.7);
    assert_eq!(json["topP"], 0.9);
}

#[test]
fn test_inference_config_minimal() {
    let config = InferenceConfig {
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop_sequences: None,
    };

    let json = serde_json::to_value(&config).unwrap();
    // All fields should be omitted due to skip_serializing_if
    assert!(json.as_object().unwrap().is_empty());
}

#[test]
fn test_tool_spec() {
    let spec = ToolSpec {
        tool_spec: ToolSpecDefinition {
            name: "calculator".to_string(),
            description: "Performs calculations".to_string(),
            input_schema: InputSchema {
                json: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "expression": {"type": "string"}
                    }
                }),
            },
        },
    };

    let json = serde_json::to_value(&spec).unwrap();
    assert_eq!(json["toolSpec"]["name"], "calculator");
    assert_eq!(json["toolSpec"]["description"], "Performs calculations");
}

#[test]
fn test_tool_choice_auto() {
    let choice = ToolChoice::Auto;
    let json = serde_json::to_value(&choice).unwrap();
    assert_eq!(json["auto"], serde_json::json!({}));
}

#[test]
fn test_tool_choice_any() {
    let choice = ToolChoice::Any;
    let json = serde_json::to_value(&choice).unwrap();
    assert_eq!(json["any"], serde_json::json!({}));
}

#[test]
fn test_tool_choice_specific_tool() {
    let choice = ToolChoice::Tool {
        name: "get_weather".to_string(),
    };
    let json = serde_json::to_value(&choice).unwrap();
    assert_eq!(json["tool"]["name"], "get_weather");
}

#[test]
fn test_guardrail_config() {
    let config = GuardrailConfig {
        guardrail_identifier: "guardrail-123".to_string(),
        guardrail_version: "1.0".to_string(),
        trace: Some(true),
    };

    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["guardrailIdentifier"], "guardrail-123");
    assert_eq!(json["guardrailVersion"], "1.0");
    assert_eq!(json["trace"], true);
}

#[test]
fn test_image_source_bytes() {
    let source = ImageSource::Bytes {
        bytes: "base64imagedata".to_string(),
    };

    let json = serde_json::to_value(&source).unwrap();
    assert_eq!(json["bytes"], "base64imagedata");
}

#[test]
fn test_document_source_bytes() {
    let source = DocumentSource::Bytes {
        bytes: "base64docdata".to_string(),
    };

    let json = serde_json::to_value(&source).unwrap();
    assert_eq!(json["bytes"], "base64docdata");
}

// ==================== Transform Tests ====================

#[test]
fn test_transform_simple_user_message() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            ..Default::default()
        }],
        ..Default::default()
    };

    let result = transform_to_converse(&request);
    assert!(result.is_ok());

    let converse = result.unwrap();
    assert_eq!(converse.messages.len(), 1);
    assert_eq!(converse.messages[0].role, "user");
}

#[test]
fn test_transform_with_system_message() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![
            ChatMessage {
                role: MessageRole::System,
                content: Some(MessageContent::Text("You are helpful".to_string())),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let result = transform_to_converse(&request);
    assert!(result.is_ok());

    let converse = result.unwrap();
    assert!(converse.system.is_some());
    let system = converse.system.unwrap();
    assert_eq!(system.len(), 1);
    assert_eq!(system[0].text, Some("You are helpful".to_string()));
}

#[test]
fn test_transform_with_inference_config() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        max_tokens: Some(500),
        temperature: Some(0.8),
        top_p: Some(0.95),
        stop: Some(vec!["END".to_string()]),
        ..Default::default()
    };

    let result = transform_to_converse(&request);
    assert!(result.is_ok());

    let converse = result.unwrap();
    assert!(converse.inference_config.is_some());

    let config = converse.inference_config.unwrap();
    assert_eq!(config.max_tokens, Some(500));
    assert!((config.temperature.unwrap() - 0.8).abs() < 0.001);
    assert!((config.top_p.unwrap() - 0.95).abs() < 0.001);
    assert_eq!(config.stop_sequences, Some(vec!["END".to_string()]));
}

#[test]
fn test_transform_prefers_max_completion_tokens() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        max_tokens: Some(500),
        max_completion_tokens: Some(128),
        ..Default::default()
    };

    let converse = transform_to_converse(&request)
        .unwrap_or_else(|err| panic!("Converse request should transform: {err}"));
    let config = converse
        .inference_config
        .unwrap_or_else(|| panic!("inferenceConfig should be emitted"));

    assert_eq!(config.max_tokens, Some(128));
}

#[test]
fn test_transform_with_forced_tool_choice() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Weather?".to_string())),
            ..Default::default()
        }],
        tools: Some(vec![crate::core::types::tools::Tool {
            tool_type: crate::core::types::tools::ToolType::Function,
            function: crate::core::types::tools::FunctionDefinition {
                name: "lookup_weather".to_string(),
                description: Some("Look up weather".to_string()),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    }
                })),
            },
        }]),
        tool_choice: Some(crate::core::types::tools::ToolChoice::Specific {
            choice_type: "function".to_string(),
            function: Some(crate::core::types::tools::FunctionChoice {
                name: "lookup_weather".to_string(),
            }),
        }),
        ..Default::default()
    };

    let converse = transform_to_converse(&request)
        .unwrap_or_else(|err| panic!("Converse request should transform: {err}"));
    let tool_config = converse
        .tool_config
        .unwrap_or_else(|| panic!("toolConfig should be emitted"));
    let tool_choice = tool_config
        .tool_choice
        .as_ref()
        .unwrap_or_else(|| panic!("toolChoice should be preserved"));

    assert!(matches!(
        tool_choice,
        ToolChoice::Tool { name } if name == "lookup_weather"
    ));

    let json = serde_json::to_value(&tool_config)
        .unwrap_or_else(|err| panic!("toolConfig should serialize: {err}"));
    assert_eq!(json["toolChoice"]["tool"]["name"], "lookup_weather");
}

#[test]
fn test_transform_conversation() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![
            ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hi".to_string())),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: Some(MessageContent::Text("Hello!".to_string())),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("How are you?".to_string())),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let result = transform_to_converse(&request);
    assert!(result.is_ok());

    let converse = result.unwrap();
    assert_eq!(converse.messages.len(), 3);
    assert_eq!(converse.messages[0].role, "user");
    assert_eq!(converse.messages[1].role, "assistant");
    assert_eq!(converse.messages[2].role, "user");
}

#[test]
fn test_transform_empty_messages() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![],
        ..Default::default()
    };

    let result = transform_to_converse(&request);
    assert!(result.is_ok());

    let converse = result.unwrap();
    assert!(converse.messages.is_empty());
    assert!(converse.system.is_none());
}

#[test]
fn test_transform_message_without_content() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: None,
            ..Default::default()
        }],
        ..Default::default()
    };

    let result = transform_to_converse(&request);
    assert!(result.is_ok());

    let converse = result.unwrap();
    assert_eq!(converse.messages.len(), 1);
    assert!(converse.messages[0].content.is_empty());
}

#[test]
fn test_bedrock_tool_result_round_trip() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![
            ChatMessage {
                role: MessageRole::Assistant,
                content: Some(MessageContent::Text("I'll call a tool.".to_string())),
                tool_calls: Some(vec![crate::core::types::tools::ToolCall {
                    id: "tool-123".to_string(),
                    tool_type: "function".to_string(),
                    function: crate::core::types::tools::FunctionCall {
                        name: "get_weather".to_string(),
                        arguments: r#"{"city":"Paris"}"#.to_string(),
                    },
                }]),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::Tool,
                content: Some(MessageContent::Text("Sunny".to_string())),
                tool_call_id: Some("tool-123".to_string()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let converse = transform_to_converse(&request).unwrap();
    let json = serde_json::to_value(&converse).unwrap();

    assert_eq!(json["messages"][0]["role"], "assistant");
    assert_eq!(
        json["messages"][0]["content"][0]["text"],
        "I'll call a tool."
    );
    let tool_use = &json["messages"][0]["content"][1]["toolUse"];
    assert_eq!(tool_use["toolUseId"], "tool-123");
    assert_eq!(tool_use["name"], "get_weather");
    assert_eq!(tool_use["input"]["city"], "Paris");

    assert_eq!(json["messages"][1]["role"], "user");
    let tool_result = &json["messages"][1]["content"][0]["toolResult"];
    assert_eq!(tool_result["toolUseId"], "tool-123");
    assert_eq!(tool_result["content"][0]["text"], "Sunny");
}

#[test]
fn test_bedrock_converse_tool_result_content_part() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Parts(vec![
                crate::core::types::content::ContentPart::Text {
                    text: "Tool output follows.".to_string(),
                },
                crate::core::types::content::ContentPart::ToolResult {
                    tool_use_id: "tool-123".to_string(),
                    content: serde_json::json!("done"),
                    is_error: Some(true),
                },
            ])),
            ..Default::default()
        }],
        ..Default::default()
    };

    let converse = transform_to_converse(&request).unwrap();
    let json = serde_json::to_value(&converse).unwrap();
    assert_eq!(
        json["messages"][0]["content"][0]["text"],
        "Tool output follows."
    );
    let tool_result = &json["messages"][0]["content"][1]["toolResult"];
    assert_eq!(tool_result["toolUseId"], "tool-123");
    assert_eq!(tool_result["content"][0]["text"], "done");
    assert_eq!(tool_result["status"], "error");
}

#[test]
fn test_bedrock_converse_unsupported_image_is_error() {
    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Parts(vec![
                crate::core::types::content::ContentPart::Image {
                    source: crate::core::types::content::ImageSource {
                        media_type: "image/png".to_string(),
                        data: "base64-image".to_string(),
                    },
                    detail: None,
                    image_url: None,
                },
            ])),
            ..Default::default()
        }],
        ..Default::default()
    };

    let err = transform_to_converse(&request).unwrap_err();
    assert!(format!("{err}").contains("image"));
}

// ==================== Converse Request Full Tests ====================

#[test]
fn test_converse_request_serialization() {
    let request = ConverseRequest {
        messages: vec![ConverseMessage {
            role: "user".to_string(),
            content: vec![ContentBlock::Text {
                text: "Hello".to_string(),
            }],
        }],
        system: Some(vec![SystemMessage {
            text: Some("Be helpful".to_string()),
            guardrail_content: None,
        }]),
        inference_config: Some(InferenceConfig {
            max_tokens: Some(100),
            temperature: Some(0.5),
            top_p: None,
            stop_sequences: None,
        }),
        prompt_variables: None,
        tool_config: None,
        guardrail_config: None,
        additional_model_request_fields: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert!(json["messages"].is_array());
    assert!(json["system"].is_array());
    assert_eq!(json["inferenceConfig"]["maxTokens"], 100);
}

#[test]
fn test_converse_request_deserialization() {
    let json = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [{"text": "Hello"}]
            }],
        "inferenceConfig": {
            "maxTokens": 200
        }
    });

    let request: ConverseRequest = serde_json::from_value(json).unwrap();
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, "user");
}

#[test]
fn prompt_management_transform_uses_supported_request_shape() {
    let mut request =
        ChatRequest::new("bedrock/arn:aws:bedrock:us-east-1:123456789012:prompt/ABC123:1")
            .add_user_message("hello");
    request.extra_params.insert(
        "promptVariables".to_string(),
        serde_json::json!({
            "topic": { "text": "Bedrock" }
        }),
    );

    let Ok(converse) = transform_to_converse(&request) else {
        panic!("prompt-management request should be supported");
    };
    let Ok(json) = serde_json::to_value(&converse) else {
        panic!("prompt-management request should serialize");
    };

    assert_eq!(json["messages"][0]["content"][0]["text"], "hello");
    assert_eq!(json["promptVariables"]["topic"]["text"], "Bedrock");
    assert!(json.get("system").is_none());
    assert!(json.get("inferenceConfig").is_none());
    assert!(json.get("toolConfig").is_none());
}

#[test]
fn prompt_management_transform_normalizes_string_prompt_variables() {
    let mut request =
        ChatRequest::new("bedrock/arn:aws:bedrock:us-east-1:123456789012:prompt/ABC123:1")
            .add_user_message("hello");
    request.extra_params.insert(
        "promptVariables".to_string(),
        serde_json::json!({
            "topic": "Bedrock"
        }),
    );

    let Ok(converse) = transform_to_converse(&request) else {
        panic!("string promptVariables should be normalized");
    };
    let Ok(json) = serde_json::to_value(&converse) else {
        panic!("prompt-management request should serialize");
    };

    assert_eq!(json["promptVariables"]["topic"]["text"], "Bedrock");
}

#[test]
fn prompt_management_transform_rejects_invalid_prompt_variables() {
    let mut request =
        ChatRequest::new("bedrock/arn:aws:bedrock:us-east-1:123456789012:prompt/ABC123:1")
            .add_user_message("hello");
    request.extra_params.insert(
        "promptVariables".to_string(),
        serde_json::json!({
            "topic": { "value": "Bedrock" }
        }),
    );

    let err = match transform_to_converse(&request) {
        Ok(_) => panic!("invalid promptVariables should be rejected"),
        Err(err) => err,
    };

    assert!(format!("{err}").contains("promptVariables values"));
}

#[test]
fn prompt_management_transform_rejects_disallowed_system_message() {
    let request = ChatRequest::new("arn:aws:bedrock:us-east-1:123456789012:prompt/ABC123:1")
        .add_system_message("system")
        .add_user_message("hello");

    let err = match transform_to_converse(&request) {
        Ok(_) => panic!("system messages must be rejected for prompt-management ARNs"),
        Err(err) => err,
    };

    assert!(format!("{err}").contains("system messages"));
}

// ==================== Tool Result Content Tests ====================

#[test]
fn test_tool_result_content_text() {
    let content = ToolResultContent::Text {
        text: "Result text".to_string(),
    };

    let json = serde_json::to_value(&content).unwrap();
    assert_eq!(json["text"], "Result text");
}

#[test]
fn test_tool_result_content_image() {
    let content = ToolResultContent::Image {
        image: ImageBlock {
            format: "jpeg".to_string(),
            source: ImageSource::Bytes {
                bytes: "imagedata".to_string(),
            },
        },
    };

    let json = serde_json::to_value(&content).unwrap();
    assert_eq!(json["image"]["format"], "jpeg");
    assert_eq!(json["image"]["source"]["bytes"], "imagedata");
}

#[test]
fn test_tool_result_content_document() {
    let content = ToolResultContent::Document {
        document: DocumentBlock {
            format: "txt".to_string(),
            name: "result.txt".to_string(),
            source: DocumentSource::Bytes {
                bytes: "docdata".to_string(),
            },
        },
    };

    let json = serde_json::to_value(&content).unwrap();
    assert_eq!(json["document"]["name"], "result.txt");
    assert_eq!(json["document"]["source"]["bytes"], "docdata");
}

// ==================== Tool Config Tests ====================

#[test]
fn test_tool_config_with_tools() {
    let config = ToolConfig {
        tools: vec![
            ToolSpec {
                tool_spec: ToolSpecDefinition {
                    name: "tool1".to_string(),
                    description: "First tool".to_string(),
                    input_schema: InputSchema {
                        json: serde_json::json!({}),
                    },
                },
            },
            ToolSpec {
                tool_spec: ToolSpecDefinition {
                    name: "tool2".to_string(),
                    description: "Second tool".to_string(),
                    input_schema: InputSchema {
                        json: serde_json::json!({}),
                    },
                },
            },
        ],
        tool_choice: Some(ToolChoice::Auto),
    };

    let json = serde_json::to_value(&config)
        .unwrap_or_else(|err| panic!("toolConfig should serialize: {err}"));
    let tools = json["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("tools should serialize as an array"));
    assert_eq!(tools.len(), 2);
    assert_eq!(json["toolChoice"]["auto"], serde_json::json!({}));
}

#[test]
fn test_guardrail_content() {
    let content = GuardrailContent {
        text: "Safety message".to_string(),
    };

    let json = serde_json::to_value(&content).unwrap();
    assert_eq!(json["text"], "Safety message");
}

#[test]
fn test_content_block_guardrail() {
    let block = ContentBlock::GuardrailContent {
        guardrail_content: GuardrailContent {
            text: "Guardrail text".to_string(),
        },
    };

    let json = serde_json::to_value(&block).unwrap();
    assert!(json.get("guardrailContent").is_some());
}
