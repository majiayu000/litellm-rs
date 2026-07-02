use super::create_test_provider;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

// ==================== Transform Response Tests ====================

#[tokio::test]
async fn test_transform_response_claude() {
    let provider = create_test_provider();

    let response = serde_json::json!({
        "content": [{"text": "Hello! I'm doing well."}],
        "usage": {
            "input_tokens": 10,
            "output_tokens": 20
        }
    });
    let response_bytes = serde_json::to_vec(&response).unwrap();

    let result = provider
        .transform_response(
            &response_bytes,
            "anthropic.claude-3-sonnet-20240229",
            "test-request-id",
        )
        .await;

    assert!(result.is_ok());
    let chat_response = result.unwrap();
    assert_eq!(chat_response.model, "anthropic.claude-3-sonnet-20240229");
    assert!(!chat_response.choices.is_empty());
}

#[tokio::test]
async fn test_transform_response_titan() {
    let provider = create_test_provider();

    let response = serde_json::json!({
        "results": [{"outputText": "Hello from Titan!"}],
        "inputTextTokenCount": 5
    });
    let response_bytes = serde_json::to_vec(&response).unwrap();

    let result = provider
        .transform_response(
            &response_bytes,
            "amazon.titan-text-express-v1",
            "test-request-id",
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_transform_response_nova() {
    let provider = create_test_provider();

    let response = serde_json::json!({
        "content": [{"text": "Nova response"}],
        "usage": {
            "input_tokens": 15,
            "output_tokens": 25
        }
    });
    let response_bytes = serde_json::to_vec(&response).unwrap();

    let result = provider
        .transform_response(&response_bytes, "amazon.nova-pro-v1:0", "test-request-id")
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_transform_response_mistral() {
    let provider = create_test_provider();

    let response = serde_json::json!({
        "outputs": [{"text": "Mistral response"}]
    });
    let response_bytes = serde_json::to_vec(&response).unwrap();

    let result = provider
        .transform_response(
            &response_bytes,
            "mistral.mistral-large-2407-v1:0",
            "test-request-id",
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_transform_response_ai21() {
    let provider = create_test_provider();

    let response = serde_json::json!({
        "completions": [{"data": {"text": "AI21 response"}}]
    });
    let response_bytes = serde_json::to_vec(&response).unwrap();

    let result = provider
        .transform_response(
            &response_bytes,
            "ai21.jamba-1-5-large-v1:0",
            "test-request-id",
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_transform_response_cohere() {
    let provider = create_test_provider();

    let response = serde_json::json!({
        "text": "Cohere response"
    });
    let response_bytes = serde_json::to_vec(&response).unwrap();

    let result = provider
        .transform_response(
            &response_bytes,
            "cohere.command-r-plus-v1:0",
            "test-request-id",
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_transform_response_invalid_json() {
    let provider = create_test_provider();

    let response_bytes = b"not valid json";

    let result = provider
        .transform_response(
            response_bytes,
            "anthropic.claude-3-sonnet-20240229",
            "test-request-id",
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_transform_response_unknown_model() {
    let provider = create_test_provider();

    let response = serde_json::json!({"text": "response"});
    let response_bytes = serde_json::to_vec(&response).unwrap();

    let result = provider
        .transform_response(&response_bytes, "unknown.model-v1", "test-request-id")
        .await;

    assert!(result.is_err());
}
