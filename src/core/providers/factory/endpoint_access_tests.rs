use super::create_provider;
use crate::config::models::provider::ProviderConfig;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::{Provider, ProviderType};
use tokio::net::TcpListener;
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
        let Provider::OpenAILike(provider) = provider else {
            panic!("Azure fallback should use OpenAILike")
        };
        assert_eq!(
            provider.config().base.endpoint_access,
            ProviderEndpointAccess::PrivateNetwork
        );
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
            serde_json::json!({"base_url": "https://unwired.example.test"}),
            serde_json::json!({"api_base": "https://unwired.example.test"}),
        ] {
            let error = Provider::from_config_async(provider_type.clone(), direct_config)
                .await
                .expect_err("unwired direct endpoint config must fail closed");
            assert!(error.to_string().contains("not policy-wired"));
        }
        let gateway_config = ProviderConfig {
            name: format!("unwired-{selector}"),
            provider_type: selector.to_string(),
            base_url: Some("https://unwired.example.test".to_string()),
            ..Default::default()
        };
        let error = create_provider(gateway_config.clone())
            .await
            .expect_err("unwired Gateway endpoint config must fail closed");
        assert!(error.to_string().contains("not policy-wired"));
        for key in ["base_url", "api_base"] {
            let mut settings_config = gateway_config.clone();
            settings_config.base_url = None;
            settings_config.settings.insert(
                key.to_string(),
                serde_json::json!("https://unwired.example.test"),
            );
            let error = create_provider(settings_config)
                .await
                .expect_err("unwired Gateway settings endpoint must fail closed");
            assert!(error.to_string().contains("not policy-wired"));
        }
    }
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener must bind");
    let config = ProviderConfig {
        name: "unwired-cloudflare".to_string(),
        provider_type: "cloudflare".to_string(),
        base_url: Some(format!(
            "http://localhost:{}",
            listener.local_addr().unwrap().port()
        )),
        ..Default::default()
    };
    let error = create_provider(config)
        .await
        .expect_err("unwired hostname endpoint must fail closed before construction");
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
        let is_local = url::Url::parse(definition.base_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .is_some_and(|host| host == "localhost");
        let config = ProviderConfig {
            name: (*name).to_string(),
            provider_type: (*name).to_string(),
            api_key: if definition.skip_api_key {
                String::new()
            } else {
                "test-key".to_string()
            },
            endpoint_access: if is_local {
                ProviderEndpointAccess::PrivateNetwork
            } else {
                ProviderEndpointAccess::PublicOnly
            },
            ..Default::default()
        };
        if is_local {
            let mut public_config = config.clone();
            public_config.endpoint_access = ProviderEndpointAccess::PublicOnly;
            assert!(create_provider(public_config).await.is_err());
        } else {
            let mut private_config = config.clone();
            private_config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
            assert!(crate::config::Validate::validate(&private_config).is_err());
        }
        assert!(crate::config::Validate::validate(&config).is_ok());
        let provider = create_provider(config)
            .await
            .unwrap_or_else(|error| panic!("Catalog provider '{name}' should work: {error}"));
        assert_eq!(provider.capabilities(), definition.capabilities);
    }
}
