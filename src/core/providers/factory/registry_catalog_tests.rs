use crate::core::providers::{Provider, ProviderType, registry as provider_registry};

#[tokio::test]
async fn issue_606_catalogified_candidates_use_catalog_runtime_path() {
    for provider_type in [
        ProviderType::MetaLlama,
        ProviderType::V0,
        ProviderType::AmazonNova,
        ProviderType::Custom("together".to_string()),
    ] {
        let expected_capabilities =
            provider_registry::catalog_definition_for_provider_type(&provider_type)
                .expect("catalogified provider should have a definition")
                .capabilities;
        let provider = Provider::from_config_async(
            provider_type.clone(),
            serde_json::json!({
                "api_key": "sk-test-key",
                "headers": {"x-test-header": "test-value"},
                "custom_headers": {"x-custom-header": "custom-value"},
                "timeout": 42,
                "max_retries": 4
            }),
        )
        .await
        .unwrap_or_else(|err| panic!("{provider_type:?} should be creatable: {err}"));

        assert!(matches!(provider, Provider::OpenAILike(_)));
        assert_eq!(provider.name(), provider_type.to_string());
        assert_eq!(provider.capabilities(), expected_capabilities);
    }
}

#[tokio::test]
async fn retired_github_models_selectors_fail_before_transport() {
    use crate::config::models::provider::ProviderConfig;
    use crate::core::providers::{ProviderError, create_provider};
    for selector in ["github", "github-models"] {
        assert!(!provider_registry::is_tier1_provider(selector));
        assert!(provider_registry::get_definition(selector).is_none());
        let result = create_provider(ProviderConfig {
            name: selector.into(),
            api_key: "test-token".into(),
            ..Default::default()
        })
        .await;
        assert!(
            matches!(result, Err(ProviderError::NotImplemented { .. })),
            "{selector}"
        );
    }
    assert!(provider_registry::catalog_policy::catalog_model_infos("github").is_none());
    assert!(provider_registry::catalog_policy::catalog_model_info("github", "gpt-4o").is_none());
    assert_ne!(
        provider_registry::canonical_selector("github_copilot"),
        "github"
    );
}
