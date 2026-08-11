use super::{OllamaConfig, OllamaProvider};
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::content::{ContentPart, ImageUrl};
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::{EmbeddingInput, EmbeddingRequest};
use crate::core::types::message::{MessageContent, MessageRole};
use crate::core::types::responses::FinishReason;
use crate::core::types::tools::{
    FunctionCall, FunctionChoice, FunctionDefinition, ResponseFormat, Tool, ToolCall, ToolChoice,
    ToolType,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn transform_request_emits_tool_arguments_as_json_object() {
    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();
    let request = ChatRequest {
        model: "ollama/llama3:8b".to_string(),
        messages: vec![
            ChatMessage {
                role: MessageRole::Assistant,
                content: Some(MessageContent::Text(String::new())),
                tool_calls: Some(vec![ToolCall {
                    id: "call_123".to_string(),
                    tool_type: "function".to_string(),
                    function: FunctionCall {
                        name: "get_weather".to_string(),
                        arguments: r#"{"location":"NYC"}"#.to_string(),
                    },
                }]),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::Tool,
                content: Some(MessageContent::Text("sunny".to_string())),
                tool_call_id: Some("call_123".to_string()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let body = LLMProvider::transform_request(&provider, request, RequestContext::default())
        .await
        .expect("valid tool arguments should transform");
    assert_eq!(
        body["messages"][0]["tool_calls"][0]["function"]["arguments"],
        serde_json::json!({"location": "NYC"})
    );
    assert_eq!(body["messages"][0]["tool_calls"][0]["id"], "call_123");
    assert_eq!(body["messages"][1]["tool_call_id"], "call_123");
    assert_eq!(body["messages"][1]["tool_name"], "get_weather");
}

fn weather_tool() -> Tool {
    Tool {
        tool_type: ToolType::Function,
        function: FunctionDefinition {
            name: "get_weather".to_string(),
            description: None,
            parameters: Some(serde_json::json!({"type": "object"})),
        },
    }
}

#[tokio::test]
async fn transform_request_preserves_generation_and_tool_selection_contract() {
    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {"answer": {"type": "string"}}
    });
    let mut request = ChatRequest {
        model: "ollama/llama3:8b".to_string(),
        tools: Some(vec![weather_tool()]),
        tool_choice: Some(ToolChoice::String("none".to_string())),
        max_tokens: Some(10),
        max_completion_tokens: Some(20),
        response_format: Some(ResponseFormat {
            format_type: "json_schema".to_string(),
            json_schema: Some(schema.clone()),
            response_type: None,
        }),
        ..Default::default()
    };

    let body =
        LLMProvider::transform_request(&provider, request.clone(), RequestContext::default())
            .await
            .expect("supported generation controls should transform");
    assert!(body.get("tools").is_none());
    assert_eq!(body["options"]["num_predict"], 20);
    assert_eq!(body["format"], schema);

    request.response_format.as_mut().unwrap().json_schema = Some(serde_json::json!({
        "name": "answer",
        "strict": true,
        "schema": schema
    }));
    let body =
        LLMProvider::transform_request(&provider, request.clone(), RequestContext::default())
            .await
            .expect("standard JSON schema envelope should transform");
    assert_eq!(body["format"]["type"], "object");

    request.tool_choice = Some(ToolChoice::String("auto".to_string()));
    let body =
        LLMProvider::transform_request(&provider, request.clone(), RequestContext::default())
            .await
            .expect("automatic tool selection should transform");
    assert_eq!(body["tools"].as_array().map(Vec::len), Some(1));

    request.n = Some(2);
    let error =
        LLMProvider::transform_request(&provider, request.clone(), RequestContext::default())
            .await
            .expect_err("native Ollama cannot return multiple choices");
    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    request.n = None;

    request.tool_choice = Some(ToolChoice::Specific {
        choice_type: "function".to_string(),
        function: Some(FunctionChoice {
            name: "get_weather".to_string(),
        }),
    });
    let error = LLMProvider::transform_request(&provider, request, RequestContext::default())
        .await
        .expect_err("native Ollama cannot force a specific tool");
    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
}

#[tokio::test]
async fn configured_system_overrides_request_system_and_template_is_rejected() {
    let provider = OllamaProvider::new(OllamaConfig {
        system: Some("configured system".to_string()),
        ..Default::default()
    })
    .await
    .unwrap();
    let request = ChatRequest::new("ollama/llama3:8b")
        .add_system_message("request system")
        .add_user_message("hello");
    let body = LLMProvider::transform_request(&provider, request, RequestContext::default())
        .await
        .unwrap();
    assert_eq!(body["messages"].as_array().map(Vec::len), Some(2));
    assert_eq!(body["messages"][0]["content"], "configured system");

    let error = OllamaProvider::new(OllamaConfig {
        template: Some("{{ .Prompt }}".to_string()),
        ..Default::default()
    })
    .await
    .expect_err("unsupported template override must fail configuration");
    assert!(matches!(error, ProviderError::Configuration { .. }));
}

#[tokio::test]
async fn transform_request_rejects_remote_image_urls() {
    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();
    let request = ChatRequest {
        model: "ollama/llava".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Parts(vec![ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: "https://example.test/image.png".to_string(),
                    detail: None,
                },
            }])),
            ..Default::default()
        }],
        ..Default::default()
    };
    let error = LLMProvider::transform_request(&provider, request, RequestContext::default())
        .await
        .expect_err("native Ollama requires inline base64 images");
    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
}

