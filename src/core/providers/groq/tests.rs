//! Unit tests for Groq provider

#[cfg(test)]
use super::*;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

#[tokio::test]
async fn test_provider_creation() {
    let config = GroqConfig::from_env().with_api_key("test-key");

    let provider = GroqProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_with_api_key() {
    let provider = GroqProvider::with_api_key("test-key").await;
    assert!(provider.is_ok());
}

#[test]
fn test_provider_name() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();
        assert_eq!(provider.name(), "groq");
    });
}

#[test]
fn test_provider_capabilities() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ToolCalling));
    });
}

#[test]
fn test_model_info() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());

        // Check if Llama models are present
        let llama_models: Vec<_> = models.iter().filter(|m| m.id.contains("llama")).collect();
        assert!(!llama_models.is_empty());

        // Check if Mixtral models are present
        let mixtral_models: Vec<_> = models.iter().filter(|m| m.id.contains("mixtral")).collect();
        assert!(!mixtral_models.is_empty());
    });
}

#[test]
fn test_model_pricing() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();
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
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();

        // Test regular model params
        let params = provider.get_supported_openai_params("llama-3.1-70b-versatile");
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"response_format"));

        // Test reasoning model params (use model that actually supports reasoning in Python LiteLLM)
        let reasoning_params =
            provider.get_supported_openai_params("deepseek-r1-distill-llama-70b");
        assert!(reasoning_params.contains(&"reasoning_effort"));
    });
}

#[test]
fn test_should_fake_stream() {
    use crate::core::types::{chat::ChatMessage, chat::ChatRequest, message::MessageRole};

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();

        // Test without response_format
        let request = ChatRequest {
            model: "llama-3.1-70b-versatile".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(crate::core::types::message::MessageContent::Text(
                    "Hello".to_string(),
                )),
                ..Default::default()
            }],
            stream: true,
            ..Default::default()
        };
        assert!(!provider.should_fake_stream(&request));

        // Test with response_format and stream
        let mut request_with_format = request.clone();
        request_with_format.response_format = Some(crate::core::types::tools::ResponseFormat {
            format_type: "json_object".to_string(),
            json_schema: None,
            response_type: None,
        });
        assert!(provider.should_fake_stream(&request_with_format));

        // Test with response_format but no stream
        request_with_format.stream = false;
        assert!(!provider.should_fake_stream(&request_with_format));
    });
}

#[tokio::test]
async fn test_cost_calculation() {
    let provider = GroqProvider::with_api_key("test-key").await.unwrap();

    // Test cost calculation for a known model
    let cost = provider
        .calculate_cost("llama-3.1-70b-versatile", 1000, 1000)
        .await;
    assert!(cost.is_ok());

    let total_cost = cost.unwrap();
    assert!(total_cost > 0.0);

    // Test cost calculation for unknown model
    let unknown_cost = provider.calculate_cost("unknown-model", 1000, 1000).await;
    assert!(unknown_cost.is_err());
}

// Note: Error mapping tests removed - GroqError is now a type alias to ProviderError

#[test]
fn test_model_capabilities() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();
        let models = provider.models();

        // Check tool-use models have proper capabilities
        let tool_models: Vec<_> = models
            .iter()
            .filter(|m| m.id.contains("tool-use"))
            .collect();

        for model in tool_models {
            assert!(model.supports_tools);
            assert!(
                model
                    .capabilities
                    .contains(&ProviderCapability::ToolCalling)
            );
        }

        // Check vision models have multimodal support
        let vision_models: Vec<_> = models.iter().filter(|m| m.id.contains("vision")).collect();

        for model in vision_models {
            assert!(model.supports_multimodal);
        }
    });
}
