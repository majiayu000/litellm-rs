//! Unit tests for Hyperbolic provider

#[cfg(test)]
use super::*;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

#[tokio::test]
async fn test_provider_creation() {
    let config = HyperbolicConfig::from_env().with_api_key("test-key");

    let provider = HyperbolicProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_with_api_key() {
    let provider = HyperbolicProvider::with_api_key("test-key").await;
    assert!(provider.is_ok());
}

#[test]
fn test_provider_name() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = HyperbolicProvider::with_api_key("test-key").await.unwrap();
        assert_eq!(provider.name(), "hyperbolic");
    });
}

#[test]
fn test_provider_capabilities() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = HyperbolicProvider::with_api_key("test-key").await.unwrap();
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ToolCalling));
    });
}

#[test]
fn test_model_info() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = HyperbolicProvider::with_api_key("test-key").await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());

        // Check if Llama models are present
        let llama_models: Vec<_> = models
            .iter()
            .filter(|m| m.id.to_lowercase().contains("llama"))
            .collect();
        assert!(!llama_models.is_empty());

        // Check if Qwen models are present
        let qwen_models: Vec<_> = models
            .iter()
            .filter(|m| m.id.to_lowercase().contains("qwen"))
            .collect();
        assert!(!qwen_models.is_empty());
    });
}

#[test]
fn test_model_pricing() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = HyperbolicProvider::with_api_key("test-key").await.unwrap();
        let models = provider.models();

        // All models should have pricing information
        for model in models {
            assert!(model.input_cost_per_1k_tokens.is_some());
            assert!(model.output_cost_per_1k_tokens.is_some());
            assert_eq!(model.currency, "USD");
        }
    });
}

#[test]
fn test_supported_openai_params() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = HyperbolicProvider::with_api_key("test-key").await.unwrap();

        let params = provider.get_supported_openai_params("meta-llama/Meta-Llama-3.1-70B-Instruct");
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"response_format"));
        assert!(params.contains(&"stream"));
    });
}

#[tokio::test]
async fn test_cost_calculation() {
    let provider = HyperbolicProvider::with_api_key("test-key").await.unwrap();

    // Test cost calculation for a known model
    let cost = provider
        .calculate_cost("meta-llama/Meta-Llama-3.1-70B-Instruct", 1000, 1000)
        .await;
    assert!(cost.is_ok());

    let total_cost = cost.unwrap();
    assert!(total_cost > 0.0);

    // Test cost calculation for unknown model
    let unknown_cost = provider.calculate_cost("unknown-model", 1000, 1000).await;
    assert!(unknown_cost.is_err());
}

// Note: Error mapping tests removed - HyperbolicError is now a type alias to ProviderError

#[test]
fn test_model_capabilities() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = HyperbolicProvider::with_api_key("test-key").await.unwrap();
        let models = provider.models();

        // Check tool-supporting models have proper capabilities
        let tool_models: Vec<_> = models.iter().filter(|m| m.supports_tools).collect();

        for model in tool_models {
            assert!(
                model
                    .capabilities
                    .contains(&ProviderCapability::ToolCalling)
            );
        }
    });
}

#[test]
fn test_config_api_base() {
    let config = HyperbolicConfig::default();
    assert_eq!(config.get_api_base(), "https://api.hyperbolic.xyz/v1");

    let custom_config = HyperbolicConfig::from_env().with_base_url("https://custom.hyperbolic.xyz");
    assert_eq!(
        custom_config.get_api_base(),
        "https://custom.hyperbolic.xyz"
    );
}

#[test]
fn test_model_info_function() {
    let info = model_info::get_model_info("meta-llama/Meta-Llama-3.1-70B-Instruct");
    assert!(info.is_some());

    let info = info.unwrap();
    assert!(info.supports_tools);
    assert_eq!(info.max_context_length, 131072);
}

#[test]
fn test_available_models() {
    let models = model_info::get_available_models();
    assert!(!models.is_empty());
    assert!(models.contains(&"meta-llama/Meta-Llama-3.1-70B-Instruct"));
    assert!(models.contains(&"Qwen/Qwen2.5-72B-Instruct"));
}

#[test]
fn test_deepseek_models() {
    let info = model_info::get_model_info("deepseek-ai/DeepSeek-V2.5");
    assert!(info.is_some());
    let info = info.unwrap();
    assert!(info.supports_tools);

    let info = model_info::get_model_info("deepseek-ai/DeepSeek-R1");
    assert!(info.is_some());
    let info = info.unwrap();
    assert!(info.supports_tools);
}
