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
        let error = create_provider(gateway_config.clone())
            .await
            .expect_err("must reject");
        assert!(error.to_string().contains("not policy-wired"));
        for key in ["base_url", "api_base"] {
            let mut settings_config = gateway_config.clone();
            settings_config.base_url = None;
            let settings = &mut settings_config.settings;
            settings.insert(key.into(), "https://x.test".into());
            let error = create_provider(settings_config)
                .await
                .expect_err("must reject");
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
    let error = create_provider(config)
        .await
        .expect_err("must reject before construction");
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
    let error = create_provider(gateway.clone())
        .await
        .expect_err("must reject");
    assert!(error.to_string().contains("must be a string"), "{error}");
    gateway.settings.insert("api_base".into(), " ".into());
    assert!(crate::config::Validate::validate(&gateway).is_err());
    assert!(create_provider(gateway.clone()).await.is_err());
    gateway.settings.clear();
    gateway.endpoint_access = PrivateNetwork;
    let error = create_provider(gateway).await.expect_err("must reject");
    assert!(error.to_string().contains("requires a base URL"), "{error}");
    for config in [
        serde_json::json!({"api_key": "sk-test", "api_base": 42}),
        serde_json::json!({"api_key": "sk-test", "api_base": " "}),
        serde_json::json!({"api_key": "sk-test", "endpoint_access": "private_network"}),
    ] {
        let error = Provider::from_config_async(ProviderType::OpenAI, config)
            .await
            .expect_err("must reject");
        assert!(error.to_string().contains("endpoint"), "{error}");
    }
}

#[tokio::test]
async fn azure_endpoint_aliases_reach_gateway_and_direct_factories() {
    for (provider_type, selector, key, endpoint) in [
        (
            ProviderType::Azure,
            "azure",
            "endpoint",
            "http://127.0.0.1:18080/openai/deployments/test",
        ),
        (
            ProviderType::Azure,
            "azure",
            "azure_endpoint",
            "http://127.0.0.1:18080/openai/deployments/test",
        ),
        (
            ProviderType::AzureAI,
            "azure_ai",
            "endpoint",
            "http://127.0.0.1:18080/models",
        ),
        (
            ProviderType::AzureAI,
            "azure_ai",
            "azure_ai_endpoint",
            "http://127.0.0.1:18080/models",
        ),
    ] {
        let direct = serde_json::json!({
            "api_key": "test-key",
            key: endpoint,
            "endpoint_access": "private_network"
        });
        Provider::from_config_async(provider_type, direct)
            .await
            .unwrap_or_else(|error| panic!("direct {selector}.{key} failed: {error}"));

        let mut gateway = ProviderConfig {
            name: format!("{selector}-test"),
            provider_type: selector.to_string(),
            api_key: "test-key".to_string(),
            endpoint_access: PrivateNetwork,
            ..Default::default()
        };
        gateway.settings.insert(key.to_string(), endpoint.into());
        create_provider(gateway)
            .await
            .unwrap_or_else(|error| panic!("gateway {selector}.{key} failed: {error}"));
    }
}

