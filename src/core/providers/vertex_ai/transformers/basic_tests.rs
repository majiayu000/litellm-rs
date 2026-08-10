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
        model: "gemini-1.5-pro".to_string(),
        messages: vec![create_test_message(MessageRole::User, "Hello")],
        ..Default::default()
    }
}

// ==================== GeminiTransformer Tests ====================

#[test]
fn test_gemini_transformer_new() {
    let transformer = GeminiTransformer::new();
    assert!(format!("{:?}", transformer).contains("GeminiTransformer"));
}

#[test]
fn test_gemini_transformer_default() {
    let transformer = GeminiTransformer;
    assert!(format!("{:?}", transformer).contains("GeminiTransformer"));
}

#[test]
fn test_gemini_transformer_clone() {
    let transformer = GeminiTransformer::new();
    let cloned = transformer.clone();
    assert!(format!("{:?}", cloned).contains("GeminiTransformer"));
}

#[test]
fn test_transform_chat_request_basic() {
    let transformer = GeminiTransformer::new();
    let request = create_test_request();
    let model = VertexAIModel::GeminiPro;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body["contents"].is_array());
    assert!(body["generationConfig"].is_object());
}

#[test]
fn test_transform_chat_request_with_system_message() {
    let transformer = GeminiTransformer::new();
    let request = ChatRequest {
        model: "gemini-1.5-pro".to_string(),
        messages: vec![
            create_test_message(MessageRole::System, "You are helpful"),
            create_test_message(MessageRole::User, "Hello"),
        ],
        ..Default::default()
    };
    let model = VertexAIModel::GeminiPro;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body["systemInstruction"].is_object());
    assert!(body["systemInstruction"]["parts"].is_array());
}

#[test]
fn test_transform_chat_request_with_temperature() {
    let transformer = GeminiTransformer::new();
    let mut request = create_test_request();
    request.temperature = Some(0.7);
    let model = VertexAIModel::GeminiPro;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!((body["generationConfig"]["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
}

#[test]
fn test_transform_chat_request_with_max_tokens() {
    let transformer = GeminiTransformer::new();
    let mut request = create_test_request();
    request.max_tokens = Some(1000);
    let model = VertexAIModel::GeminiPro;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 1000);
}

#[test]
fn test_transform_chat_request_with_top_p() {
    let transformer = GeminiTransformer::new();
    let mut request = create_test_request();
    request.top_p = Some(0.9);
    let model = VertexAIModel::GeminiPro;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!((body["generationConfig"]["topP"].as_f64().unwrap() - 0.9).abs() < 0.001);
}

#[test]
fn test_transform_chat_request_with_stop_sequences() {
    let transformer = GeminiTransformer::new();
    let mut request = create_test_request();
    request.stop = Some(vec!["END".to_string(), "STOP".to_string()]);
    let model = VertexAIModel::GeminiPro;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    let stop_seqs = body["generationConfig"]["stopSequences"]
        .as_array()
        .unwrap();
    assert_eq!(stop_seqs.len(), 2);
}

#[test]
fn test_transform_chat_request_multi_turn() {
    let transformer = GeminiTransformer::new();
    let request = ChatRequest {
        model: "gemini-1.5-pro".to_string(),
        messages: vec![
            create_test_message(MessageRole::User, "Hello"),
            create_test_message(MessageRole::Assistant, "Hi there!"),
            create_test_message(MessageRole::User, "How are you?"),
        ],
        ..Default::default()
    };
    let model = VertexAIModel::GeminiPro;

    let result = transformer.transform_chat_request(&request, &model);
    assert!(result.is_ok());
    let body = result.unwrap();
    let contents = body["contents"].as_array().unwrap();
    assert_eq!(contents.len(), 3);
}

#[test]
fn test_transform_chat_response_basic() {
    let transformer = GeminiTransformer::new();
    let response = json!({
        "candidates": [{
            "content": {
                "parts": [{"text": "Hello! How can I help?"}]
            },
            "finishReason": "STOP"
        }],
        "usageMetadata": {
            "promptTokenCount": 10,
            "candidatesTokenCount": 20,
            "totalTokenCount": 30
        }
    });
    let model = VertexAIModel::GeminiPro;

    let result = transformer.transform_chat_response(response, &model);
    assert!(result.is_ok());
    let chat_response = result.unwrap();
    assert_eq!(chat_response.object, "chat.completion");
    assert_eq!(chat_response.choices.len(), 1);
    assert_eq!(
        chat_response.choices[0].finish_reason,
        Some(FinishReason::Stop)
    );
}

#[test]
fn test_transform_chat_response_finish_reasons() {
    let transformer = GeminiTransformer::new();
    let model = VertexAIModel::GeminiPro;

    // Test STOP
    let response = json!({
        "candidates": [{"content": {"parts": [{"text": "Done"}]}, "finishReason": "STOP"}]
    });
    let result = transformer
        .transform_chat_response(response, &model)
        .unwrap();
    assert_eq!(result.choices[0].finish_reason, Some(FinishReason::Stop));

    // Test MAX_TOKENS
    let response = json!({
        "candidates": [{"content": {"parts": [{"text": "Done"}]}, "finishReason": "MAX_TOKENS"}]
    });
    let result = transformer
        .transform_chat_response(response, &model)
        .unwrap();
    assert_eq!(result.choices[0].finish_reason, Some(FinishReason::Length));

    // Test SAFETY
    let response = json!({
        "candidates": [{"content": {"parts": [{"text": ""}]}, "finishReason": "SAFETY"}]
    });
    let result = transformer
        .transform_chat_response(response, &model)
        .unwrap();
    assert_eq!(
        result.choices[0].finish_reason,
        Some(FinishReason::ContentFilter)
    );
}
