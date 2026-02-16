//! Provider integration tests
//!
//! Tests provider creation, configuration, and basic functionality.
//! E2E tests that require real API keys are marked with #[ignore].

#[cfg(test)]
mod tests {
    use litellm_rs::core::providers::groq::{GroqConfig, GroqProvider};
    use litellm_rs::core::traits::provider::llm_provider::trait_definition::LLMProvider;
    use litellm_rs::core::types::model::ProviderCapability;

    /// Test Groq provider creation
    #[tokio::test]
    async fn test_groq_provider_creation() {
        let config = GroqConfig::from_env().with_api_key("test-key");

        let provider = GroqProvider::new(config).await;
        assert!(provider.is_ok());
    }

    /// Test Groq provider with API key
    #[tokio::test]
    async fn test_groq_provider_with_api_key() {
        let provider = GroqProvider::with_api_key("test-key").await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "groq");
    }

    /// Test Groq provider capabilities
    #[tokio::test]
    async fn test_groq_provider_capabilities() {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
    }

    /// Test Groq provider models
    #[tokio::test]
    async fn test_groq_provider_models() {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());

        // Check for Llama models
        let llama_models: Vec<_> = models.iter().filter(|m| m.id.contains("llama")).collect();
        assert!(!llama_models.is_empty(), "Should have Llama models");
    }

    /// Test Groq provider model pricing
    #[tokio::test]
    async fn test_groq_provider_model_pricing() {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();
        let models = provider.models();

        for model in models {
            assert!(model.input_cost_per_1k_tokens.is_some());
            assert!(model.output_cost_per_1k_tokens.is_some());
            assert_eq!(model.currency, "USD");
        }
    }

    /// Test Groq provider cost calculation
    #[tokio::test]
    async fn test_groq_provider_cost_calculation() {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();

        // Test cost calculation for a known model
        let cost = provider
            .calculate_cost("llama-3.1-70b-versatile", 1000, 1000)
            .await;
        assert!(cost.is_ok());
        assert!(cost.unwrap() > 0.0);

        // Unknown model should fail
        let cost = provider.calculate_cost("unknown-model", 1000, 1000).await;
        assert!(cost.is_err());
    }

    /// Test Groq provider supported params
    #[tokio::test]
    async fn test_groq_provider_supported_params() {
        let provider = GroqProvider::with_api_key("test-key").await.unwrap();

        let params = provider.get_supported_openai_params("llama-3.1-70b-versatile");
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
    }

    // E2E tests that require real API keys
    // Run with: cargo test --all-features -- --ignored

    /// E2E test for Groq chat completion (requires GROQ_API_KEY)
    /// Note: This test is disabled by default. Run with --ignored flag.
    #[tokio::test]
    #[ignore]
    async fn test_groq_real_chat_completion() {
        use litellm_rs::core::types::context::RequestContext;
        use litellm_rs::core::types::{
            chat::ChatMessage, chat::ChatRequest, message::MessageContent, message::MessageRole,
        };

        let api_key =
            std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY environment variable not set");

        let provider = GroqProvider::with_api_key(&api_key).await.unwrap();

        let request = ChatRequest {
            model: "llama-3.1-8b-instant".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text(
                    "Say 'Hello' and nothing else".to_string(),
                )),
                ..Default::default()
            }],
            max_tokens: Some(10),
            ..Default::default()
        };

        let context = RequestContext::default();
        let response = provider.chat_completion(request, context).await;
        assert!(
            response.is_ok(),
            "Chat completion failed: {:?}",
            response.err()
        );

        let response = response.unwrap();
        assert!(!response.choices.is_empty());
        assert!(response.choices[0].message.content.is_some());
    }
}
