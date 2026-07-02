use super::*;

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
