use super::super::provider::BEDROCK_CAPABILITIES;
use super::create_test_provider;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

// ==================== Cost Calculation Tests ====================

#[tokio::test]
async fn test_calculate_cost_known_model() {
    let provider = create_test_provider();

    let cost = provider
        .calculate_cost("anthropic.claude-3-opus-20240229", 1000, 500)
        .await;

    assert!(cost.is_ok());
    let cost_value = cost.unwrap();
    assert!(cost_value > 0.0);
}

#[tokio::test]
async fn test_calculate_cost_unknown_model() {
    let provider = create_test_provider();

    let cost = provider.calculate_cost("unknown.model-v1", 1000, 500).await;

    assert!(cost.is_err());
}

#[tokio::test]
async fn test_calculate_cost_zero_tokens() {
    let provider = create_test_provider();

    let cost = provider
        .calculate_cost("anthropic.claude-3-haiku-20240307", 0, 0)
        .await;

    assert!(cost.is_ok());
    assert!((cost.unwrap() - 0.0).abs() < 0.0001);
}

// ==================== Error Mapper Tests ====================

#[test]
fn test_get_error_mapper() {
    let provider = create_test_provider();
    let mapper = provider.get_error_mapper();

    // Test that we can get an error mapper and use it
    let err = mapper.map_http_error(500, "test error");
    assert!(err.to_string().contains("test error") || err.to_string().contains("500"));
}

// ==================== Client Access Tests ====================

#[test]
fn test_agents_client_access() {
    let provider = create_test_provider();
    let _agents_client = provider.agents_client();
    // Just verify we can access the agents client
}

#[test]
fn test_knowledge_bases_client_access() {
    let provider = create_test_provider();
    let _kb_client = provider.knowledge_bases_client();
    // Just verify we can access the knowledge bases client
}

#[test]
fn test_batch_client_access() {
    let provider = create_test_provider();
    let _batch_client = provider.batch_client();
    // Just verify we can access the batch client
}

#[test]
fn test_guardrails_client_access() {
    let provider = create_test_provider();
    let _guardrails_client = provider.guardrails_client();
    // Just verify we can access the guardrails client
}

// ==================== Capabilities Constants Tests ====================

#[test]
fn test_bedrock_capabilities_constant() {
    assert!(BEDROCK_CAPABILITIES.contains(&ProviderCapability::ChatCompletion));
    assert!(BEDROCK_CAPABILITIES.contains(&ProviderCapability::ChatCompletionStream));
    assert!(BEDROCK_CAPABILITIES.contains(&ProviderCapability::FunctionCalling));
    assert!(BEDROCK_CAPABILITIES.contains(&ProviderCapability::Embeddings));
    assert_eq!(BEDROCK_CAPABILITIES.len(), 4);
}

// ==================== Provider Clone/Debug Tests ====================

#[test]
fn test_provider_clone() {
    let provider = create_test_provider();
    let cloned = provider.clone();

    assert_eq!(provider.name(), cloned.name());
    assert_eq!(provider.capabilities().len(), cloned.capabilities().len());
}

#[test]
fn test_provider_debug() {
    let provider = create_test_provider();
    let debug_str = format!("{:?}", provider);

    assert!(debug_str.contains("BedrockProvider"));
}
