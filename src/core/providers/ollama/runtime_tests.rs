use super::streaming::OllamaStream;
use super::{OllamaConfig, OllamaProvider};
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::content::{ContentPart, ImageUrl};
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::{EmbeddingInput, EmbeddingRequest};
use crate::core::types::message::{MessageContent, MessageRole};
use crate::core::types::responses::{EmbeddingResponse, FinishReason};
use crate::core::types::tools::{
    FunctionCall, FunctionChoice, FunctionDefinition, ResponseFormat, Tool, ToolCall, ToolChoice,
    ToolType,
};
use bytes::Bytes;
use futures::StreamExt as _;
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
async fn transform_request_preserves_raw_schema_annotation_keywords() {
    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();
    let request = ChatRequest {
        model: "ollama/llama3:8b".to_string(),
        response_format: Some(ResponseFormat {
            format_type: "json_schema".to_string(),
            json_schema: Some(serde_json::json!({
                "name": "answer",
                "strict": true,
                "schema": {"custom": "annotation"},
                "type": "object"
            })),
            response_type: None,
        }),
        ..Default::default()
    };

    let body = LLMProvider::transform_request(&provider, request, RequestContext::default())
        .await
        .expect("raw JSON Schema annotation keywords must not imply an OpenAI envelope");
    assert_eq!(body["format"]["name"], "answer");
    assert_eq!(body["format"]["strict"], true);
    assert_eq!(body["format"]["schema"]["custom"], "annotation");
}

#[tokio::test]
async fn transform_request_forwards_validated_request_options_with_typed_precedence() {
    let provider = OllamaProvider::new(OllamaConfig {
        num_ctx: Some(8_192),
        repeat_penalty: Some(1.1),
        ..Default::default()
    })
    .await
    .unwrap();
    let request = ChatRequest {
        model: "ollama/llama3:8b".to_string(),
        max_completion_tokens: Some(32),
        extra_params: std::collections::HashMap::from([
            ("num_ctx".to_string(), serde_json::json!(8_192.0)),
            ("num_predict".to_string(), serde_json::json!(64.0)),
            ("repeat_penalty".to_string(), serde_json::json!(1.25)),
        ]),
        ..Default::default()
    };

    let body = LLMProvider::transform_request(&provider, request, RequestContext::default())
        .await
        .expect("valid request-scoped options should transform");

    assert_eq!(body["options"]["num_ctx"], 8_192);
    assert_eq!(body["options"]["num_predict"], 32);
    assert_eq!(body["options"]["repeat_penalty"], 1.25);
}

#[tokio::test]
async fn transform_request_enforces_configured_num_ctx_ceiling() {
    let provider = OllamaProvider::new(OllamaConfig {
        num_ctx: Some(2_048),
        ..Default::default()
    })
    .await
    .unwrap();
    let request = ChatRequest {
        model: "ollama/llama3:8b".to_string(),
        extra_params: std::collections::HashMap::from([(
            "num_ctx".to_string(),
            serde_json::json!(4_096),
        )]),
        ..Default::default()
    };

    let error = LLMProvider::transform_request(&provider, request, RequestContext::default())
        .await
        .expect_err("request num_ctx must not exceed the operator ceiling");
    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
}

