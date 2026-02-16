//! Unit tests for Novita provider

#[cfg(test)]
use super::*;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

#[tokio::test]
async fn test_provider_creation() {
    let config = NovitaConfig::from_env().with_api_key("test-key");

    let provider = NovitaProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_with_api_key() {
    let provider = NovitaProvider::with_api_key("test-key").await;
    assert!(provider.is_ok());
}

#[test]
fn test_provider_name() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = NovitaProvider::with_api_key("test-key").await.unwrap();
        assert_eq!(provider.name(), "novita");
    });
}

#[test]
fn test_provider_capabilities() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = NovitaProvider::with_api_key("test-key").await.unwrap();
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ToolCalling));
    });
}

#[test]
fn test_model_info() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = NovitaProvider::with_api_key("test-key").await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());

        // Check if Llama models are present
        let llama_models: Vec<_> = models.iter().filter(|m| m.id.contains("llama")).collect();
        assert!(!llama_models.is_empty());

        // Check if Mistral models are present
        let mistral_models: Vec<_> = models.iter().filter(|m| m.id.contains("mistral")).collect();
        assert!(!mistral_models.is_empty());
    });
}

#[test]
fn test_model_pricing() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = NovitaProvider::with_api_key("test-key").await.unwrap();
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
        let provider = NovitaProvider::with_api_key("test-key").await.unwrap();

        let params = provider.get_supported_openai_params("meta-llama/llama-3.1-70b-instruct");
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"response_format"));
    });
}

#[tokio::test]
async fn test_cost_calculation() {
    let provider = NovitaProvider::with_api_key("test-key").await.unwrap();

    // Test cost calculation for a known model
    let cost = provider
        .calculate_cost("meta-llama/llama-3.1-70b-instruct", 1000, 1000)
        .await;
    assert!(cost.is_ok());

    let total_cost = cost.unwrap();
    assert!(total_cost > 0.0);

    // Test cost calculation for unknown model
    let unknown_cost = provider.calculate_cost("unknown-model", 1000, 1000).await;
    assert!(unknown_cost.is_err());
}

#[test]
fn test_error_types() {
    // Test ProviderError factory methods
    let auth_error = ProviderError::authentication("novita", "test");
    assert_eq!(auth_error.http_status(), 401);

    let rate_error = ProviderError::rate_limit("novita", None);
    assert_eq!(rate_error.http_status(), 429);
    assert!(rate_error.is_retryable());

    let model_error = ProviderError::model_not_found("novita", "gpt-5");
    assert_eq!(model_error.http_status(), 404);
}

#[test]
fn test_error_retryability() {
    // Rate limit errors should be retryable
    let rate_error = ProviderError::rate_limit("novita", Some(60));
    assert!(rate_error.is_retryable());
    assert!(rate_error.retry_delay().is_some());

    // Service unavailable should be retryable
    let service_error = ProviderError::provider_unavailable("novita", "Service down");
    assert!(service_error.is_retryable());
    assert!(service_error.retry_delay().is_some());

    // Authentication errors should not be retryable
    let auth_error = ProviderError::authentication("novita", "Bad key");
    assert!(!auth_error.is_retryable());
    assert!(auth_error.retry_delay().is_none());
}

#[test]
fn test_model_capabilities() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = NovitaProvider::with_api_key("test-key").await.unwrap();
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
    let config = NovitaConfig::from_env();
    assert_eq!(config.get_api_base(), "https://api.novita.ai/v3/openai");

    let custom_config = NovitaConfig::from_env().with_base_url("https://custom.novita.ai");
    assert_eq!(custom_config.get_api_base(), "https://custom.novita.ai");
}

#[test]
fn test_model_info_function() {
    let info = model_info::get_model_info("meta-llama/llama-3.1-70b-instruct");
    assert!(info.is_some());

    let info = info.unwrap();
    assert!(info.supports_tools);
    assert_eq!(info.max_context_length, 131072);
}

#[test]
fn test_available_models() {
    let models = model_info::get_available_models();
    assert!(!models.is_empty());
    assert!(models.contains(&"meta-llama/llama-3.1-70b-instruct"));
    assert!(models.contains(&"mistralai/mixtral-8x7b-instruct"));
}
