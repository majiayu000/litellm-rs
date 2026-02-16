//! Tests for Voyage AI Provider

use super::*;

#[cfg(test)]
mod provider_tests {
    use super::*;
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
    use crate::core::types::{
        chat::ChatMessage, chat::ChatRequest, embedding::EmbeddingInput,
        embedding::EmbeddingRequest, message::MessageContent, message::MessageRole,
    };
    use crate::core::types::{context::RequestContext, model::ProviderCapability};

    async fn create_test_provider() -> VoyageProvider {
        let config = VoyageConfig::from_env().with_api_key("test-key");
        VoyageProvider::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let config = VoyageConfig::from_env().with_api_key("test-key");
        let provider = VoyageProvider::new(config).await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_provider_with_api_key() {
        let provider = VoyageProvider::with_api_key("test-key").await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_provider_name() {
        let provider = create_test_provider().await;
        assert_eq!(provider.name(), "voyage");
    }

    #[tokio::test]
    async fn test_provider_capabilities() {
        let provider = create_test_provider().await;
        let capabilities = provider.capabilities();
        assert!(capabilities.contains(&ProviderCapability::Embeddings));
        assert!(!capabilities.contains(&ProviderCapability::ChatCompletion));
    }

    #[tokio::test]
    async fn test_provider_models() {
        let provider = create_test_provider().await;
        let models = provider.models();
        assert!(!models.is_empty());

        // Check that we have voyage-3 model
        assert!(models.iter().any(|m| m.id == "voyage-3"));
    }

    #[tokio::test]
    async fn test_supports_embeddings() {
        let provider = create_test_provider().await;
        assert!(provider.supports_embeddings());
    }

    #[tokio::test]
    async fn test_does_not_support_chat() {
        let provider = create_test_provider().await;
        assert!(!provider.supports_streaming());
        assert!(!provider.supports_tools());
    }

    #[tokio::test]
    async fn test_get_supported_openai_params() {
        let provider = create_test_provider().await;
        let params = provider.get_supported_openai_params("voyage-3");
        assert!(params.contains(&"encoding_format"));
        assert!(params.contains(&"dimensions"));
    }

    #[tokio::test]
    async fn test_map_openai_params() {
        let provider = create_test_provider().await;
        let mut params = std::collections::HashMap::new();
        params.insert("dimensions".to_string(), serde_json::json!(512));

        let mapped = provider
            .map_openai_params(params, "voyage-3")
            .await
            .unwrap();

        // dimensions should be mapped to output_dimension for voyage-3
        assert!(mapped.contains_key("output_dimension"));
        assert!(!mapped.contains_key("dimensions"));
    }

    #[tokio::test]
    async fn test_map_openai_params_non_v3() {
        let provider = create_test_provider().await;
        let mut params = std::collections::HashMap::new();
        params.insert("dimensions".to_string(), serde_json::json!(512));

        let mapped = provider
            .map_openai_params(params, "voyage-2")
            .await
            .unwrap();

        // dimensions should NOT be mapped for voyage-2 (doesn't support custom dimensions)
        assert!(!mapped.contains_key("output_dimension"));
    }

    #[tokio::test]
    async fn test_calculate_cost() {
        let provider = create_test_provider().await;
        let cost = provider
            .calculate_cost("voyage-3", 1000000, 0)
            .await
            .unwrap();

        // Voyage 3 costs $0.06 per million tokens
        assert!((cost - 0.06).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_calculate_cost_lite() {
        let provider = create_test_provider().await;
        let cost = provider
            .calculate_cost("voyage-3-lite", 1000000, 0)
            .await
            .unwrap();

        // Voyage 3 Lite costs $0.02 per million tokens
        assert!((cost - 0.02).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let provider = create_test_provider().await;
        let result = provider.calculate_cost("unknown-model", 1000, 0).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_chat_completion_returns_error() {
        let provider = create_test_provider().await;

        let request = ChatRequest {
            model: "voyage-3".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                ..Default::default()
            }],
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.chat_completion(request, context).await;

        assert!(result.is_err());
        // VoyageError is now a type alias to ProviderError
        if let Err(err) = result {
            assert!(matches!(
                err,
                crate::core::providers::unified_provider::ProviderError::NotSupported { .. }
            ));
        }
    }

    #[tokio::test]
    async fn test_transform_embedding_request() {
        let provider = create_test_provider().await;

        let request = EmbeddingRequest {
            model: "voyage-3".to_string(),
            input: EmbeddingInput::Text("Hello world".to_string()),
            encoding_format: Some("float".to_string()),
            dimensions: Some(512),
            user: None,
            task_type: Some("document".to_string()),
        };

        let result = provider.transform_embedding_request(&request);
        assert!(result.is_ok());

        let json = result.unwrap();
        assert_eq!(json["model"], "voyage-3");
        assert_eq!(json["encoding_format"], "float");
        assert_eq!(json["output_dimension"], 512);
        assert_eq!(json["input_type"], "document");
    }

    #[tokio::test]
    async fn test_transform_embedding_request_array() {
        let provider = create_test_provider().await;

        let request = EmbeddingRequest {
            model: "voyage-3".to_string(),
            input: EmbeddingInput::Array(vec!["Hello".to_string(), "World".to_string()]),
            encoding_format: None,
            dimensions: None,
            user: None,
            task_type: None,
        };

        let result = provider.transform_embedding_request(&request);
        assert!(result.is_ok());

        let json = result.unwrap();
        assert!(json["input"].as_array().is_some());
        assert_eq!(json["input"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_transform_embedding_response() {
        let provider = create_test_provider().await;

        let response = serde_json::json!({
            "object": "list",
            "model": "voyage-3",
            "data": [{
                "object": "embedding",
                "index": 0,
                "embedding": [0.1, 0.2, 0.3, 0.4, 0.5]
            }],
            "usage": {
                "total_tokens": 5
            }
        });

        let result = provider.transform_embedding_response(response);
        assert!(result.is_ok());

        let embedding_response = result.unwrap();
        assert_eq!(embedding_response.object, "list");
        assert_eq!(embedding_response.model, "voyage-3");
        assert_eq!(embedding_response.data.len(), 1);
        assert_eq!(embedding_response.data[0].embedding.len(), 5);

        let usage = embedding_response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 5);
        assert_eq!(usage.total_tokens, 5);
    }

    #[tokio::test]
    async fn test_transform_embedding_response_multiple() {
        let provider = create_test_provider().await;

        let response = serde_json::json!({
            "object": "list",
            "model": "voyage-3",
            "data": [
                {
                    "object": "embedding",
                    "index": 0,
                    "embedding": [0.1, 0.2, 0.3]
                },
                {
                    "object": "embedding",
                    "index": 1,
                    "embedding": [0.4, 0.5, 0.6]
                }
            ],
            "usage": {
                "total_tokens": 10
            }
        });

        let result = provider.transform_embedding_response(response);
        assert!(result.is_ok());

        let embedding_response = result.unwrap();
        assert_eq!(embedding_response.data.len(), 2);
        assert_eq!(embedding_response.data[0].index, 0);
        assert_eq!(embedding_response.data[1].index, 1);
    }
}

#[cfg(test)]
mod model_info_tests {
    use super::model_info::*;

    #[test]
    fn test_get_available_models() {
        let models = get_available_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"voyage-3"));
    }

    #[test]
    fn test_get_model_info() {
        let model = get_model_info("voyage-3").unwrap();
        assert_eq!(model.display_name, "Voyage 3");
        assert_eq!(model.embedding_dimensions, 1024);
    }

    #[test]
    fn test_get_default_model() {
        assert_eq!(get_default_model(), "voyage-3");
    }

    #[test]
    fn test_get_model_dimensions() {
        assert_eq!(get_model_dimensions("voyage-3"), Some(1024));
        assert_eq!(get_model_dimensions("voyage-3-lite"), Some(512));
        assert_eq!(get_model_dimensions("unknown"), None);
    }

    #[test]
    fn test_supports_custom_dimensions() {
        assert!(supports_custom_dimensions("voyage-3"));
        assert!(supports_custom_dimensions("voyage-3-lite"));
        assert!(!supports_custom_dimensions("voyage-2"));
    }
}

#[cfg(test)]
mod config_tests {
    use super::config::*;
    use crate::core::traits::provider::ProviderConfig;

    #[test]
    fn test_config_default() {
        let config = VoyageConfig::default();
        assert!(config.base.api_key.is_none());
        assert_eq!(config.base.timeout, 60);
        assert_eq!(config.base.max_retries, 3);
    }

    #[test]
    fn test_config_validation_with_key() {
        let config = VoyageConfig::from_env().with_api_key("test-key");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_missing_key() {
        let config = VoyageConfig::default();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_get_api_base_default() {
        let config = VoyageConfig::default();
        assert_eq!(config.get_api_base(), "https://api.voyageai.com/v1");
    }

    #[test]
    fn test_get_embeddings_url() {
        let config = VoyageConfig::default();
        assert_eq!(
            config.get_embeddings_url(),
            "https://api.voyageai.com/v1/embeddings"
        );
    }
}
