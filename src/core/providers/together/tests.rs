//! Unit tests for Together AI provider

#[cfg(test)]
use super::*;
use crate::core::traits::provider::ProviderConfig;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

#[tokio::test]
async fn test_provider_creation() {
    let config = TogetherConfig::from_env().with_api_key("test-key");

    let provider = TogetherProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_with_api_key() {
    let provider = TogetherProvider::with_api_key("test-key").await;
    assert!(provider.is_ok());
}

#[test]
fn test_provider_name() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = TogetherProvider::with_api_key("test-key").await.unwrap();
        assert_eq!(provider.name(), "together");
    });
}

#[test]
fn test_provider_capabilities() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = TogetherProvider::with_api_key("test-key").await.unwrap();
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ToolCalling));
        assert!(capabilities.contains(&ProviderCapability::Embeddings));
    });
}

#[test]
fn test_model_info() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = TogetherProvider::with_api_key("test-key").await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());

        // Check if Llama models are present
        let llama_models: Vec<_> = models
            .iter()
            .filter(|m| m.id.to_lowercase().contains("llama"))
            .collect();
        assert!(!llama_models.is_empty());

        // Check if DeepSeek models are present
        let deepseek_models: Vec<_> = models
            .iter()
            .filter(|m| m.id.to_lowercase().contains("deepseek"))
            .collect();
        assert!(!deepseek_models.is_empty());
    });
}

#[test]
fn test_model_pricing() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = TogetherProvider::with_api_key("test-key").await.unwrap();
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
        let provider = TogetherProvider::with_api_key("test-key").await.unwrap();

        // Test model with function calling support
        let params =
            provider.get_supported_openai_params("meta-llama/Llama-3.3-70B-Instruct-Turbo");
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"response_format"));

        // Test vision model (no function calling)
        let vision_params =
            provider.get_supported_openai_params("meta-llama/Llama-3.2-90B-Vision-Instruct-Turbo");
        assert!(vision_params.contains(&"temperature"));
        assert!(!vision_params.contains(&"tools"));
        assert!(!vision_params.contains(&"response_format"));
    });
}

#[test]
fn test_should_handle_response_format() {
    use crate::core::types::{
        chat::ChatMessage, chat::ChatRequest, message::MessageRole, tools::ResponseFormat,
    };

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = TogetherProvider::with_api_key("test-key").await.unwrap();

        // Test model with function calling - should not need special handling
        let request_with_fc = ChatRequest {
            model: "meta-llama/Llama-3.3-70B-Instruct-Turbo".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(crate::core::types::message::MessageContent::Text(
                    "Hello".to_string(),
                )),
                ..Default::default()
            }],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
                response_type: None,
            }),
            ..Default::default()
        };
        assert!(!provider.should_handle_response_format(&request_with_fc));

        // Test vision model - should need special handling
        let request_vision = ChatRequest {
            model: "meta-llama/Llama-3.2-90B-Vision-Instruct-Turbo".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(crate::core::types::message::MessageContent::Text(
                    "Hello".to_string(),
                )),
                ..Default::default()
            }],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
                response_type: None,
            }),
            ..Default::default()
        };
        assert!(provider.should_handle_response_format(&request_vision));

        // Test without response_format
        let request_no_format = ChatRequest {
            model: "meta-llama/Llama-3.2-90B-Vision-Instruct-Turbo".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(crate::core::types::message::MessageContent::Text(
                    "Hello".to_string(),
                )),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(!provider.should_handle_response_format(&request_no_format));
    });
}

#[tokio::test]
async fn test_cost_calculation() {
    let provider = TogetherProvider::with_api_key("test-key").await.unwrap();

    // Test cost calculation for a known model
    let cost = provider
        .calculate_cost("meta-llama/Llama-3.3-70B-Instruct-Turbo", 1000, 1000)
        .await;
    assert!(cost.is_ok());

    let total_cost = cost.unwrap();
    assert!(total_cost > 0.0);

    // Test cost calculation for unknown model
    let unknown_cost = provider.calculate_cost("unknown-model", 1000, 1000).await;
    assert!(unknown_cost.is_err());
}

// Note: Error mapping tests removed - TogetherError is now a type alias to ProviderError

