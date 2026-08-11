use super::create_provider;
#[cfg(feature = "providers-extended")]
use crate::core::providers::Provider;
use crate::core::providers::ProviderError;

#[tokio::test]
async fn reports_unknown_custom_provider() {
    let config = crate::config::models::provider::ProviderConfig {
        name: "my-custom-provider".to_string(),
        provider_type: "".to_string(),
        api_key: "test-key".to_string(),
        ..Default::default()
    };

    let error = create_provider(config)
        .await
        .expect_err("Expected unknown custom provider to fail");
    // Unknown provider strings produce InvalidRequest at selector parsing time.
    assert!(
        matches!(error, ProviderError::InvalidRequest { .. }),
        "Expected InvalidRequest error, got {error}"
    );
    assert!(
        error.to_string().contains("my-custom-provider"),
        "Expected custom provider name in error, got {error}"
    );
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn creates_native_ollama_from_gateway_config() {
    let config = crate::config::models::provider::ProviderConfig {
        name: "local-ollama".to_string(),
        provider_type: "ollama".to_string(),
        base_url: Some("http://127.0.0.1:11434".to_string()),
        endpoint_access: crate::core::net::ProviderEndpointAccess::PrivateNetwork,
        ..Default::default()
    };

    let provider = create_provider(config)
        .await
        .unwrap_or_else(|error| panic!("gateway config should create native Ollama: {error}"));
    assert!(matches!(provider, Provider::Ollama(_)));
}
