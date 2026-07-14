use super::create_provider;
use crate::config::models::provider::ProviderConfig;
use crate::core::net::ProviderEndpointAccess::PrivateNetwork;
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
    let accepted =
        tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept()).await;
    assert!(accepted.is_err(), "unwired endpoint gate must not connect");
}
#[tokio::test]
async fn invalid_or_implicit_private_endpoints_fail_closed() {
    let mut gateway = ProviderConfig {
        name: "openai-test".into(),
        provider_type: "openai".into(),
        api_key: "sk-test".into(),
        ..Default::default()
    };
    gateway.settings.insert("api_base".into(), 42.into());
    let error = crate::config::Validate::validate(&gateway).expect_err("must reject");
    assert!(error.contains("must be a string"), "{error}");
    let result = create_provider(gateway.clone()).await;
    let error = result.expect_err("must reject");
    assert!(error.to_string().contains("must be a string"), "{error}");
    gateway.settings.clear();
    gateway.endpoint_access = PrivateNetwork;
    let error = create_provider(gateway).await.expect_err("must reject");
    assert!(error.to_string().contains("requires a base URL"), "{error}");
    for config in [
        serde_json::json!({"api_key": "sk-test", "api_base": 42}),
        serde_json::json!({"api_key": "sk-test", "endpoint_access": "private_network"}),
    ] {
        let error = Provider::from_config_async(ProviderType::OpenAI, config)
            .await
            .expect_err("must reject");
        assert!(error.to_string().contains("endpoint"), "{error}");
    }
}
