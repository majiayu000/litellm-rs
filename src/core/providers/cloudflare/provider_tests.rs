use super::*;
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

fn create_test_config() -> CloudflareConfig {
    CloudflareConfig {
        account_id: Some("test_account".to_string()),
        api_token: Some("test_token".to_string()),
        ..Default::default()
    }
}

async fn create_test_provider() -> CloudflareProvider {
    CloudflareProvider::new(create_test_config()).await.unwrap()
}

// ==================== Provider Creation Tests ====================

#[tokio::test]
async fn test_provider_creation() {
    let config = CloudflareConfig {
        account_id: Some("test_account".to_string()),
        api_token: Some("test_token".to_string()),
        ..Default::default()
    };

    let provider = CloudflareProvider::new(config).await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(provider.name(), "cloudflare");
    assert!(!provider.models().is_empty());
}

#[tokio::test]
async fn test_provider_creation_with_custom_api_base() {
    let config = CloudflareConfig {
        account_id: Some("test_account".to_string()),
        api_token: Some("test_token".to_string()),
        api_base: Some("https://custom.cloudflare.com".to_string()),
        ..Default::default()
    };

    let provider = CloudflareProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_with_credentials_factory() {
    let provider = CloudflareProvider::with_credentials("account123", "token456").await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(provider.name(), "cloudflare");
}

#[tokio::test]
async fn test_provider_without_credentials() {
    let config = CloudflareConfig {
        account_id: None,
        api_token: None,
        ..Default::default()
    };

    let provider = CloudflareProvider::new(config).await;
    assert!(provider.is_err());
}

#[tokio::test]
async fn test_provider_without_account_id() {
    let config = CloudflareConfig {
        account_id: None,
        api_token: Some("test_token".to_string()),
        ..Default::default()
    };

    let provider = CloudflareProvider::new(config).await;
    assert!(provider.is_err());
}

#[tokio::test]
async fn test_provider_without_api_token() {
    let config = CloudflareConfig {
        account_id: Some("test_account".to_string()),
        api_token: None,
        ..Default::default()
    };

    let provider = CloudflareProvider::new(config).await;
    assert!(provider.is_err());
}

// ==================== Provider Capabilities Tests ====================

#[test]
fn test_capabilities() {
    assert!(CLOUDFLARE_CAPABILITIES.contains(&ProviderCapability::ChatCompletion));
    assert!(CLOUDFLARE_CAPABILITIES.contains(&ProviderCapability::ChatCompletionStream));
    assert_eq!(CLOUDFLARE_CAPABILITIES.len(), 2);
}

#[tokio::test]
async fn test_provider_name() {
    let provider = create_test_provider().await;
    assert_eq!(provider.name(), "cloudflare");
}

#[tokio::test]
async fn test_provider_capabilities_method() {
    let provider = create_test_provider().await;
    let caps = provider.capabilities();

    assert!(caps.contains(&ProviderCapability::ChatCompletion));
    assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
}

#[tokio::test]
async fn test_provider_supported_openai_params() {
    let provider = create_test_provider().await;
    let params = provider.get_supported_openai_params("@cf/meta/llama-3-8b-instruct");

    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"top_p"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"stop"));
    assert!(params.contains(&"frequency_penalty"));
    assert!(params.contains(&"presence_penalty"));
    assert!(params.contains(&"n"));
    assert!(params.contains(&"seed"));
}

#[tokio::test]
async fn test_provider_models_not_empty() {
    let provider = create_test_provider().await;
    assert!(!provider.models().is_empty());
}

#[tokio::test]
async fn test_provider_models_have_cloudflare_prefix() {
    let provider = create_test_provider().await;
    for model in provider.models() {
        assert!(
            model.id.starts_with("cloudflare/"),
            "Model {} should start with cloudflare/",
            model.id
        );
        assert_eq!(model.provider, "cloudflare");
    }
}

// ==================== Transform Request Tests ====================

#[test]
fn test_transform_request() {
    let config = CloudflareConfig {
        account_id: Some("test".to_string()),
        api_token: Some("test".to_string()),
        ..Default::default()
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let provider = runtime.block_on(CloudflareProvider::new(config)).unwrap();

    let request = ChatRequest {
        model: "@cf/meta/llama-3-8b-instruct".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            thinking: None,
            audio: None,
        }],
        temperature: Some(0.7),
        max_tokens: Some(100),
        stream: false,
        ..Default::default()
    };

    let transformed = provider.transform_to_cloudflare_format(&request);
    assert!(transformed["messages"].is_array());
    let temp_value = transformed["temperature"].as_f64().unwrap();
    assert!(
        (temp_value - 0.7).abs() < 1e-6,
        "Expected 0.7, got {}",
        temp_value
    );
    assert_eq!(transformed["max_tokens"], 100);
}

#[tokio::test]
async fn test_transform_request_with_top_p() {
    let provider = create_test_provider().await;

    let request = ChatRequest {
        model: "@cf/meta/llama-3-8b-instruct".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        top_p: Some(0.9),
        ..Default::default()
    };

    let transformed = provider.transform_to_cloudflare_format(&request);
    let top_p_value = transformed["top_p"].as_f64().unwrap();
    assert!((top_p_value - 0.9).abs() < 1e-6);
}

#[tokio::test]
async fn test_transform_request_with_streaming() {
    let provider = create_test_provider().await;

    let request = ChatRequest {
        model: "@cf/meta/llama-3-8b-instruct".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        stream: true,
        ..Default::default()
    };

    let transformed = provider.transform_to_cloudflare_format(&request);
    assert_eq!(transformed["stream"], true);
}

