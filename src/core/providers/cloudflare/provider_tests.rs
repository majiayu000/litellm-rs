use super::*;
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

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

async fn read_http_headers(socket: &mut TcpStream) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];

    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buffer[..bytes_read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(());
        }
    }
}

async fn health_response_base_url(status: &str) -> std::io::Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let response = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");

    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        if read_http_headers(&mut socket).await.is_err() {
            return;
        }
        if let Err(error) = socket.write_all(response.as_bytes()).await {
            eprintln!("test server failed to write response: {error}");
        }
    });

    Ok(format!("http://{addr}"))
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

#[tokio::test]
async fn health_check_is_healthy_only_for_2xx_responses() {
    for (response_status, expected_health) in [
        ("200 OK", HealthStatus::Healthy),
        ("204 No Content", HealthStatus::Healthy),
        ("302 Found", HealthStatus::Unhealthy),
        ("401 Unauthorized", HealthStatus::Unhealthy),
        ("403 Forbidden", HealthStatus::Unhealthy),
        ("429 Too Many Requests", HealthStatus::Unhealthy),
        ("500 Internal Server Error", HealthStatus::Unhealthy),
    ] {
        let api_base = health_response_base_url(response_status)
            .await
            .expect("test health endpoint should start");
        let provider = CloudflareProvider::new(CloudflareConfig {
            api_base: Some(api_base),
            ..create_test_config()
        })
        .await
        .expect("Cloudflare provider should construct");

        assert_eq!(
            provider.health_check().await,
            expected_health,
            "unexpected health for HTTP {response_status}"
        );
    }
}

// ==================== Provider Capabilities Tests ====================

