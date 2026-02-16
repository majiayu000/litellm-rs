//! Unit tests for Watsonx provider

#[cfg(test)]
use super::*;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

fn create_test_config() -> WatsonxConfig {
    WatsonxConfig {
        api_key: Some("test-api-key".to_string()),
        api_base: Some("https://us-south.ml.cloud.ibm.com".to_string()),
        project_id: Some("test-project-id".to_string()),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_provider_creation() {
    let config = create_test_config();
    let provider = WatsonxProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_with_credentials() {
    let provider = WatsonxProvider::with_credentials(
        "test-key",
        "test-project",
        Some("https://us-south.ml.cloud.ibm.com".to_string()),
    )
    .await;
    assert!(provider.is_ok());
}

#[test]
fn test_provider_name() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let config = create_test_config();
        let provider = WatsonxProvider::new(config).await.unwrap();
        assert_eq!(provider.name(), "watsonx");
    });
}

#[test]
fn test_provider_capabilities() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let config = create_test_config();
        let provider = WatsonxProvider::new(config).await.unwrap();
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ToolCalling));
    });
}

#[test]
fn test_model_info() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let config = create_test_config();
        let provider = WatsonxProvider::new(config).await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());

        // Check if IBM Granite models are present
        let granite_models: Vec<_> = models.iter().filter(|m| m.id.contains("granite")).collect();
        assert!(!granite_models.is_empty());

        // Check if Llama models are present
        let llama_models: Vec<_> = models.iter().filter(|m| m.id.contains("llama")).collect();
        assert!(!llama_models.is_empty());
    });
}

#[test]
fn test_model_pricing() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let config = create_test_config();
        let provider = WatsonxProvider::new(config).await.unwrap();
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
        let config = create_test_config();
        let provider = WatsonxProvider::new(config).await.unwrap();

        // Test model with tools
        let params = provider.get_supported_openai_params("ibm/granite-13b-chat-v2");
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"response_format"));

        // Test model without tools
        let params_no_tools = provider.get_supported_openai_params("ibm/granite-3b-code-instruct");
        assert!(params_no_tools.contains(&"temperature"));
        assert!(params_no_tools.contains(&"max_tokens"));
        assert!(!params_no_tools.contains(&"tools"));
    });
}

#[tokio::test]
async fn test_cost_calculation() {
    let config = create_test_config();
    let provider = WatsonxProvider::new(config).await.unwrap();

    // Test cost calculation for a known model
    let cost = provider
        .calculate_cost("ibm/granite-13b-chat-v2", 1000, 1000)
        .await;
    assert!(cost.is_ok());

    let total_cost = cost.unwrap();
    assert!(total_cost > 0.0);

    // Test cost calculation for unknown model
    let unknown_cost = provider.calculate_cost("unknown-model", 1000, 1000).await;
    assert!(unknown_cost.is_err());
}

#[test]
fn test_error_mapping() {
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let mapper = error::WatsonxErrorMapper;

    // Test 401 error mapping
    let auth_error = mapper.map_http_error(401, "Unauthorized");
    assert!(matches!(auth_error, ProviderError::Authentication { .. }));

    // Test 429 error mapping
    let rate_error = mapper.map_http_error(429, "Too many requests");
    assert!(matches!(rate_error, ProviderError::RateLimit { .. }));

    // Test 404 error mapping
    let not_found = mapper.map_http_error(404, "Not found");
    assert!(matches!(not_found, ProviderError::ModelNotFound { .. }));
}

#[test]
fn test_error_retryability() {
    // Rate limit errors should be retryable
    let rate_error = ProviderError::rate_limit("watsonx", Some(60));
    assert!(rate_error.is_retryable());
    assert!(rate_error.retry_delay().is_some());

    // Rate limit without retry_after is still retryable but no delay
    let rate_error_no_delay = ProviderError::rate_limit("watsonx", None);
    assert!(rate_error_no_delay.is_retryable());

    // Service unavailable should be retryable
    let service_error = ProviderError::provider_unavailable("watsonx", "Service down");
    assert!(service_error.is_retryable());
    assert!(service_error.retry_delay().is_some());

    // Network errors should be retryable
    let network_error = ProviderError::network("watsonx", "Connection failed");
    assert!(network_error.is_retryable());
    assert!(network_error.retry_delay().is_some());

    // Authentication errors should not be retryable
    let auth_error = ProviderError::authentication("watsonx", "Bad key");
    assert!(!auth_error.is_retryable());
    assert!(auth_error.retry_delay().is_none());
}