#[tokio::test]
async fn test_transform_request_multiple_messages() {
    let provider = create_test_provider().await;

    let request = ChatRequest {
        model: "@cf/meta/llama-3-8b-instruct".to_string(),
        messages: vec![
            ChatMessage {
                role: MessageRole::System,
                content: Some(MessageContent::Text(
                    "You are a helpful assistant.".to_string(),
                )),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: Some(MessageContent::Text("Hi there!".to_string())),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let transformed = provider.transform_to_cloudflare_format(&request);
    let messages = transformed["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
}

#[tokio::test]
async fn test_transform_request_no_optional_params() {
    let provider = create_test_provider().await;

    let request = ChatRequest {
        model: "@cf/meta/llama-3-8b-instruct".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let transformed = provider.transform_to_cloudflare_format(&request);
    assert!(transformed["messages"].is_array());
    assert!(transformed.get("temperature").is_none() || transformed["temperature"].is_null());
    assert!(transformed.get("max_tokens").is_none() || transformed["max_tokens"].is_null());
}

// ==================== Transform Response Tests ====================

#[tokio::test]
async fn test_transform_response_success() {
    let provider = create_test_provider().await;

    let response_json = serde_json::json!({
        "result": {
            "response": "Hello! I'm doing well, thank you for asking."
        },
        "success": true
    });
    let response_bytes = serde_json::to_vec(&response_json).unwrap();

    let result = provider
        .transform_response(
            &response_bytes,
            "@cf/meta/llama-3-8b-instruct",
            "test-request-id",
        )
        .await;

    assert!(result.is_ok());
    let chat_response = result.unwrap();
    assert_eq!(chat_response.id, "test-request-id");
    assert_eq!(chat_response.model, "@cf/meta/llama-3-8b-instruct");
    assert!(!chat_response.choices.is_empty());
}

#[tokio::test]
async fn test_transform_response_empty_content() {
    let provider = create_test_provider().await;

    let response_json = serde_json::json!({
        "result": {},
        "success": true
    });
    let response_bytes = serde_json::to_vec(&response_json).unwrap();

    let result = provider
        .transform_response(
            &response_bytes,
            "@cf/meta/llama-3-8b-instruct",
            "test-request-id",
        )
        .await;

    assert!(result.is_ok());
    let chat_response = result.unwrap();
    assert!(!chat_response.choices.is_empty());
}

#[tokio::test]
async fn test_transform_response_invalid_json() {
    let provider = create_test_provider().await;

    let response_bytes = b"not valid json";

    let result = provider
        .transform_response(
            response_bytes,
            "@cf/meta/llama-3-8b-instruct",
            "test-request-id",
        )
        .await;

    assert!(result.is_err());
}

// ==================== OpenAI Params Mapping Tests ====================

#[tokio::test]
async fn test_map_openai_params_passthrough() {
    let provider = create_test_provider().await;

    let mut params = HashMap::new();
    params.insert("temperature".to_string(), serde_json::json!(0.7));
    params.insert("max_tokens".to_string(), serde_json::json!(100));

    let result = provider
        .map_openai_params(params.clone(), "@cf/meta/llama-3-8b-instruct")
        .await;

    assert!(result.is_ok());
    let mapped = result.unwrap();
    assert_eq!(mapped, params);
}

// ==================== Cost Calculation Tests ====================

#[tokio::test]
async fn test_calculate_cost_known_model() {
    let provider = create_test_provider().await;

    let cost = provider
        .calculate_cost("@cf/meta/llama-3-8b-instruct", 1000, 500)
        .await;

    assert!(cost.is_ok());
    let cost_value = cost.unwrap();
    assert!(cost_value >= 0.0);
}

#[tokio::test]
async fn test_calculate_cost_unknown_model() {
    let provider = create_test_provider().await;

    let cost = provider.calculate_cost("unknown-model", 1000, 500).await;

    assert!(cost.is_err());
}

// ==================== Streaming Tests ====================

#[tokio::test]
async fn test_streaming_not_implemented() {
    let provider = create_test_provider().await;

    let request = ChatRequest {
        model: "@cf/meta/llama-3-8b-instruct".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        stream: true,
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.chat_completion_stream(request, context).await;

    assert!(result.is_err());
}

// ==================== Embeddings Tests ====================

#[tokio::test]
async fn test_embeddings_not_implemented() {
    let provider = create_test_provider().await;

    let request = EmbeddingRequest {
        model: "@cf/baai/bge-base-en-v1.5".to_string(),
        input: crate::core::types::embedding::EmbeddingInput::Text("test".to_string()),
        encoding_format: None,
        dimensions: None,
        user: None,
        task_type: None,
    };

    let context = RequestContext::default();
    let result = provider.embeddings(request, context).await;

    assert!(result.is_err());
}

// ==================== Error Mapper Tests ====================

#[tokio::test]
async fn test_get_error_mapper() {
    let provider = create_test_provider().await;
    let _mapper = provider.get_error_mapper();
    // Verify we can get an error mapper - just checking it doesn't panic
}

// ==================== Transform Request Trait Tests ====================

#[tokio::test]
async fn test_transform_request_trait() {
    let provider = create_test_provider().await;

    let request = ChatRequest {
        model: "@cf/meta/llama-3-8b-instruct".to_string(),
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
    let transformed = result.unwrap();
    assert!(transformed["messages"].is_array());
}

// ==================== Clone/Debug Tests ====================

#[tokio::test]
async fn test_provider_clone() {
    let provider = create_test_provider().await;
    let cloned = provider.clone();

    assert_eq!(provider.name(), cloned.name());
    assert_eq!(provider.models().len(), cloned.models().len());
}

#[tokio::test]
async fn test_provider_debug() {
    let provider = create_test_provider().await;
    let debug_str = format!("{:?}", provider);

    assert!(debug_str.contains("CloudflareProvider"));
}