#[test]
fn test_capabilities() {
    assert!(CLOUDFLARE_CAPABILITIES.contains(&ProviderCapability::ChatCompletion));
    assert!(CLOUDFLARE_CAPABILITIES.contains(&ProviderCapability::ChatCompletionStream));
    assert_eq!(CLOUDFLARE_CAPABILITIES.len(), 3);
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
    let params = provider.get_supported_openai_params("@cf/openai/gpt-oss-120b");

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

#[tokio::test]
async fn test_transform_request() {
    let config = CloudflareConfig {
        account_id: Some("test".to_string()),
        api_token: Some("test".to_string()),
        ..Default::default()
    };

    let provider = CloudflareProvider::new(config).await.unwrap();

    let request = ChatRequest {
        model: "@cf/openai/gpt-oss-120b".to_string(),
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

    let transformed = provider.transform_to_cloudflare_format(request).unwrap();
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
        model: "@cf/openai/gpt-oss-120b".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        top_p: Some(0.9),
        ..Default::default()
    };

    let transformed = provider.transform_to_cloudflare_format(request).unwrap();
    let top_p_value = transformed["top_p"].as_f64().unwrap();
    assert!((top_p_value - 0.9).abs() < 1e-6);
}

#[tokio::test]
async fn test_transform_request_with_streaming() {
    let provider = create_test_provider().await;

    let request = ChatRequest {
        model: "@cf/openai/gpt-oss-120b".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        stream: true,
        ..Default::default()
    };

    let transformed = provider.transform_to_cloudflare_format(request).unwrap();
    assert_eq!(transformed["stream"], true);
}

#[tokio::test]
async fn test_transform_request_multiple_messages() {
    let provider = create_test_provider().await;

    let request = ChatRequest {
        model: "@cf/openai/gpt-oss-120b".to_string(),
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

    let transformed = provider.transform_to_cloudflare_format(request).unwrap();
    let messages = transformed["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
}

#[tokio::test]
async fn test_transform_request_no_optional_params() {
    let provider = create_test_provider().await;

    let request = ChatRequest {
        model: "@cf/openai/gpt-oss-120b".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let transformed = provider.transform_to_cloudflare_format(request).unwrap();
    assert!(transformed["messages"].is_array());
    assert!(transformed.get("temperature").is_none() || transformed["temperature"].is_null());
    assert!(transformed.get("max_tokens").is_none() || transformed["max_tokens"].is_null());
}

// ==================== Transform Response Tests ====================

#[tokio::test]
async fn test_transform_response_success() {
    let provider = create_test_provider().await;

    let response_json = chat_response_json("Hello!");
    let response_bytes = serde_json::to_vec(&response_json).unwrap();

    let result = provider
        .transform_response(
            &response_bytes,
            "@cf/openai/gpt-oss-120b",
            "test-request-id",
        )
        .await;

    assert!(result.is_ok());
    let chat_response = result.unwrap();
    assert_eq!(chat_response.id, "test-request-id");
    assert_eq!(chat_response.model, "@cf/openai/gpt-oss-120b");
    assert!(!chat_response.choices.is_empty());
}

#[tokio::test]
async fn test_transform_response_missing_content_is_an_error() {
    let provider = create_test_provider().await;

    let response_json = serde_json::json!({
        "result": {},
        "success": true
    });
    let response_bytes = serde_json::to_vec(&response_json).unwrap();

    let result = provider
        .transform_response(
            &response_bytes,
            "@cf/openai/gpt-oss-120b",
            "test-request-id",
        )
        .await;

    assert!(matches!(result, Err(ProviderError::ResponseParsing { .. })));
}

#[tokio::test]
async fn test_chat_http_errors_preserve_status_and_retry_delay() {
    for (status, expected, body) in [
        (
            "401 Unauthorized",
            401,
            r#"{"success":false,"errors":[{"message":"denied"}]}"#,
        ),
        ("403 Forbidden", 403, "permission denied"),
        ("429 Too Many Requests", 429, "rate limited"),
        ("503 Service Unavailable", 503, r#"{"success":false}"#),
        (
            "200 OK",
            502,
            r#"{"success":false,"result":{"response":"must not return"},"errors":[{"message":"inference failed"}]}"#,
        ),
        ("200 OK", 502, r#"{"success":true,"result":{}}"#),
        ("200 OK", 502, "not JSON"),
    ] {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nRetry-After: 17\r\nConnection: close\r\n\r\n{body}",
            body.len(),
        );
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_http_headers(&mut socket).await.unwrap();
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let provider = CloudflareProvider::new(CloudflareConfig {
            api_base: Some(format!("http://{addr}")),
            ..create_test_config()
        })
        .await
        .unwrap();
        let error = provider
            .chat_completion(
                ChatRequest {
                    model: "@cf/test/model".to_string(),
                    ..Default::default()
                },
                RequestContext::default(),
            )
            .await
            .unwrap_err();
        server.await.unwrap();
        assert_eq!(error.http_facts().status, expected, "{status}: {error}");
        if expected == 429 {
            assert!(matches!(
                error,
                ProviderError::RateLimit {
                    retry_after: Some(17),
                    ..
                }
            ));
        }
        if expected == 401 {
            assert!(matches!(error, ProviderError::Authentication { .. }));
        }
    }
}

#[tokio::test]
async fn test_response_decoder_rejects_failure_but_allows_explicit_empty_text() {
    let provider = create_test_provider().await;
    let failed = br#"{"success":false,"errors":[{"message":"inference failed"}]}"#;
    let error = provider
        .transform_response(failed, "test", "id")
        .await
        .unwrap_err();
    assert_eq!(error.http_facts().status, 502);
    assert!(error.to_string().contains("inference failed"));
    let empty = serde_json::to_vec(&chat_response_json("")).unwrap();
    let response = provider
        .transform_response(&empty, "test", "id")
        .await
        .unwrap();
    // The shared OpenAI transformer normalizes empty text to absent content.
    assert!(response.choices[0].message.content.is_none());
}

#[tokio::test]
async fn test_transform_response_invalid_json() {
    let provider = create_test_provider().await;

    let response_bytes = b"not valid json";

    let result = provider
        .transform_response(response_bytes, "@cf/openai/gpt-oss-120b", "test-request-id")
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
        .map_openai_params(params.clone(), "@cf/openai/gpt-oss-120b")
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
        .calculate_cost("@cf/openai/gpt-oss-120b", 1000, 500)
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

fn chat_response_json(content: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "test-request-id", "object": "chat.completion", "created": 1,
        "model": "@cf/openai/gpt-oss-120b",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 3}
    })
}

async fn chat_server(
    status: &str,
    content_type: &str,
    body: String,
) -> (String, tokio::task::JoinHandle<serde_json::Value>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nRetry-After: 17\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let (header_end, length) = loop {
            let count = socket.read(&mut buffer).await.unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
            if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]);
                assert!(
                    headers
                        .starts_with("POST /accounts/test_account/ai/v1/chat/completions HTTP/1.1")
                );
                assert!(
                    headers
                        .to_lowercase()
                        .contains("authorization: bearer test_token")
                );
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                break (end + 4, length);
            }
        };
        while request.len() < header_end + length {
            let count = socket.read(&mut buffer).await.unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
        }
        socket.write_all(response.as_bytes()).await.unwrap();
        serde_json::from_slice(&request[header_end..header_end + length]).unwrap()
    });
    (format!("http://{addr}"), task)
}

