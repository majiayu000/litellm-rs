use super::*;
use crate::core::types::content::ContentPart;

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
            "candidatesTokenCount": 50,
            "totalTokenCount": 150
        }
    });
    let model = VertexAIModel::GeminiPro;

    let result = transformer
        .transform_chat_response(response, &model)
        .unwrap();
    let usage = result.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);
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
