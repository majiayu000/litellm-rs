use super::VoyageProvider;
use crate::core::net::ProviderEndpointAccess;
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
fn unknown_configured_model_fails_closed() {
    let error = provider(&["voyage-4-lookalike"]).expect_err("unknown model must fail");

    assert!(error.to_string().contains("Unknown Voyage model"));
}