#[tokio::test]
async fn test_chat_preserves_tools_images_reasoning_usage_and_wire_parameters() {
    let mut reply = chat_response_json("");
    reply["choices"][0]["message"] = serde_json::json!({
        "role":"assistant", "content": null, "reasoning_content": "thinking",
        "tool_calls": [{"id":"call-1","type":"function","function":{"name":"weather","arguments":"{}"}}]
    });
    reply["choices"][0]["finish_reason"] = serde_json::json!("tool_calls");
    let (base, server) = chat_server("200 OK", "application/json", reply.to_string()).await;
    let provider = CloudflareProvider::new(CloudflareConfig {
        api_base: Some(base),
        ..create_test_config()
    })
    .await
    .unwrap();
    let request: ChatRequest = serde_json::from_value(serde_json::json!({
        "model":"cloudflare/@cf/test/model", "messages":[{"role":"user","content":[{"type":"text","text":"look"},{"type":"image_url","image_url":{"url":"https://example.test/image.png"}}]}],
        "tools":[{"type":"function","function":{"name":"weather","parameters":{"type":"object"}}}],
        "tool_choice":"auto", "frequency_penalty":0.25, "reasoning_effort":"low",
        "options":{"rejectIfBusy":true}, "stream":true
    })).unwrap();
    let response = provider
        .chat_completion(request, RequestContext::default())
        .await
        .unwrap();
    let wire = server.await.unwrap();
    assert_eq!(wire["model"], "@cf/test/model");
    assert_eq!(wire["stream"], false);
    assert_eq!(wire["messages"][0]["content"][1]["type"], "image_url");
    assert_eq!(wire["tools"][0]["function"]["name"], "weather");
    assert_eq!(wire["options"]["rejectIfBusy"], true);
    assert_eq!(wire["reasoning_effort"], "low");
    assert_eq!(wire["frequency_penalty"], 0.25);
    assert_eq!(
        response.choices[0].message.thinking_text(),
        Some("thinking")
    );
    assert_eq!(
        response.choices[0].message.tool_calls.as_ref().unwrap()[0].id,
        "call-1"
    );
    assert_eq!(response.usage.unwrap().total_tokens, 3);
}

#[tokio::test]
async fn test_streaming_preserves_deltas_usage_and_reports_errors() {
    use futures::StreamExt;
    let first = r#"data: {"id":"s","model":"test","choices":[{"index":0,"delta":{"content":"hello","reasoning_content":"thought"},"finish_reason":null}]}"#;
    let finish = r#"data: {"id":"s","model":"test","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
    let usage = r#"data: {"id":"s","model":"test","choices":[],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}}"#;
    for (body, fails) in [
        (
            format!("{first}\n\n{finish}\n\n{usage}\n\ndata: [DONE]\n\n"),
            false,
        ),
        (
            format!("{first}\n\ndata: {{\"error\":{{\"message\":\"upstream failed\"}}}}\n\n"),
            true,
        ),
    ] {
        let (base, server) = chat_server("200 OK", "text/event-stream", body).await;
        let provider = CloudflareProvider::new(CloudflareConfig {
            api_base: Some(base),
            ..create_test_config()
        })
        .await
        .unwrap();
        let stream = provider
            .chat_completion_stream(
                ChatRequest {
                    model: "cloudflare/@cf/test/model".into(),
                    ..Default::default()
                },
                RequestContext::default(),
            )
            .await
            .unwrap();
        let chunks: Vec<_> = stream.collect().await;
        assert_eq!(chunks.iter().any(Result::is_err), fails);
        if !fails {
            let first = chunks.first().unwrap().as_ref().unwrap();
            assert_eq!(first.choices[0].delta.content.as_deref(), Some("hello"));
            assert!(first.choices[0].delta.thinking.is_some());
            assert_eq!(
                chunks
                    .last()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .usage
                    .as_ref()
                    .unwrap()
                    .total_tokens,
                3
            );
        }
        assert_eq!(server.await.unwrap()["stream"], true);
    }
    let (base, server) = chat_server(
        "429 Too Many Requests",
        "application/json",
        "rate limited".into(),
    )
    .await;
    let provider = CloudflareProvider::new(CloudflareConfig {
        api_base: Some(base),
        ..create_test_config()
    })
    .await
    .unwrap();
    let result = provider
        .chat_completion_stream(
            ChatRequest {
                model: "test".into(),
                ..Default::default()
            },
            RequestContext::default(),
        )
        .await;
    assert!(matches!(
        result,
        Err(ProviderError::RateLimit {
            retry_after: Some(17),
            ..
        })
    ));
    server.await.unwrap();
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
        truncation: None,
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
        model: "@cf/openai/gpt-oss-120b".to_string(),
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