#[cfg(feature = "providers-extra")]
#[tokio::test]
async fn vertex_endpoint_alias_reaches_gateway_and_direct_factories() {
    let endpoint = "http://127.0.0.1:18080/v1/projects/test";
    let direct = serde_json::json!({
        "project_id": "test-project",
        "access_token": "test-token",
        "endpoint": endpoint,
        "endpoint_access": "private_network"
    });
    Provider::from_config_async(ProviderType::VertexAI, direct)
        .await
        .unwrap_or_else(|error| panic!("direct vertex_ai.endpoint failed: {error}"));

    let mut gateway = ProviderConfig {
        name: "vertex-test".to_string(),
        provider_type: "vertex_ai".to_string(),
        project: Some("test-project".to_string()),
        endpoint_access: PrivateNetwork,
        ..Default::default()
    };
    gateway
        .settings
        .insert("endpoint".to_string(), endpoint.into());
    gateway
        .settings
        .insert("access_token".to_string(), "test-token".into());
    create_provider(gateway)
        .await
        .unwrap_or_else(|error| panic!("gateway vertex_ai.endpoint failed: {error}"));

    for invalid in [serde_json::json!(42), serde_json::json!(" ")] {
        let direct = serde_json::json!({
            "project_id": "test-project",
            "access_token": "test-token",
            "endpoint": invalid.clone(),
            "endpoint_access": "private_network"
        });
        let error = Provider::from_config_async(ProviderType::VertexAI, direct)
            .await
            .expect_err("invalid direct Vertex endpoint alias must fail");
        assert!(
            error.to_string().contains("endpoint must be a string"),
            "{error}"
        );

        let mut gateway = ProviderConfig {
            name: "vertex-invalid".to_string(),
            provider_type: "vertex_ai".to_string(),
            project: Some("test-project".to_string()),
            endpoint_access: PrivateNetwork,
            ..Default::default()
        };
        gateway.settings.insert("endpoint".to_string(), invalid);
        gateway
            .settings
            .insert("access_token".to_string(), "test-token".into());
        let error = create_provider(gateway)
            .await
            .expect_err("invalid gateway Vertex endpoint alias must fail");
        assert!(
            error.to_string().contains("endpoint must be a string"),
            "{error}"
        );
    }

    let unrelated = serde_json::json!({
        "api_key": "sk-test",
        "endpoint": endpoint,
        "endpoint_access": "private_network"
    });
    let error = Provider::from_config_async(ProviderType::OpenAI, unrelated)
        .await
        .expect_err("Vertex endpoint alias must stay provider-specific");
    assert!(error.to_string().contains("requires a base URL"), "{error}");

    let mut unrelated = ProviderConfig {
        name: "openai-test".to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        endpoint_access: PrivateNetwork,
        ..Default::default()
    };
    unrelated
        .settings
        .insert("endpoint".to_string(), endpoint.into());
    let error = create_provider(unrelated)
        .await
        .expect_err("Vertex gateway endpoint alias must stay provider-specific");
    assert!(error.to_string().contains("requires a base URL"), "{error}");
}

#[tokio::test]
async fn official_openai_authority_rejects_private_factory_access() {
    for endpoint in ["https://api.openai.com/v1", "https://api.openai.com./v1"] {
        for provider_type in [ProviderType::OpenAI, ProviderType::OpenAICompatible] {
            for endpoint_key in ["base_url", "api_base"] {
                let mut direct_config = serde_json::json!({
                    "api_key": "sk-test",
                    "endpoint_access": "private_network"
                });
                direct_config[endpoint_key] = endpoint.into();
                let direct = Provider::from_config_async(provider_type.clone(), direct_config)
                    .await
                    .expect_err("direct official OpenAI endpoint must stay public");
                assert!(
                    direct.to_string().contains("official OpenAI"),
                    "{provider_type}.{endpoint_key}: {direct}"
                );
            }
        }

        for provider_type in ["openai", "openai_compatible"] {
            for endpoint_key in ["base_url", "api_base"] {
                let mut gateway = ProviderConfig {
                    name: "openai-test".to_string(),
                    provider_type: provider_type.to_string(),
                    api_key: "sk-test".to_string(),
                    endpoint_access: PrivateNetwork,
                    ..Default::default()
                };
                if endpoint_key == "base_url" {
                    gateway.base_url = Some(endpoint.to_string());
                } else {
                    gateway
                        .settings
                        .insert(endpoint_key.to_string(), endpoint.into());
                }
                let error = create_provider(gateway)
                    .await
                    .expect_err("gateway official OpenAI endpoint must stay public");
                assert!(
                    error.to_string().contains("official OpenAI"),
                    "{provider_type}.{endpoint_key}: {error}"
                );
            }
        }
    }
}

#[tokio::test]
async fn gateway_selector_normalization_preserves_endpoint_alias_policy() {
    let mut config = ProviderConfig {
        name: "azure-test".to_string(),
        provider_type: " Azure ".to_string(),
        api_key: "sk-test".to_string(),
        endpoint_access: PrivateNetwork,
        ..Default::default()
    };
    config.settings.insert(
        "azure_endpoint".to_string(),
        "http://127.0.0.1:18080/openai/deployments/test".into(),
    );

    create_provider(config)
        .await
        .expect("normalized Azure selector must retain its endpoint alias");
}
