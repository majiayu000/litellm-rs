use super::super::config::BedrockConfig;
use super::super::provider::BedrockProvider;
use super::create_test_provider;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

// ==================== Provider Creation Tests ====================

#[tokio::test]
async fn test_bedrock_provider_creation() {
    let config = BedrockConfig {
        aws_access_key_id: "AKIATEST123456789012".to_string(),
        aws_secret_access_key: "test_secret".to_string(),
        aws_session_token: None,
        aws_region: "us-east-1".to_string(),
        timeout_seconds: 30,
        max_retries: 3,
        endpoint_access: Default::default(),
    };

    let provider = BedrockProvider::new(config).await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(provider.name(), "bedrock");
    assert!(
        provider
            .capabilities()
            .contains(&ProviderCapability::ChatCompletion)
    );
}

#[tokio::test]
async fn test_bedrock_provider_creation_with_session_token() {
    let config = BedrockConfig {
        aws_access_key_id: "AKIATEST123456789012".to_string(),
        aws_secret_access_key: "test_secret".to_string(),
        aws_session_token: Some("session_token".to_string()),
        aws_region: "us-west-2".to_string(),
        timeout_seconds: 60,
        max_retries: 5,
        endpoint_access: Default::default(),
    };

    let provider = BedrockProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_bedrock_provider_creation_invalid_region() {
    let config = BedrockConfig {
        aws_access_key_id: "AKIATEST123456789012".to_string(),
        aws_secret_access_key: "test_secret".to_string(),
        aws_session_token: None,
        aws_region: "invalid-region-xyz".to_string(),
        timeout_seconds: 30,
        max_retries: 3,
        endpoint_access: Default::default(),
    };

    let provider = BedrockProvider::new(config).await;
    assert!(provider.is_err());
}

#[tokio::test]
async fn test_bedrock_provider_creation_empty_credentials() {
    let config = BedrockConfig {
        aws_access_key_id: "".to_string(),
        aws_secret_access_key: "test_secret".to_string(),
        aws_session_token: None,
        aws_region: "us-east-1".to_string(),
        timeout_seconds: 30,
        max_retries: 3,
        endpoint_access: Default::default(),
    };

    let provider = BedrockProvider::new(config).await;
    assert!(provider.is_err());
}

// ==================== Provider Capabilities Tests ====================

#[test]
fn test_provider_name() {
    let provider = create_test_provider();
    assert_eq!(provider.name(), "bedrock");
}

#[test]
fn test_provider_capabilities() {
    let provider = create_test_provider();
    let caps = provider.capabilities();

    assert!(caps.contains(&ProviderCapability::ChatCompletion));
    assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    assert!(caps.contains(&ProviderCapability::FunctionCalling));
    assert!(caps.contains(&ProviderCapability::Embeddings));
}

#[test]
fn test_provider_supported_openai_params() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("anthropic.claude-3-sonnet-20240229");

    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"top_p"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"stop"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"tool_choice"));
}

#[test]
fn test_provider_models_empty_initially() {
    let provider = create_test_provider();
    assert!(provider.models().is_empty());
}

// ==================== Embedding Model Detection Tests ====================

#[test]
fn test_embedding_model_detection() {
    let provider = create_test_provider();

    assert!(provider.is_embedding_model("amazon.titan-embed-text-v1"));
    assert!(provider.is_embedding_model("cohere.embed-english-v3"));
    assert!(provider.is_embedding_model("my-embed-model"));
    assert!(!provider.is_embedding_model("anthropic.claude-3-sonnet"));
    assert!(!provider.is_embedding_model("amazon.titan-text-express-v1"));
}
