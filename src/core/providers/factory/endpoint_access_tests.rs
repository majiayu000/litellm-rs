use super::create_provider;
use crate::config::models::provider::ProviderConfig;
use crate::core::net::ProviderEndpointAccess::{PrivateNetwork, PublicOnly};
use crate::core::providers::{Provider, ProviderType};
#[cfg(not(feature = "providers-extra"))]
#[tokio::test]
async fn azure_fallbacks_propagate_private_endpoint_access() {
    for (provider_type, base_url) in [
        (ProviderType::Azure, "http://127.0.0.1/openai/deployments/x"),
        (ProviderType::AzureAI, "http://127.0.0.1/models"),
    ] {
        let provider = Provider::from_config_async(
            provider_type,
            serde_json::json!({"api_key": "x", "base_url": base_url,
                "endpoint_access": "private_network"}),
        )
        .await
        .unwrap_or_else(|error| panic!("Azure fallback must propagate access: {error}"));
        assert!(matches!(provider, Provider::OpenAILike(provider)
            if provider.config().base.endpoint_access == PrivateNetwork));
    }
}
#[tokio::test]
async fn unwired_gateway_and_direct_endpoint_config_fail_closed() {
    for (provider_type, selector) in [
        (ProviderType::Cloudflare, "cloudflare"),
        (ProviderType::FalAI, "fal_ai"),
        (ProviderType::Replicate, "replicate"),
        (ProviderType::GitHubCopilot, "github_copilot"),
    ] {
        for direct_config in [
            serde_json::json!({"endpoint_access": "public_only"}),
            serde_json::json!({"base_url": "https://x.test"}),
            serde_json::json!({"api_base": "https://x.test"}),
        ] {
            let error = Provider::from_config_async(provider_type.clone(), direct_config)
                .await
                .expect_err("must reject unwired endpoint");
            assert!(error.to_string().contains("not policy-wired"));
        }
        let gateway_config = ProviderConfig {
            provider_type: selector.to_string(),
            base_url: Some("https://x.test".to_string()),
            ..Default::default()
        };
        let result = create_provider(gateway_config.clone()).await;
        let error = result.expect_err("must reject");
        assert!(error.to_string().contains("not policy-wired"));
        for key in ["base_url", "api_base"] {
            let mut settings_config = gateway_config.clone();
            settings_config.base_url = None;
            let settings = &mut settings_config.settings;
            settings.insert(key.into(), "https://x.test".into());
            let result = create_provider(settings_config).await;
            let error = result.expect_err("must reject");
            assert!(error.to_string().contains("not policy-wired"));
        }
    }
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener must bind");
    let port = listener.local_addr().unwrap().port();
    let config = ProviderConfig {
        provider_type: "cloudflare".to_string(),
        base_url: Some(format!("http://localhost:{port}")),
        ..Default::default()
    };
    let result = create_provider(config).await;
    let error = result.expect_err("must reject before construction");
    assert!(error.to_string().contains("not policy-wired"));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
            .await
            .is_err(),
        "unwired endpoint gate must not connect to the listener"
    );
}
#[tokio::test]
async fn catalog_local_providers_require_explicit_private_access() {
    for (name, definition) in super::provider_registry::PROVIDER_CATALOG.iter() {
        let is_local = definition.base_url.contains("localhost");
        let config = ProviderConfig {
            name: (*name).to_string(),
            provider_type: (*name).to_string(),
            api_key: "test-key".into(),
            endpoint_access: if is_local { PrivateNetwork } else { PublicOnly },
            ..Default::default()
        };
        let mut opposite = config.clone();
        opposite.endpoint_access = if is_local { PublicOnly } else { PrivateNetwork };
        if is_local {
            assert!(create_provider(opposite).await.is_err());
        } else {
            assert!(crate::config::Validate::validate(&opposite).is_err());
        }
        assert!(crate::config::Validate::validate(&config).is_ok());
        let provider = create_provider(config)
            .await
            .unwrap_or_else(|error| panic!("Catalog provider '{name}' should work: {error}"));
        assert_eq!(provider.capabilities(), definition.capabilities);
    }
}
