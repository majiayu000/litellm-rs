use super::*;
use crate::core::providers::ProviderType;
use serde_json::json;
use std::collections::HashMap;

// ==================== TransformChatRequest Tests ====================

#[test]
fn test_chat_request_serialization_full() {
    let request = TransformChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: Some(json!("Hello")),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }],
        temperature: Some(0.7),
        max_tokens: Some(1000),
        stream: Some(false),
        functions: None,
        function_call: None,
        tools: None,
        tool_choice: None,
        top_p: Some(0.9),
        presence_penalty: Some(0.0),
        frequency_penalty: Some(0.0),
        stop: Some(vec!["END".to_string()]),
        response_format: None,
        seed: Some(42),
        logit_bias: None,
        user: Some("test-user".to_string()),
        extra_headers: None,
        extra_body: None,
        thinking: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["model"], "gpt-4");
    assert_eq!(json["temperature"], 0.7);
    assert_eq!(json["max_tokens"], 1000);
    assert_eq!(json["seed"], 42);
}

#[test]
fn test_chat_request_minimal() {
    let request = TransformChatRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![],
        temperature: None,
        max_tokens: None,
        stream: None,
        functions: None,
        function_call: None,
        tools: None,
        tool_choice: None,
        top_p: None,
        presence_penalty: None,
        frequency_penalty: None,
        stop: None,
        response_format: None,
        seed: None,
        logit_bias: None,
        user: None,
        extra_headers: None,
        extra_body: None,
        thinking: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["model"], "gpt-3.5-turbo");
    assert!(json["temperature"].is_null());
}

// ==================== ChatMessage Tests ====================

#[test]
fn test_chat_message_user() {
    let message = ChatMessage {
        role: "user".to_string(),
        content: Some(json!("Hello, world!")),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
    };

    let json = serde_json::to_value(&message).unwrap();
    assert_eq!(json["role"], "user");
    assert_eq!(json["content"], "Hello, world!");
}

#[test]
fn test_chat_message_assistant_with_tool_calls() {
    let message = ChatMessage {
        role: "assistant".to_string(),
        content: None,
        name: None,
        function_call: None,
        tool_calls: Some(vec![ToolCall {
            id: "call_abc123".to_string(),
            tool_type: "function".to_string(),
            function: FunctionCallResponse {
                name: "get_weather".to_string(),
                arguments: r#"{"location": "NYC"}"#.to_string(),
            },
        }]),
        tool_call_id: None,
    };

    let json = serde_json::to_value(&message).unwrap();
    assert_eq!(json["role"], "assistant");
    assert_eq!(json["tool_calls"][0]["id"], "call_abc123");
    assert_eq!(json["tool_calls"][0]["function"]["name"], "get_weather");
}

#[test]
fn test_chat_message_tool_response() {
    let message = ChatMessage {
        role: "tool".to_string(),
        content: Some(json!("Weather: Sunny, 72°F")),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: Some("call_abc123".to_string()),
    };

    let json = serde_json::to_value(&message).unwrap();
    assert_eq!(json["role"], "tool");
    assert_eq!(json["tool_call_id"], "call_abc123");
}

// ==================== Function Tests ====================

#[test]
fn test_function_serialization() {
    let function = Function {
        name: "get_weather".to_string(),
        description: Some("Get weather information".to_string()),
        parameters: json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            }
        }),
    };

    let json = serde_json::to_value(&function).unwrap();
    assert_eq!(json["name"], "get_weather");
    assert_eq!(json["description"], "Get weather information");
}

// ==================== Tool Tests ====================

#[test]
fn test_tool_serialization() {
    let tool = Tool {
        tool_type: "function".to_string(),
        function: Function {
            name: "search".to_string(),
            description: Some("Search the web".to_string()),
            parameters: json!({"type": "object"}),
        },
    };

    let json = serde_json::to_value(&tool).unwrap();
    assert_eq!(json["type"], "function");
    assert_eq!(json["function"]["name"], "search");
}

// ==================== ToolCall Tests ====================

#[test]
fn test_tool_call_serialization() {
    let tool_call = ToolCall {
        id: "call_123".to_string(),
        tool_type: "function".to_string(),
        function: FunctionCallResponse {
            name: "calculate".to_string(),
            arguments: r#"{"a": 1, "b": 2}"#.to_string(),
        },
    };

    let json = serde_json::to_value(&tool_call).unwrap();
    assert_eq!(json["id"], "call_123");
    assert_eq!(json["type"], "function");
    assert_eq!(json["function"]["name"], "calculate");
}