#[test]
fn test_model_capabilities() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = TogetherProvider::with_api_key("test-key").await.unwrap();
        let models = provider.models();

        // Check tool-capable models have proper capabilities
        let tool_models: Vec<_> = models.iter().filter(|m| m.supports_tools).collect();

        for model in tool_models {
            assert!(
                model
                    .capabilities
                    .contains(&ProviderCapability::ToolCalling)
            );
        }

        // Check vision models have multimodal support
        let vision_models: Vec<_> = models
            .iter()
            .filter(|m| m.id.to_lowercase().contains("vision"))
            .collect();

        for model in vision_models {
            assert!(model.supports_multimodal);
        }
    });
}

#[test]
fn test_model_info_functions() {
    use super::model_info::*;

    // Test get_model_info
    let info = get_model_info("meta-llama/Llama-3.3-70B-Instruct-Turbo");
    assert!(info.is_some());
    let info = info.unwrap();
    assert!(info.supports_tools);
    assert!(!info.is_embedding);

    // Test is_function_calling_model
    assert!(is_function_calling_model(
        "meta-llama/Llama-3.3-70B-Instruct-Turbo"
    ));
    assert!(!is_function_calling_model(
        "meta-llama/Llama-3.2-90B-Vision-Instruct-Turbo"
    ));

    // Test get_embedding_models
    let embedding_models = get_embedding_models();
    assert!(!embedding_models.is_empty());
    for model in embedding_models {
        assert!(is_embedding_model(model));
    }

    // Test get_rerank_models
    let rerank_models = get_rerank_models();
    assert!(!rerank_models.is_empty());
    for model in rerank_models {
        assert!(is_rerank_model(model));
    }
}

#[test]
fn test_rerank_request_creation() {
    let request = RerankRequest::new(
        "Salesforce/Llama-Rank-V1",
        "What is machine learning?",
        vec![
            "Machine learning is a subset of AI".to_string(),
            "Deep learning uses neural networks".to_string(),
        ],
    );

    assert_eq!(request.model, "Salesforce/Llama-Rank-V1");
    assert_eq!(request.query, "What is machine learning?");
    assert_eq!(request.documents.len(), 2);
}

#[test]
fn test_rerank_request_with_options() {
    let request = RerankRequest::new(
        "Salesforce/Llama-Rank-V1",
        "test query",
        vec!["doc1".to_string()],
    )
    .with_top_n(5)
    .with_return_documents(false);

    assert_eq!(request.top_n, Some(5));
    assert_eq!(request.return_documents, Some(false));
}

#[test]
fn test_config_defaults() {
    let config = TogetherConfig::default();
    assert!(config.base.api_key.is_none());
    assert_eq!(config.base.timeout, 60);
    assert_eq!(config.base.max_retries, 3);
    assert!(!config.debug);
}

#[test]
fn test_config_get_api_base() {
    let config = TogetherConfig::from_env();
    assert_eq!(config.get_api_base(), "https://api.together.xyz/v1");

    let custom_config = TogetherConfig::from_env().with_base_url("https://custom.together.xyz");
    assert_eq!(custom_config.get_api_base(), "https://custom.together.xyz");
}

#[test]
fn test_config_validation() {
    let valid_config = TogetherConfig::from_env().with_api_key("test-key");
    assert!(valid_config.validate().is_ok());

    let mut invalid_timeout_config = TogetherConfig::from_env().with_api_key("test-key");
    invalid_timeout_config.base.timeout = 0;
    assert!(invalid_timeout_config.validate().is_err());
}

#[test]
fn test_together_model_enum() {
    let model = TogetherModel::Llama3_3_70B_Instruct_Turbo;
    assert_eq!(format!("{:?}", model), "Llama3_3_70B_Instruct_Turbo");

    let deepseek = TogetherModel::DeepSeekV3;
    assert_eq!(format!("{:?}", deepseek), "DeepSeekV3");
}

#[test]
fn test_pricing_category() {
    use super::model_info::get_pricing_category;

    assert_eq!(
        get_pricing_category("model-3b"),
        Some("together-ai-up-to-4b")
    );
    assert_eq!(
        get_pricing_category("model-7b"),
        Some("together-ai-4.1b-8b")
    );
    assert_eq!(
        get_pricing_category("model-70b"),
        Some("together-ai-41.1b-80b")
    );
}
