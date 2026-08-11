use super::{OllamaConfig, OllamaProvider};
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::context::RequestContext;
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

    request.tool_choice = Some(ToolChoice::String("auto".to_string()));
    let body =
        LLMProvider::transform_request(&provider, request.clone(), RequestContext::default())
            .await
            .expect("automatic tool selection should transform");
    assert_eq!(body["tools"].as_array().map(Vec::len), Some(1));

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