// ==================== ResponseFormat Tests ====================

#[test]
fn test_response_format_json() {
    let format = ResponseFormat {
        format_type: "json_object".to_string(),
    };

    let json = serde_json::to_value(&format).unwrap();
    assert_eq!(json["type"], "json_object");
}

#[test]
fn test_response_format_text() {
    let format = ResponseFormat {
        format_type: "text".to_string(),
    };

    let json = serde_json::to_value(&format).unwrap();
    assert_eq!(json["type"], "text");
}

// ==================== ChatResponse Tests ====================

#[test]
fn test_chat_response_serialization() {
    let response = ChatResponse {
        id: "chatcmpl-123".to_string(),
        object: "chat.completion".to_string(),
        created: 1699472400,
        model: "gpt-4".to_string(),
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: Some(json!("Hello!")),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            },
            finish_reason: Some("stop".to_string()),
            logprobs: None,
        }],
        usage: Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        }),
        system_fingerprint: Some("fp_abc123".to_string()),
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["id"], "chatcmpl-123");
    assert_eq!(json["choices"][0]["message"]["content"], "Hello!");
    assert_eq!(json["usage"]["total_tokens"], 15);
}

// ==================== Usage Tests ====================

#[test]
fn test_usage_serialization() {
    let usage = Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
    };

    let json = serde_json::to_value(&usage).unwrap();
    assert_eq!(json["prompt_tokens"], 100);
    assert_eq!(json["completion_tokens"], 50);
    assert_eq!(json["total_tokens"], 150);
}

// ==================== EmbeddingRequest Tests ====================

#[test]
fn test_embedding_request_string_input() {
    let request = EmbeddingRequest {
        model: "text-embedding-ada-002".to_string(),
        input: EmbeddingInput::String("Hello world".to_string()),
        encoding_format: None,
        dimensions: None,
        user: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["model"], "text-embedding-ada-002");
    assert_eq!(json["input"], "Hello world");
}

#[test]
fn test_embedding_request_array_input() {
    let request = EmbeddingRequest {
        model: "text-embedding-3-small".to_string(),
        input: EmbeddingInput::Strings(vec!["Hello".to_string(), "World".to_string()]),
        encoding_format: Some("float".to_string()),
        dimensions: Some(256),
        user: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["dimensions"], 256);
}

