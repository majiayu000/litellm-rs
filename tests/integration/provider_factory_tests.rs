//! Provider factory integration tests
//!
//! Tests for the Provider enum and factory functions that create providers
//! from configuration. These tests verify the unified provider interface works
//! correctly across different provider types.

#[cfg(test)]
mod tests {
    use litellm_rs::core::providers::{Provider, ProviderType, create_provider};
    use serde_json::json;

    /// Test creating OpenAI provider from config
    #[tokio::test]
    async fn test_openai_provider_from_config() {
        let config = json!({
            "api_key": "sk-proj-test-key-1234567890abcdef"  // Valid OpenAI format
        });

        let result = Provider::from_config_async(ProviderType::OpenAI, config).await;
        assert!(
            result.is_ok(),
            "Failed to create OpenAI provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert_eq!(provider.name(), "openai");
    }

    /// Test creating Anthropic provider from config
    #[tokio::test]
    async fn test_anthropic_provider_from_config() {
        let config = json!({
            "api_key": "sk-ant-test-key"
        });

        let result = Provider::from_config_async(ProviderType::Anthropic, config).await;
        assert!(
            result.is_ok(),
            "Failed to create Anthropic provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert_eq!(provider.name(), "anthropic");
    }

    /// Test creating Groq provider via create_provider (catalog path)
    #[tokio::test]
    async fn test_groq_provider_from_config() {
        use litellm_rs::core::providers::create_provider;

        let config = litellm_rs::config::models::provider::ProviderConfig {
            name: "groq".to_string(),
            provider_type: "groq".to_string(),
            api_key: "gsk-test-key".to_string(),
            ..Default::default()
        };

        let result = create_provider(config).await;
        assert!(
            result.is_ok(),
            "Failed to create Groq provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }

    /// Test creating XAI provider via create_provider (catalog path)
    #[tokio::test]
    async fn test_xai_provider_from_config() {
        use litellm_rs::core::providers::create_provider;

        let config = litellm_rs::config::models::provider::ProviderConfig {
            name: "xai".to_string(),
            provider_type: "xai".to_string(),
            api_key: "xai-test-key".to_string(),
            ..Default::default()
        };

        let result = create_provider(config).await;
        assert!(
            result.is_ok(),
            "Failed to create XAI provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }

    /// Test creating OpenRouter provider via create_provider (catalog path)
    #[tokio::test]
    async fn test_openrouter_provider_from_config() {
        let config = litellm_rs::config::models::provider::ProviderConfig {
            name: "openrouter".to_string(),
            provider_type: "openrouter".to_string(),
            api_key: "or-test-key".to_string(),
            ..Default::default()
        };

        let result = create_provider(config).await;
        assert!(
            result.is_ok(),
            "Failed to create OpenRouter provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }

    /// Test creating Mistral provider from config
    #[tokio::test]
    async fn test_mistral_provider_from_config() {
        let config = json!({
            "api_key": "mistral-test-key"
        });

        let result = Provider::from_config_async(ProviderType::Mistral, config).await;
        assert!(
            result.is_ok(),
            "Failed to create Mistral provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert_eq!(provider.name(), "mistral");
    }

    /// Test creating DeepSeek provider via create_provider (catalog path)
    #[tokio::test]
    async fn test_deepseek_provider_from_config() {
        use litellm_rs::core::providers::create_provider;

        let config = litellm_rs::config::models::provider::ProviderConfig {
            name: "deepseek".to_string(),
            provider_type: "deepseek".to_string(),
            api_key: "deepseek-test-key".to_string(),
            ..Default::default()
        };

        let result = create_provider(config).await;
        assert!(
            result.is_ok(),
            "Failed to create DeepSeek provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }

    /// Test creating Xiaomi MiMo provider via create_provider (catalog path)
    #[tokio::test]
    async fn test_xiaomi_mimo_provider_from_config() {
        let config = litellm_rs::config::models::provider::ProviderConfig {
            name: "xiaomi_mimo".to_string(),
            provider_type: "xiaomi_mimo".to_string(),
            api_key: "mimo-test-key".to_string(),
            ..Default::default()
        };

        let result = create_provider(config).await;
        assert!(
            result.is_ok(),
            "Failed to create Xiaomi MiMo provider: {:?}",
            result.err()
        );

        match result.unwrap() {
            Provider::OpenAILike(provider) => {
                assert_eq!(provider.config().provider_name, "xiaomi_mimo");
                assert_eq!(
                    provider.config().base.api_base.as_deref(),
                    Some("https://api.xiaomimimo.com/v1")
                );
            }
            _ => panic!("Expected Xiaomi MiMo to create OpenAILike provider"),
        }
    }

    /// Test creating Moonshot provider from catalog (Tier 1 → OpenAILike)
    #[tokio::test]
    async fn test_moonshot_provider_from_config() {
        let config = litellm_rs::config::models::provider::ProviderConfig {
            name: "moonshot".to_string(),
            provider_type: "".to_string(),
            api_key: "moonshot-test-key".to_string(),
            ..Default::default()
        };

        let result = create_provider(config).await;
        assert!(
            result.is_ok(),
            "Failed to create Moonshot provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }

    /// Test creating Cloudflare provider from config
    #[tokio::test]
    async fn test_cloudflare_provider_from_config() {
        let config = json!({
            "account_id": "test-account-id",
            "api_token": "test-api-token"
        });

        let result = Provider::from_config_async(ProviderType::Cloudflare, config).await;
        assert!(
            result.is_ok(),
            "Failed to create Cloudflare provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert_eq!(provider.name(), "cloudflare");
    }

    /// Test creating native Bedrock provider from AWS-specific config.
    #[tokio::test]
    async fn test_bedrock_provider_from_aws_config() {
        let config = json!({
            "aws_access_key_id": "AKIATEST123456789012",
            "aws_secret_access_key": "test-secret-key",
            "aws_region": "us-east-1"
        });

        let result = Provider::from_config_async(ProviderType::Bedrock, config).await;
        assert!(
            result.is_ok(),
            "Failed to create Bedrock provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert_eq!(provider.name(), "bedrock");
        assert!(matches!(provider, Provider::Bedrock(_)));
    }

    /// Test Bedrock Access Gateway stays on the OpenAI-compatible path.
    #[tokio::test]
    async fn test_bedrock_access_gateway_uses_openai_compatible_provider() {
        let config = json!({
            "api_key": "bedrock-access-gateway-key",
            "base_url": "https://bedrock-access-gateway.example.com/api/v1"
        });

        let result = Provider::from_config_async(ProviderType::OpenAICompatible, config).await;
        assert!(
            result.is_ok(),
            "Failed to create OpenAI-compatible Bedrock proxy provider: {:?}",
            result.err()
        );

        let provider = result.unwrap();
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }

    /// Test Azure OpenAI uses the native provider path when providers-extra is enabled.
    #[cfg(feature = "providers-extra")]
    #[tokio::test]
    async fn test_azure_provider_uses_native_dispatch() {
        let config = json!({
            "api_key": "azure-test-key",
            "azure_endpoint": "https://test-resource.openai.azure.com",
            "deployment_name": "gpt-4o-deployment",
            "api_version": "2024-02-01"
        });

        let result = Provider::from_config_async(ProviderType::Azure, config).await;
        assert!(
            result.is_ok(),
            "Failed to create native Azure provider: {:?}",
            result.err()
        );

        let provider = match result {
            Ok(provider) => provider,
            Err(err) => panic!("native Azure provider should be created: {err}"),
        };
        assert_eq!(provider.name(), "azure");
        assert_eq!(provider.provider_type(), ProviderType::Azure);
        assert!(matches!(provider, Provider::Azure(_)));
    }

    /// Test Azure AI uses the native provider path when providers-extra is enabled.
    #[cfg(feature = "providers-extra")]
    #[tokio::test]
    async fn test_azure_ai_provider_uses_native_dispatch() {
        let config = json!({
            "api_key": "azure-ai-test-key",
            "base_url": "https://test-resource.services.ai.azure.com",
            "api_version": "2024-05-01-preview"
        });

        let result = Provider::from_config_async(ProviderType::AzureAI, config).await;
        assert!(
            result.is_ok(),
            "Failed to create native Azure AI provider: {:?}",
            result.err()
        );

        let provider = match result {
            Ok(provider) => provider,
            Err(err) => panic!("native Azure AI provider should be created: {err}"),
        };
        assert_eq!(provider.name(), "azure_ai");
        assert_eq!(provider.provider_type(), ProviderType::AzureAI);
        assert!(matches!(provider, Provider::AzureAI(_)));
    }

    /// Test provider creation fails with missing api_key
    #[tokio::test]
    async fn test_provider_creation_fails_without_api_key() {
        let config = json!({});

        let result = Provider::from_config_async(ProviderType::OpenAI, config).await;
        assert!(result.is_err(), "Should fail without api_key");

        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("api_key") || err_str.contains("required"),
            "Error should mention api_key: {}",
            err_str
        );
    }

    /// Test Cloudflare provider fails without account_id
    #[tokio::test]
    async fn test_cloudflare_fails_without_account_id() {
        let config = json!({
            "api_token": "test-token"
        });

        let result = Provider::from_config_async(ProviderType::Cloudflare, config).await;
        assert!(result.is_err(), "Should fail without account_id");
    }

    /// Test provider capabilities are available
    #[tokio::test]
    async fn test_provider_capabilities() {
        let config = json!({
            "api_key": "sk-proj-test-key-1234567890abcdef"
        });

        let provider = Provider::from_config_async(ProviderType::OpenAI, config)
            .await
            .unwrap();

        let capabilities = provider.capabilities();
        assert!(
            !capabilities.is_empty(),
            "Provider should have capabilities"
        );
    }

    /// Test provider models list
    #[tokio::test]
    async fn test_provider_models_list() {
        let config = json!({
            "api_key": "sk-proj-test-key-1234567890abcdef"
        });

        let provider = Provider::from_config_async(ProviderType::OpenAI, config)
            .await
            .unwrap();

        let models = provider.list_models();
        assert!(!models.is_empty(), "Provider should have models");

        // All models should have required fields
        for model in models {
            assert!(!model.id.is_empty(), "Model should have id");
            assert!(!model.provider.is_empty(), "Model should have provider");
        }
    }

    /// Test provider type display
    #[test]
    fn test_provider_type_display() {
        assert_eq!(format!("{}", ProviderType::OpenAI), "openai");
        assert_eq!(format!("{}", ProviderType::Anthropic), "anthropic");
        assert_eq!(format!("{}", ProviderType::Groq), "groq");
        assert_eq!(format!("{}", ProviderType::XAI), "xai");
        assert_eq!(format!("{}", ProviderType::DeepSeek), "deepseek");
    }
}
