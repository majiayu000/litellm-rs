use crate::core::providers::provider_type::{ProviderType, all_non_custom_provider_types};
use crate::core::providers::{Provider, ProviderError};

#[tokio::test]
async fn supported_variants_do_not_fallthrough_to_not_implemented() {
    for provider_type in Provider::factory_supported_provider_types() {
        let result =
            Provider::from_config_async(provider_type.clone(), serde_json::json!({})).await;
        // Success is fine (e.g. local catalog providers with skip_api_key);
        // a real config error is also fine. Only NotImplemented is wrong.
        if let Err(error) = result {
            assert!(
                !matches!(error, ProviderError::NotImplemented { .. }),
                "{provider_type:?} unexpectedly fell through to NotImplemented: {error}"
            );
        }
    }
}

#[tokio::test]
async fn unsupported_variants_return_not_implemented() {
    let supported = Provider::factory_supported_provider_types();

    for provider_type in all_non_custom_provider_types() {
        if supported.contains(&provider_type) {
            continue;
        }

        let error = Provider::from_config_async(provider_type.clone(), serde_json::json!({}))
            .await
            .expect_err("Expected unsupported provider to fail");
        assert!(
            matches!(error, ProviderError::NotImplemented { .. }),
            "Expected NotImplemented for {provider_type:?}, got {error}"
        );
        assert_eq!(
            error.provider(),
            provider_type.to_string(),
            "NotImplemented provider name should identify the requested provider"
        );
    }
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn ollama_factory_creates_policy_wired_native_provider() {
    let provider = Provider::from_config_async(
        ProviderType::Ollama,
        serde_json::json!({
            "base_url": "http://127.0.0.1:11434",
            "endpoint_access": "private_network"
        }),
    )
    .await
    .unwrap_or_else(|error| panic!("ollama should create a native provider: {error}"));

    assert!(matches!(provider, Provider::Ollama(_)));
    assert_eq!(provider.name(), "ollama");
    assert_eq!(provider.provider_type(), ProviderType::Ollama);
    let capabilities = provider.capabilities();
    assert!(capabilities.contains(&crate::core::types::model::ProviderCapability::ChatCompletion));
    assert!(
        capabilities.contains(&crate::core::types::model::ProviderCapability::ChatCompletionStream)
    );
    assert!(capabilities.contains(&crate::core::types::model::ProviderCapability::Embeddings));
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn ollama_factory_rejects_explicit_public_loopback() {
    let error = Provider::from_config_async(
        ProviderType::Ollama,
        serde_json::json!({
            "api_base": "http://127.0.0.1:11434",
            "endpoint_access": "public_only"
        }),
    )
    .await
    .expect_err("public-only Ollama must reject an explicit loopback endpoint");

    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(error.to_string().contains("private or reserved"));
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn ollama_factory_rejects_conflicting_endpoint_aliases() {
    let error = Provider::from_config_async(
        ProviderType::Ollama,
        serde_json::json!({
            "base_url": "https://example.com",
            "api_base": "http://127.0.0.1:11434",
            "endpoint_access": "public_only"
        }),
    )
    .await
    .expect_err("conflicting Ollama endpoint aliases must fail closed");

    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(error.to_string().contains("different endpoints"));
}

#[cfg(not(feature = "providers-extended"))]
#[tokio::test]
async fn ollama_factory_requires_providers_extended() {
    let error = Provider::from_config_async(ProviderType::Ollama, serde_json::json!({}))
        .await
        .expect_err("ollama should require providers-extended");

    assert!(matches!(error, ProviderError::NotImplemented { .. }));
    assert_eq!(error.provider(), "ollama");
    assert!(error.to_string().contains("providers-extended"));
}