#[test]
fn test_embedding_request_token_input() {
    let request = EmbeddingRequest {
        model: "text-embedding-ada-002".to_string(),
        input: EmbeddingInput::Tokens(vec![1, 2, 3, 4]),
        encoding_format: None,
        dimensions: None,
        user: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert!(json["input"].is_array());
}

// ==================== EmbeddingResponse Tests ====================

#[test]
fn test_embedding_response_serialization() {
    let response = EmbeddingResponse {
        object: "list".to_string(),
        data: vec![EmbeddingData {
            object: "embedding".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            index: 0,
        }],
        model: "text-embedding-ada-002".to_string(),
        usage: Usage {
            prompt_tokens: 5,
            completion_tokens: 0,
            total_tokens: 5,
        },
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["object"], "list");
    assert_eq!(json["data"][0]["embedding"][0], 0.1);
}

// ==================== ProviderRequest Tests ====================

#[test]
fn test_provider_request_serialization() {
    let request = ProviderRequest {
        endpoint: "/v1/chat/completions".to_string(),
        method: "POST".to_string(),
        headers: HashMap::from([("Content-Type".to_string(), "application/json".to_string())]),
        body: json!({"model": "gpt-4"}),
        query_params: HashMap::new(),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["endpoint"], "/v1/chat/completions");
    assert_eq!(json["method"], "POST");
}

// ==================== ProviderResponse Tests ====================

#[test]
fn test_provider_response_serialization() {
    let response = ProviderResponse {
        status_code: 200,
        headers: HashMap::from([("content-type".to_string(), "application/json".to_string())]),
        body: json!({"id": "test"}),
        latency_ms: 150.5,
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["status_code"], 200);
    assert_eq!(json["latency_ms"], 150.5);
}

// ==================== TransformMetadata Tests ====================

#[test]
fn test_transform_metadata_serialization() {
    let metadata = TransformMetadata {
        provider_type: ProviderType::Anthropic,
        original_model: "gpt-4".to_string(),
        transformed_model: "claude-3-sonnet".to_string(),
        transformations_applied: vec!["message_transform".to_string()],
        warnings: vec!["Some features not supported".to_string()],
        cost_estimate: Some(0.01),
    };

    let json = serde_json::to_value(&metadata).unwrap();
    assert_eq!(json["original_model"], "gpt-4");
    assert_eq!(json["transformed_model"], "claude-3-sonnet");
    assert_eq!(json["cost_estimate"], 0.01);
}

// ==================== ModelMapping Tests ====================

#[test]
fn test_model_mapping_serialization() {
    let mapping = ModelMapping {
        provider_model: "claude-3-sonnet-20240229".to_string(),
        openai_equivalent: "gpt-4".to_string(),
        capabilities: vec!["chat".to_string(), "vision".to_string()],
        parameter_mappings: HashMap::from([("max_tokens".to_string(), "max_tokens".to_string())]),
    };

    let json = serde_json::to_value(&mapping).unwrap();
    assert_eq!(json["provider_model"], "claude-3-sonnet-20240229");
    assert_eq!(json["capabilities"][0], "chat");
}

// ==================== TransformContext Tests ====================

#[test]
fn test_transform_context_creation() {
    let context = TransformContext {
        provider_type: ProviderType::VertexAI,
        original_model: "gpt-4".to_string(),
        target_model: "gemini-pro".to_string(),
        config: HashMap::new(),
        metadata: HashMap::from([("request_id".to_string(), "req-123".to_string())]),
    };

    assert_eq!(context.original_model, "gpt-4");
    assert_eq!(context.target_model, "gemini-pro");
    assert_eq!(
        context.metadata.get("request_id"),
        Some(&"req-123".to_string())
    );
}

// ==================== DefaultTransformEngine Tests ====================

#[test]
fn test_default_transform_engine_new() {
    let engine = DefaultTransformEngine::new();

    // Should have initialized default pipelines
    let anthropic_transforms = engine.get_supported_transformations(&ProviderType::Anthropic);
    assert!(!anthropic_transforms.is_empty());

    let vertexai_transforms = engine.get_supported_transformations(&ProviderType::VertexAI);
    assert!(!vertexai_transforms.is_empty());
}

#[test]
fn test_default_transform_engine_model_mapping_anthropic() {
    let engine = DefaultTransformEngine::new();

    // Claude model should pass through
    let mapped = engine.map_model_name("claude-3-opus", &ProviderType::Anthropic);
    assert_eq!(mapped, "claude-3-opus");

    // Non-Claude model should get default
    let mapped = engine.map_model_name("gpt-4", &ProviderType::Anthropic);
    assert_eq!(mapped, "claude-3-sonnet-20240229");
}

#[test]
fn test_default_transform_engine_model_mapping_vertexai() {
    let engine = DefaultTransformEngine::new();

    // Gemini model should pass through
    let mapped = engine.map_model_name("gemini-1.5-pro", &ProviderType::VertexAI);
    assert_eq!(mapped, "gemini-1.5-pro");

    // Non-Gemini model should get default
    let mapped = engine.map_model_name("gpt-4", &ProviderType::VertexAI);
    assert_eq!(mapped, "gemini-1.0-pro");
}

#[test]
fn test_default_transform_engine_model_mapping_other() {
    let engine = DefaultTransformEngine::new();

    // Other providers should pass model through unchanged
    let mapped = engine.map_model_name("custom-model", &ProviderType::OpenAI);
    assert_eq!(mapped, "custom-model");
}

#[tokio::test]
async fn test_validate_request_compatibility_anthropic() {
    let engine = DefaultTransformEngine::new();

    let request = TransformChatRequest {
        model: "claude-3".to_string(),
        messages: vec![],
        temperature: None,
        max_tokens: None,
        stream: None,
        functions: Some(vec![]), // Anthropic doesn't support functions
        function_call: None,
        tools: None,
        tool_choice: None,
        top_p: None,
        presence_penalty: None,
        frequency_penalty: None,
        stop: None,
        response_format: None,
        seed: None,
        logit_bias: Some(HashMap::new()), // Also not supported
        user: None,
        extra_headers: None,
        extra_body: None,
        thinking: None,
    };

    let issues = engine
        .validate_request_compatibility(&request, &ProviderType::Anthropic)
        .await
        .unwrap();
    assert!(issues.iter().any(|i| i.contains("Functions")));
    assert!(issues.iter().any(|i| i.contains("Logit bias")));
}

#[tokio::test]
async fn test_validate_request_compatibility_vertexai() {
    let engine = DefaultTransformEngine::new();

    let request = TransformChatRequest {
        model: "gemini-pro".to_string(),
        messages: vec![],
        temperature: None,
        max_tokens: None,
        stream: None,
        functions: Some(vec![]),
        function_call: None,
        tools: None,
        tool_choice: None,
        top_p: None,
        presence_penalty: None,
        frequency_penalty: None,
        stop: None,
        response_format: None,
        seed: None,
        logit_bias: None,
        user: None,
        extra_headers: None,
        extra_body: None,
        thinking: None,
    };

    let issues = engine
        .validate_request_compatibility(&request, &ProviderType::VertexAI)
        .await
        .unwrap();
    assert!(issues.iter().any(|i| i.contains("Function calling")));
}

#[tokio::test]
async fn test_validate_request_compatibility_no_issues() {
    let engine = DefaultTransformEngine::new();

    let request = TransformChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        temperature: None,
        max_tokens: None,
        stream: None,
        functions: None,
        function_call: None,
        tools: None,
        tool_choice: None,
        top_p: None,
        presence_penalty: None,
        frequency_penalty: None,
        stop: None,
        response_format: None,
        seed: None,
        logit_bias: None,
        user: None,
        extra_headers: None,
        extra_body: None,
        thinking: None,
    };

    let issues = engine
        .validate_request_compatibility(&request, &ProviderType::OpenAI)
        .await
        .unwrap();
    assert!(issues.is_empty());
}

