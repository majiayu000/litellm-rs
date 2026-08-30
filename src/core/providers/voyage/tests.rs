use super::{VoyageEmbeddingData, VoyageEmbeddingResponse, VoyageProvider, VoyageUsage};
use crate::core::net::ProviderEndpointAccess;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::health::HealthStatus;
use crate::core::types::model::ProviderCapability;

fn provider(models: &[&str]) -> Result<VoyageProvider, crate::core::providers::ProviderError> {
    VoyageProvider::new(
        "test-key".to_string(),
        Some("http://127.0.0.1:1/v1"),
        ProviderEndpointAccess::PrivateNetwork,
        1,
        0,
        &models
            .iter()
            .map(|model| (*model).to_string())
            .collect::<Vec<_>>(),
    )
}

#[test]
fn configured_models_keep_exact_retrieval_capabilities() {
    let provider = provider(&["voyage-4", "rerank-2.5"]).expect("known models should bind");

    assert!(provider.supports_capability_for_model("voyage-4", &ProviderCapability::Embeddings));
    assert!(!provider.supports_capability_for_model("voyage-4", &ProviderCapability::Rerank));
    assert!(provider.supports_capability_for_model("rerank-2.5", &ProviderCapability::Rerank));
}

#[test]
fn legacy_voyage_3_models_keep_fixed_dimensions() {
    let provider =
        provider(&["voyage-3", "voyage-3-large"]).expect("known embedding models should bind");

    assert!(
        !provider
            .get_supported_openai_params("voyage-3")
            .contains(&"dimensions")
    );
    assert!(
        provider
            .get_supported_openai_params("voyage-3-large")
            .contains(&"dimensions")
    );
}

#[test]
fn unknown_configured_model_fails_closed() {
    let error = provider(&["voyage-4-lookalike"]).expect_err("unknown model must fail");

    assert!(error.to_string().contains("Unknown Voyage model"));
}

#[test]
fn embedding_response_is_ordered_by_index() {
    let response = VoyageEmbeddingResponse {
        object: "list".to_string(),
        data: vec![
            VoyageEmbeddingData {
                object: "embedding".to_string(),
                embedding: vec![1.0],
                index: 1,
            },
            VoyageEmbeddingData {
                object: "embedding".to_string(),
                embedding: vec![0.0],
                index: 0,
            },
        ],
        model: "voyage-4".to_string(),
        usage: VoyageUsage { total_tokens: 2 },
    };

    let transformed = VoyageProvider::transform_embedding_response(response, 2)
        .expect("valid out-of-order embeddings should be normalized");

    assert_eq!(transformed.data[0].index, 0);
    assert_eq!(transformed.data[0].embedding, vec![0.0]);
    assert_eq!(transformed.data[1].index, 1);
    assert_eq!(transformed.data[1].embedding, vec![1.0]);
}

#[tokio::test]
async fn health_is_unknown_without_a_probe() {
    let provider = provider(&["voyage-4"]).expect("provider should bind");

    assert_eq!(provider.health_check().await, HealthStatus::Unknown);
}
