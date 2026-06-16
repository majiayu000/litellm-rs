//! E2E tests for DeepSeek provider
//!
//! These tests make real API calls and require DEEPSEEK_API_KEY.
//! Run with: DEEPSEEK_API_KEY=xxx cargo test --all-features -- --ignored deepseek

#[cfg(test)]
mod tests {
    use litellm_rs::core::providers::openai_like::OpenAILikeProvider;
    use litellm_rs::core::providers::registry;
    use litellm_rs::core::traits::provider::llm_provider::trait_definition::LLMProvider;
    use litellm_rs::core::types::context::RequestContext;
    use litellm_rs::core::types::{
        chat::ChatMessage, chat::ChatRequest, message::MessageContent, message::MessageRole,
    };

    /// Helper to create a simple chat request
    fn create_chat_request(model: &str, content: &str) -> ChatRequest {
        ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text(content.to_string())),
                ..Default::default()
            }],
            max_tokens: Some(100),
            temperature: Some(0.0),
            ..Default::default()
        }
    }

    /// E2E test for DeepSeek chat completion
    #[tokio::test]
    #[ignore]
    async fn test_deepseek_chat_completion() {
        let api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY not set");

        let def = registry::get_definition("deepseek").unwrap();
        let config = def.to_openai_like_config(Some(&api_key), None);
        let provider = OpenAILikeProvider::new(config).await.unwrap();

        let request = create_chat_request("deepseek-v4-flash", "Say hello");
        let context = RequestContext::default();
        let response = provider.chat_completion(request, context).await;

        assert!(response.is_ok(), "Failed: {:?}", response.err());
        let response = response.unwrap();
        assert!(!response.choices.is_empty());
    }
}