// ==================== Transform Trait Tests ====================

#[tokio::test]
async fn test_anthropic_message_transform_name() {
    let transform = AnthropicMessageTransform::new();
    assert_eq!(transform.name(), "anthropic_message_transform");
}

#[tokio::test]
async fn test_anthropic_parameter_transform_name() {
    let transform = AnthropicParameterTransform::new();
    assert_eq!(transform.name(), "anthropic_parameter_transform");
}

#[tokio::test]
async fn test_google_message_transform_name() {
    let transform = GoogleMessageTransform::new();
    assert_eq!(transform.name(), "google_message_transform");
}

#[tokio::test]
async fn test_google_parameter_transform_name() {
    let transform = GoogleParameterTransform::new();
    assert_eq!(transform.name(), "google_parameter_transform");
}

#[tokio::test]
async fn test_transform_passthrough() {
    let transform = AnthropicMessageTransform::new();
    let context = TransformContext {
        provider_type: ProviderType::Anthropic,
        original_model: "gpt-4".to_string(),
        target_model: "claude-3".to_string(),
        config: HashMap::new(),
        metadata: HashMap::new(),
    };

    let input = json!({"messages": [{"role": "user", "content": "Hello"}]});
    let result = transform
        .transform_request(input.clone(), &context)
        .await
        .unwrap();

    // Current implementation is passthrough
    assert_eq!(result, input);
}

// ==================== Clone and Debug Tests ====================

#[test]
fn test_chat_request_clone() {
    let request = TransformChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        temperature: Some(0.5),
        max_tokens: None,
        stream: None,
        functions: None,
        function_call: None,
        tools: None,
        tool_choice: None,
        top_p: None,
        presence_penalty: None,
        frequency_penalty: None,
        stop: None,
        response_format: None,
        seed: None,
        logit_bias: None,
        user: None,
        extra_headers: None,
        extra_body: None,
        thinking: None,
    };

    let cloned = request.clone();
    assert_eq!(request.model, cloned.model);
    assert_eq!(request.temperature, cloned.temperature);
}

#[test]
fn test_chat_response_debug() {
    let response = ChatResponse {
        id: "test-id".to_string(),
        object: "chat.completion".to_string(),
        created: 12345,
        model: "gpt-4".to_string(),
        choices: vec![],
        usage: None,
        system_fingerprint: None,
    };

    let debug = format!("{:?}", response);
    assert!(debug.contains("ChatResponse"));
    assert!(debug.contains("test-id"));
}

#[test]
fn test_transform_context_clone() {
    let context = TransformContext {
        provider_type: ProviderType::OpenAI,
        original_model: "model-a".to_string(),
        target_model: "model-b".to_string(),
        config: HashMap::new(),
        metadata: HashMap::new(),
    };

    let cloned = context.clone();
    assert_eq!(context.original_model, cloned.original_model);
}
