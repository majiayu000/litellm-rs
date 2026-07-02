use super::create_test_provider;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatMessage;
use crate::core::types::message::{MessageContent, MessageRole};

// ==================== Transform Request Tests ====================

#[tokio::test]
async fn test_transform_request_claude() {
    use crate::core::types::chat::ChatRequest;
    use crate::core::types::context::RequestContext;

    let provider = create_test_provider();

    let request = ChatRequest {
        model: "anthropic.claude-3-sonnet-20240229".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        max_tokens: Some(1000),
        temperature: Some(0.7),
        top_p: Some(0.9),
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.get("messages").is_some());
    assert_eq!(body.get("max_tokens").unwrap(), 1000);
    assert!(body.get("anthropic_version").is_some());
}

#[tokio::test]
async fn test_transform_request_titan() {
    use crate::core::types::chat::ChatRequest;
    use crate::core::types::context::RequestContext;

    let provider = create_test_provider();

    let request = ChatRequest {
        model: "amazon.titan-text-express-v1".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        max_tokens: Some(500),
        temperature: Some(0.5),
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.get("inputText").is_some());
    assert!(body.get("textGenerationConfig").is_some());
}

#[tokio::test]
async fn test_transform_request_nova() {
    use crate::core::types::chat::ChatRequest;
    use crate::core::types::context::RequestContext;

    let provider = create_test_provider();

    let request = ChatRequest {
        model: "amazon.nova-pro-v1:0".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        max_tokens: Some(2000),
        temperature: Some(0.8),
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.get("messages").is_some());
    assert_eq!(body.get("max_tokens").unwrap(), 2000);
}

#[tokio::test]
async fn test_transform_request_llama() {
    use crate::core::types::chat::ChatRequest;
    use crate::core::types::context::RequestContext;

    let provider = create_test_provider();

    let request = ChatRequest {
        model: "meta.llama3-70b-instruct-v1:0".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        max_tokens: Some(1500),
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_transform_request_mistral() {
    use crate::core::types::chat::ChatRequest;
    use crate::core::types::context::RequestContext;

    let provider = create_test_provider();

    let request = ChatRequest {
        model: "mistral.mistral-large-2407-v1:0".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.get("prompt").is_some());
}

#[tokio::test]
async fn test_transform_request_ai21() {
    use crate::core::types::chat::ChatRequest;
    use crate::core::types::context::RequestContext;

    let provider = create_test_provider();

    let request = ChatRequest {
        model: "ai21.jamba-1-5-large-v1:0".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.get("prompt").is_some());
    assert!(body.get("maxTokens").is_some());
}

#[tokio::test]
async fn test_transform_request_cohere() {
    use crate::core::types::chat::ChatRequest;
    use crate::core::types::context::RequestContext;

    let provider = create_test_provider();

    let request = ChatRequest {
        model: "cohere.command-r-plus-v1:0".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.get("prompt").is_some());
}

#[tokio::test]
async fn test_transform_request_embedding_model_error() {
    use crate::core::types::chat::ChatRequest;
    use crate::core::types::context::RequestContext;

    let provider = create_test_provider();

    let request = ChatRequest {
        model: "amazon.titan-embed-text-v1".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_transform_request_unknown_model() {
    use crate::core::types::chat::ChatRequest;
    use crate::core::types::context::RequestContext;

    let provider = create_test_provider();

    let request = ChatRequest {
        model: "unknown.model-v1".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_err());
}
