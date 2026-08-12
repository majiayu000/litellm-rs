use crate::core::providers::{Provider, ProviderType, registry as provider_registry};

#[tokio::test]
async fn issue_606_catalogified_candidates_use_catalog_runtime_path() {
    for provider_type in [
        ProviderType::MetaLlama,
        ProviderType::V0,
        ProviderType::AmazonNova,
        ProviderType::GitHub,
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
