use super::create_provider;
use crate::config::models::provider::ProviderConfig;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::{Provider, ProviderType};
use tokio::net::TcpListener;

#[tokio::test]
async fn gateway_and_direct_registry_activate_wired_endpoint_access() {
    let mut config = ProviderConfig {
        name: "test-openai-like".to_string(),
        provider_type: "openai_compatible".to_string(),
        base_url: Some("https://api.example.com/v1".to_string()),
        ..Default::default()
    };
    config
        .settings
        .insert("skip_api_key".to_string(), serde_json::json!(true));

    let provider = create_provider(config.clone())
        .await
        .unwrap_or_else(|error| panic!("public-only Gateway config should work: {error}"));
    let Provider::OpenAILike(provider) = provider else {
        panic!("expected OpenAILike provider");
    };
    assert_eq!(
        provider.config().base.endpoint_access,
        ProviderEndpointAccess::PublicOnly
    );

    config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config.base_url = Some("http://127.0.0.1:18080/v1".to_string());
    let provider = create_provider(config)
        .await
        .unwrap_or_else(|error| panic!("private Gateway access should activate: {error}"));
    let Provider::OpenAILike(provider) = provider else {
        panic!("expected OpenAILike provider");
    };
    assert_eq!(
        provider.config().base.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );

    for (explicit, base_url, expected) in [
        (
            "public_only",
            "https://api.example.test/v1",
            ProviderEndpointAccess::PublicOnly,
        ),
        (
            "private_network",
            "http://127.0.0.1:18081/v1",
            ProviderEndpointAccess::PrivateNetwork,
        ),
    ] {
        let provider = Provider::from_config_async(
            ProviderType::OpenAICompatible,
            serde_json::json!({
                "endpoint_access": explicit,
                "base_url": base_url,
                "skip_api_key": true
            }),
        )
        .await
        .unwrap_or_else(|error| panic!("wired direct config should activate: {error}"));
        let Provider::OpenAILike(provider) = provider else {
            panic!("expected OpenAILike provider");
        };
        assert_eq!(provider.config().base.endpoint_access, expected);
    }
}

#[tokio::test]
async fn endpoint_access_alias_and_unwired_direct_provider_fail_closed() {
    let mut settings_override = ProviderConfig {
        name: "openai".to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        ..Default::default()
    };
    settings_override.settings.insert(
        "endpoint_access".to_string(),
        serde_json::json!("private_network"),
    );
    let error = create_provider(settings_override)
        .await
        .err()
        .unwrap_or_else(|| panic!("settings must not override endpoint access"));
    assert!(error.to_string().contains("top-level"));

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
        let error = create_provider(gateway_config)
            .await
            .expect_err("unwired Gateway endpoint config must fail closed");
        assert!(error.to_string().contains("not policy-wired"));
    }
}

#[tokio::test]
async fn unwired_hostname_endpoint_is_rejected_without_connecting() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("loopback listener must bind");
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
            let error = create_provider(public_config)
                .await
                .expect_err("Local catalog providers must require explicit private access");
            assert!(
                error.to_string().contains("private or reserved"),
                "Local catalog provider '{name}' returned an unexpected error: {error}"
            );
        }

        let provider = create_provider(config)
            .await
            .unwrap_or_else(|error| panic!("Catalog provider '{name}' should work: {error}"));
        assert!(matches!(&provider, Provider::OpenAILike(_)));
        assert_eq!(provider.capabilities(), definition.capabilities);
    }
}
