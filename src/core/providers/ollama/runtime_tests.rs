use super::{OllamaConfig, OllamaProvider};
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::context::RequestContext;
use crate::core::types::message::{MessageContent, MessageRole};
use crate::core::types::tools::{FunctionCall, ToolCall};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn transform_request_emits_tool_arguments_as_json_object() {
    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();
    let request = ChatRequest {
        model: "ollama/llama3:8b".to_string(),
        messages: vec![ChatMessage {
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
        }],
        ..Default::default()
    };

    let body = LLMProvider::transform_request(&provider, request, RequestContext::default())
        .await
        .expect("valid tool arguments should transform");
    assert_eq!(
        body["messages"][0]["tool_calls"][0]["function"]["arguments"],
        serde_json::json!({"location": "NYC"})
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