#[tokio::test]
async fn non_streaming_tool_calls_override_stop_finish_reason() {
    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();
    let raw = serde_json::to_vec(&serde_json::json!({
        "model": "llama3:8b",
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "function": {"name": "get_weather", "arguments": {"city": "Paris"}}
            }]
        },
        "done": true,
        "done_reason": "stop"
    }))
    .unwrap();

    let response = LLMProvider::transform_response(&provider, &raw, "llama3:8b", "request-1")
        .await
        .expect("tool response should parse");
    assert_eq!(
        response.choices[0].finish_reason,
        Some(FinishReason::ToolCalls)
    );
    assert!(
        response.choices[0].message.tool_calls.as_ref().unwrap()[0]
            .id
            .starts_with("call_")
    );
}

#[tokio::test]
async fn non_streaming_maps_upstream_error_status() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        let (mut socket, _) = listener
            .accept()
            .await
            .expect("test server accepts request");
        let mut buffer = [0_u8; 4096];
        let _ = socket
            .read(&mut buffer)
            .await
            .expect("test server reads request");
        let body = r#"{"error":"model not found"}"#;
        let response = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("test server writes response");
    });

    let provider = OllamaProvider::new(OllamaConfig {
        api_base: Some(format!("http://{address}")),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        ..Default::default()
    })
    .await?;
    let error = LLMProvider::chat_completion(
        &provider,
        ChatRequest::new("ollama/missing").add_user_message("hello"),
        RequestContext::default(),
    )
    .await
    .expect_err("404 response must map before JSON success parsing");

    assert!(matches!(error, ProviderError::ModelNotFound { .. }));
    Ok(())
}

#[tokio::test]
async fn embeddings_preserve_prompt_token_usage() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        let (mut socket, _) = listener
            .accept()
            .await
            .expect("test server accepts request");
        let mut buffer = [0_u8; 4096];
        socket
            .read(&mut buffer)
            .await
            .expect("test server reads request");
        let body = r#"{"embeddings":[[0.1,0.2]],"prompt_eval_count":7}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("test server writes response");
    });
    let provider = OllamaProvider::new(OllamaConfig {
        api_base: Some(format!("http://{address}")),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        ..Default::default()
    })
    .await?;
    let response = LLMProvider::embeddings(
        &provider,
        EmbeddingRequest {
            model: "ollama/nomic-embed-text".to_string(),
            input: EmbeddingInput::Text("hello".to_string()),
            user: None,
            encoding_format: None,
            dimensions: None,
            task_type: None,
        },
        RequestContext::default(),
    )
    .await?;
    let usage = response.usage.expect("embedding usage should be present");
    assert_eq!(usage.prompt_tokens, 7);
    assert_eq!(usage.total_tokens, 7);
    Ok(())
}