#[test]
fn test_model_capabilities() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let config = create_test_config();
        let provider = WatsonxProvider::new(config).await.unwrap();
        let models = provider.models();

        // Check tool-supporting models have proper capabilities
        for model in models.iter().filter(|m| m.supports_tools) {
            assert!(
                model
                    .capabilities
                    .contains(&ProviderCapability::ToolCalling),
                "Model {} should have ToolCalling capability",
                model.id
            );
        }

        // Check vision models have multimodal support
        let vision_models: Vec<_> = models.iter().filter(|m| m.id.contains("vision")).collect();
        for model in vision_models {
            assert!(
                model.supports_multimodal,
                "Vision model {} should support multimodal",
                model.id
            );
        }
    });
}

#[test]
fn test_config_build_url() {
    let config = WatsonxConfig {
        api_base: Some("https://us-south.ml.cloud.ibm.com".to_string()),
        api_version: "2024-05-31".to_string(),
        ..Default::default()
    };

    // Test chat endpoint URL
    let url = config.build_url("/ml/v1/text/chat", false).unwrap();
    assert!(url.contains("https://us-south.ml.cloud.ibm.com"));
    assert!(url.contains("/ml/v1/text/chat"));
    assert!(url.contains("version=2024-05-31"));

    // Test stream endpoint URL
    let stream_url = config.build_url("/ml/v1/text/chat_stream", true).unwrap();
    assert!(stream_url.contains("/ml/v1/text/chat_stream"));
    assert!(stream_url.contains("version=2024-05-31"));
}

#[test]
fn test_model_info_accessors() {
    // Test get_model_info
    let model = model_info::get_model_info("ibm/granite-13b-chat-v2");
    assert!(model.is_some());

    // Test get_available_models
    let models = model_info::get_available_models();
    assert!(!models.is_empty());

    // Test get_models_by_provider
    let ibm_models = model_info::get_models_by_provider("ibm");
    assert!(!ibm_models.is_empty());

    let meta_models = model_info::get_models_by_provider("meta");
    assert!(!meta_models.is_empty());

    // Test supports_chat
    assert!(model_info::supports_chat("ibm/granite-13b-chat-v2"));

    // Test supports_tools
    assert!(model_info::supports_tools("ibm/granite-13b-chat-v2"));
    assert!(!model_info::supports_tools("ibm/granite-3b-code-instruct"));
}

#[tokio::test]
async fn test_map_openai_params() {
    let config = create_test_config();
    let provider = WatsonxProvider::new(config).await.unwrap();

    let mut params = std::collections::HashMap::new();
    params.insert("max_tokens".to_string(), serde_json::json!(100));
    params.insert("temperature".to_string(), serde_json::json!(0.7));
    params.insert("frequency_penalty".to_string(), serde_json::json!(0.5));
    params.insert("stop".to_string(), serde_json::json!(["END"]));
    params.insert("seed".to_string(), serde_json::json!(42));

    let mapped = provider
        .map_openai_params(params, "ibm/granite-13b-chat-v2")
        .await
        .unwrap();

    // Check parameter mapping
    assert!(mapped.contains_key("max_new_tokens"));
    assert!(mapped.contains_key("temperature"));
    assert!(mapped.contains_key("repetition_penalty"));
    assert!(mapped.contains_key("stop_sequences"));
    assert!(mapped.contains_key("random_seed"));
}

#[test]
fn test_config_validation() {
    use crate::core::traits::provider::ProviderConfig;

    // Valid config
    let valid_config = WatsonxConfig {
        api_key: Some("test-key".to_string()),
        project_id: Some("test-project".to_string()),
        ..Default::default()
    };
    assert!(valid_config.validate().is_ok());

    // Config with token instead of api_key
    let token_config = WatsonxConfig {
        token: Some("test-token".to_string()),
        project_id: Some("test-project".to_string()),
        ..Default::default()
    };
    assert!(token_config.validate().is_ok());

    // Config with zen_api_key
    let zen_config = WatsonxConfig {
        zen_api_key: Some("zen-key".to_string()),
        project_id: Some("test-project".to_string()),
        ..Default::default()
    };
    assert!(zen_config.validate().is_ok());

    // Config with space_id instead of project_id
    let space_config = WatsonxConfig {
        api_key: Some("test-key".to_string()),
        space_id: Some("test-space".to_string()),
        ..Default::default()
    };
    assert!(space_config.validate().is_ok());

    // Invalid timeout
    let invalid_timeout = WatsonxConfig {
        api_key: Some("test-key".to_string()),
        project_id: Some("test-project".to_string()),
        timeout: 0,
        ..Default::default()
    };
    assert!(invalid_timeout.validate().is_err());
}