#[tokio::test]
async fn transform_request_rejects_invalid_request_scoped_option_types() {
    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();
    let invalid = [
        ("num_ctx", serde_json::json!(-1)),
        ("num_predict", serde_json::json!(1.5)),
        ("repeat_penalty", serde_json::json!("high")),
    ];

    for (name, value) in invalid {
        let request = ChatRequest {
            model: "ollama/llama3:8b".to_string(),
            extra_params: std::collections::HashMap::from([(name.to_string(), value)]),
            ..Default::default()
        };
        let error = LLMProvider::transform_request(&provider, request, RequestContext::default())
            .await
            .expect_err("invalid advertised option must fail visibly");
        assert!(
            matches!(error, ProviderError::InvalidRequest { .. }),
            "unexpected error for {name}: {error}"
        );
    }
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

fn stream_from_records(
    records: Vec<Bytes>,
) -> OllamaStream<impl futures::Stream<Item = Result<Bytes, reqwest::Error>>> {
    OllamaStream::new(futures::stream::iter(
        records.into_iter().map(Ok::<_, reqwest::Error>),
    ))
}

#[tokio::test]
async fn stream_preserves_tool_indices_and_non_stop_terminal_reason() {
    let records = [
        "{\"model\":\"llama3:8b\",\"message\":{\"role\":\"assistant\",\"tool_calls\":[{\"function\":{\"index\":2,\"name\":\"first\",\"arguments\":{}}}]},\"done\":false}\n",
        "{\"model\":\"llama3:8b\",\"message\":{\"role\":\"assistant\",\"tool_calls\":[{\"function\":{\"name\":\"second\",\"arguments\":{}}}]},\"done\":false}\n",
        "{\"model\":\"llama3:8b\",\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":true,\"done_reason\":\"length\"}",
    ];
    let mut stream = stream_from_records(
        records
            .map(|record| Bytes::from_static(record.as_bytes()))
            .to_vec(),
    );

    let first = stream.next().await.unwrap().unwrap();
    let second = stream.next().await.unwrap().unwrap();
    let last = stream.next().await.unwrap().unwrap();
    let first_call = &first.choices[0].delta.tool_calls.as_ref().unwrap()[0];
    let second_call = &second.choices[0].delta.tool_calls.as_ref().unwrap()[0];
    assert_eq!(first_call.index, 2);
    assert_eq!(second_call.index, 3);
    assert_ne!(first_call.id, second_call.id);
    assert_eq!(last.choices[0].finish_reason, Some(FinishReason::Length));
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn premature_eof_and_missing_model_fail_terminally() {
    for bytes in [
        Bytes::new(),
        Bytes::from_static(
            b"{\"message\":{\"role\":\"assistant\",\"content\":\"complete\"},\"done\":true}",
        ),
    ] {
        let mut stream = stream_from_records(vec![bytes]);
        assert!(stream.next().await.unwrap().is_err());
        assert!(stream.next().await.is_none());
    }

    let mut partial = stream_from_records(vec![Bytes::from_static(
        b"{\"model\":\"llama3:8b\",\"message\":{\"role\":\"assistant\",\"content\":\"partial\"},\"done\":false}\n",
    )]);
    assert_eq!(
        partial.next().await.unwrap().unwrap().choices[0]
            .delta
            .content
            .as_deref(),
        Some("partial")
    );
    assert!(partial.next().await.unwrap().is_err());
    assert!(partial.next().await.is_none());
}

#[tokio::test]
async fn stream_errors_preserve_status_redact_and_are_fused() {
    let records = concat!(
        "{\"error\":\"Authorization: Bearer sk-secret123 model missing\",\"status\":404}\n",
        "{\"model\":\"llama3:8b\",\"done\":true}\n"
    );
    let mut stream = stream_from_records(vec![Bytes::from_static(records.as_bytes())]);
    let error = stream.next().await.unwrap().unwrap_err();
    assert!(matches!(error, ProviderError::ModelNotFound { .. }));
    assert!(!error.to_string().contains("sk-secret123"));
    assert!(error.to_string().contains("[REDACTED]"));
    assert!(stream.next().await.is_none());

    for status in ["\"bad\"", "700", "-1"] {
        let record = format!("{{\"error\":\"runner failed\",\"status\":{status}}}\n");
        let mut stream = stream_from_records(vec![Bytes::from(record)]);
        assert!(matches!(
            stream.next().await.unwrap(),
            Err(ProviderError::ApiError { status: 500, .. })
        ));
        assert!(stream.next().await.is_none());
    }
}

#[tokio::test]
async fn stream_rejects_usage_overflow() {
    let mut stream = stream_from_records(vec![Bytes::from_static(
        b"{\"model\":\"llama3:8b\",\"done\":true,\"prompt_eval_count\":4294967295,\"eval_count\":1}",
    )]);
    assert!(matches!(
        stream.next().await.unwrap(),
        Err(ProviderError::ResponseParsing { .. })
    ));
    assert!(stream.next().await.is_none());
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

async fn embeddings_from_fixture(
    body: &str,
    input: EmbeddingInput,
) -> Result<EmbeddingResponse, ProviderError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("fixture listener should bind");
    let address = listener
        .local_addr()
        .expect("fixture listener should have an address");
    let body = body.to_string();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener
            .accept()
            .await
            .expect("fixture server accepts request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = socket
                .read(&mut buffer)
                .await
                .expect("fixture server reads request");
            assert!(read > 0, "fixture request ended before its complete body");
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
                continue;
            };
            let headers = std::str::from_utf8(&request[..header_end])
                .expect("fixture request headers are UTF-8");
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .expect("fixture request has Content-Length");
            if request.len() >= header_end + 4 + content_length {
                let request_body: serde_json::Value = serde_json::from_slice(
                    &request[header_end + 4..header_end + 4 + content_length],
                )
                .expect("fixture request body is JSON");
                assert_eq!(request_body["model"], "nomic-embed-text");
                assert!(request_body["input"].is_array());
                break;
            }
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("fixture server writes response");
    });

    let provider = OllamaProvider::new(OllamaConfig {
        api_base: Some(format!("http://{address}")),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        ..Default::default()
    })
    .await
    .expect("fixture provider should build");
    let result = LLMProvider::embeddings(
        &provider,
        EmbeddingRequest {
            model: "ollama/nomic-embed-text".to_string(),
            input,
            user: None,
            encoding_format: None,
            dimensions: None,
            task_type: None,
        },
        RequestContext::default(),
    )
    .await;
    server.await.expect("fixture server task should finish");
    result
}

#[tokio::test]
async fn embeddings_reject_malformed_vectors_and_count_mismatches() {
    let malformed = [
        (
            r#"{"embeddings":[42]}"#,
            EmbeddingInput::Text("a".to_string()),
        ),
        (
            r#"{"embeddings":[[0.1,"bad"]]}"#,
            EmbeddingInput::Text("a".to_string()),
        ),
        (
            r#"{"embeddings":[[1e39]]}"#,
            EmbeddingInput::Text("a".to_string()),
        ),
        (
            r#"{"embeddings":[[0.1],[0.2,0.3]]}"#,
            EmbeddingInput::Array(vec!["a".to_string(), "b".to_string()]),
        ),
        (
            r#"{"embeddings":[[0.1,0.2]]}"#,
            EmbeddingInput::Array(vec!["a".to_string(), "b".to_string()]),
        ),
    ];

    for (body, input) in malformed {
        let error = embeddings_from_fixture(body, input)
            .await
            .expect_err("malformed embedding response must fail visibly");
        assert!(matches!(error, ProviderError::ResponseParsing { .. }));
        assert!(!error.is_retryable());
    }
}
