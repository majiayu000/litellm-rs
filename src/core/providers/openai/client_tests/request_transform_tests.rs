use super::*;

// ==================== Transform Request Tests ====================

#[test]
fn test_transform_chat_request_basic() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["model"], "gpt-4");
    assert!(transformed["messages"].is_array());
}

#[test]
fn test_transform_chat_request_with_temperature() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        temperature: Some(0.7),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert!(transformed.get("temperature").is_some());
}

#[test]
fn test_transform_chat_request_with_max_tokens() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        max_tokens: Some(1000),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["max_tokens"], 1000);
}

#[test]
fn test_transform_chat_request_with_max_completion_tokens() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "o1-preview".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        max_completion_tokens: Some(2000),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["max_completion_tokens"], 2000);
}

#[test]
fn test_transform_chat_request_with_top_p() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        top_p: Some(0.9),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert!(transformed.get("top_p").is_some());
}

#[test]
fn test_transform_chat_request_with_user() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        user: Some("user-123".to_string()),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["user"], "user-123");
}

#[test]
fn test_transform_chat_request_with_seed() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        seed: Some(42),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["seed"], 42);
}

#[test]
fn test_transform_chat_request_with_n() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        n: Some(3),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["n"], 3);
}

#[test]
fn test_transform_chat_request_forwards_typed_params_and_extras() {
    let provider = create_test_provider();
    let request = chat_request_with_typed_params("modalities");

    let Ok(transformed) = provider.transform_chat_request(request) else {
        panic!("transform_chat_request must succeed");
    };
    assert_typed_params_forwarded(&transformed, "modalities");
}

#[tokio::test]
async fn test_openai_like_transform_request_forwards_typed_params_and_extras() {
    let config = OpenAILikeConfig::new("https://api.example.com/v1").with_skip_api_key(true);
    let Ok(provider) = OpenAILikeProvider::new(config).await else {
        panic!("provider creation must succeed");
    };
    let request = chat_request_with_typed_params("provider_flag");

    let Ok(transformed) = provider
        .transform_request(request, RequestContext::default())
        .await
    else {
        panic!("transform_request must succeed");
    };
    assert_typed_params_forwarded(&transformed, "provider_flag");
}

// ==================== Map OpenAI Params Tests ====================

#[tokio::test]
async fn test_map_openai_params_passthrough() {
    let provider = create_test_provider();

    let mut params = HashMap::new();
    params.insert("temperature".to_string(), serde_json::json!(0.7));
    params.insert("max_tokens".to_string(), serde_json::json!(100));

    let result = provider.map_openai_params(params.clone(), "gpt-4").await;
    assert!(result.is_ok());

    let mapped = result.unwrap();
    // OpenAI params should pass through unchanged
    assert_eq!(mapped, params);
}

// ==================== Cost Calculation Tests ====================

#[tokio::test]
async fn test_calculate_cost() {
    let provider = create_test_provider();

    let cost = provider.calculate_cost("gpt-4o-mini", 1000, 500).await;
    assert!(cost.is_ok());

    let cost_value = cost.unwrap();
    assert!((cost_value - 0.00045).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_calculate_cost_zero_tokens() {
    let provider = create_test_provider();

    let cost = provider.calculate_cost("gpt-4", 0, 0).await;
    assert!(cost.is_ok());
    assert!((cost.unwrap() - 0.0).abs() < 0.0001);
}
