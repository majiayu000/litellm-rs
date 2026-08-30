use super::OpenAIProvider;
use super::config::test_openai_config;
use crate::core::providers::base::HttpMethod;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::{ChatMessage, ChatRequest};
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::{EmbeddingInput, EmbeddingRequest};
use crate::core::types::health::HealthStatus;
use crate::core::types::image::ImageGenerationRequest;
use crate::core::types::message::{MessageContent, MessageRole};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn read_full_http_request(socket: &mut TcpStream) -> std::io::Result<()> {
    let mut request_bytes = Vec::new();
    let mut buffer = [0_u8; 1024];

    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(());
        }

        request_bytes.extend_from_slice(&buffer[..bytes_read]);
        if let Some(header_end) = request_bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&request_bytes[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);

            if request_bytes.len() >= header_end + 4 + content_length {
                return Ok(());
            }
        }
    }
}

async fn response_url(status: &str, body: &str) -> std::io::Result<String> {
    response_url_with_declared_length(status, body, body.len()).await
}

async fn truncated_response_url(status: &str, body: &str) -> std::io::Result<String> {
    response_url_with_declared_length(status, body, body.len() + 64).await
}

async fn response_url_with_declared_length(
    status: &str,
    body: &str,
    declared_length: usize,
) -> std::io::Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        declared_length
    );

    tokio::spawn(async move {
        let (mut socket, _) = match listener.accept().await {
            Ok(connection) => connection,
            Err(err) => panic!("test server failed to accept request: {err}"),
        };
        if let Err(err) = read_full_http_request(&mut socket).await {
            panic!("test server failed to read request: {err}");
        }
        if let Err(err) = socket.write_all(response.as_bytes()).await {
            panic!("test server failed to write response: {err}");
        }
    });

    Ok(format!("http://{addr}"))
}

fn openai_chat_stream_request() -> ChatRequest {
    ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[tokio::test]
async fn test_openai_streaming_maps_non_success_status_before_sse()
-> Result<(), Box<dyn std::error::Error>> {
    let body = r#"{"error":{"type":"rate_limit_error","message":"slow down"}}"#;
    let api_base = response_url("429 Too Many Requests", body).await?;

    let config = test_openai_config(api_base, "sk-test123456789012345678901234567890123456");
    let provider = OpenAIProvider::new(config).await?;

    let err = match LLMProvider::chat_completion_stream(
        &provider,
        openai_chat_stream_request(),
        RequestContext::default(),
    )
    .await
    {
        Ok(_) => panic!("streaming response should map upstream status to provider error"),
        Err(err) => err,
    };

    match err {
        ProviderError::ApiError {
            provider,
            status,
            message,
        } => {
            assert_eq!(provider, "openai");
            assert_eq!(status, 429);
            assert!(message.contains("rate_limit_error"));
        }
        other => panic!("expected OpenAI API error envelope, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn test_openai_chat_maps_non_success_status_before_deserialization()
-> Result<(), Box<dyn std::error::Error>> {
    let body = r#"{"error":{"type":"permission_error","message":"project access denied"}}"#;
    let api_base = response_url("403 Forbidden", body).await?;
    let config = test_openai_config(api_base, "sk-test123456789012345678901234567890123456");
    let provider = OpenAIProvider::new(config).await?;

    let error = LLMProvider::chat_completion(
        &provider,
        openai_chat_stream_request(),
        RequestContext::default(),
    )
    .await
    .expect_err("non-streaming OpenAI 403 should be mapped before response parsing");

    assert!(matches!(
        error,
        ProviderError::ApiError {
            status: 403,
            ref message,
            ..
        } if message.contains("project access denied")
    ));
    Ok(())
}

#[tokio::test]
async fn test_openai_chat_preserves_403_when_error_body_is_truncated()
-> Result<(), Box<dyn std::error::Error>> {
    let api_base =
        truncated_response_url("403 Forbidden", r#"{"error":{"message":"cut"}}"#).await?;
    let provider = OpenAIProvider::new(test_openai_config(api_base, "sk-test-truncated")).await?;

    let error = LLMProvider::chat_completion(
        &provider,
        openai_chat_stream_request(),
        RequestContext::default(),
    )
    .await
    .expect_err("truncated OpenAI error body must not erase HTTP 403");

    assert!(matches!(
        error,
        ProviderError::ApiError {
            status: 403,
            ref message,
            ..
        } if message.contains("failed to read upstream error body")
    ));
    Ok(())
}

#[tokio::test]
async fn test_openai_streaming_preserves_403_when_error_body_is_truncated()
-> Result<(), Box<dyn std::error::Error>> {
    let api_base =
        truncated_response_url("403 Forbidden", r#"{"error":{"message":"cut"}}"#).await?;
    let provider = OpenAIProvider::new(test_openai_config(api_base, "sk-test-stream-cut")).await?;

    let error = match LLMProvider::chat_completion_stream(
        &provider,
        openai_chat_stream_request(),
        RequestContext::default(),
    )
    .await
    {
        Ok(_) => panic!("truncated streaming error body must not erase HTTP 403"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        ProviderError::ApiError {
            status: 403,
            ref message,
            ..
        } if message.contains("failed to read upstream error body")
    ));
    Ok(())
}

#[tokio::test]
async fn test_openai_embeddings_map_non_success_status_before_deserialization()
-> Result<(), Box<dyn std::error::Error>> {
    let body = r#"{"error":{"type":"permission_error","message":"embedding access denied"}}"#;
    let api_base = response_url("403 Forbidden", body).await?;
    let provider = OpenAIProvider::new(test_openai_config(api_base, "sk-test-embedding")).await?;
    let request = EmbeddingRequest {
        model: "text-embedding-3-small".to_string(),
        input: EmbeddingInput::Text("hello".to_string()),
        user: None,
        encoding_format: None,
        dimensions: None,
        task_type: None,
        truncation: None,
    };

    let error = LLMProvider::embeddings(&provider, request, RequestContext::default())
        .await
        .expect_err("OpenAI embeddings 403 should be mapped before response parsing");
    assert!(matches!(error, ProviderError::ApiError { status: 403, .. }));
    Ok(())
}

#[tokio::test]
async fn test_openai_image_generation_maps_non_success_status_before_deserialization()
-> Result<(), Box<dyn std::error::Error>> {
    let body = r#"{"error":{"type":"permission_error","message":"image access denied"}}"#;
    let api_base = response_url("403 Forbidden", body).await?;
    let provider = OpenAIProvider::new(test_openai_config(api_base, "sk-test-image")).await?;
    let request = ImageGenerationRequest {
        prompt: "restricted image".to_string(),
        model: Some("gpt-image-1".to_string()),
        n: None,
        size: None,
        quality: None,
        response_format: None,
        style: None,
        user: None,
    };

    let error = LLMProvider::image_generation(&provider, request, RequestContext::default())
        .await
        .expect_err("OpenAI image 403 should be mapped before response parsing");
    assert!(matches!(error, ProviderError::ApiError { status: 403, .. }));
    Ok(())
}

#[tokio::test]
async fn openai_policy_pool_rejects_cross_authority_without_connect()
-> Result<(), Box<dyn std::error::Error>> {
    let api_base = response_url("200 OK", "{}").await?;
    let health_provider = OpenAIProvider::new(test_openai_config(api_base, "sk-health")).await?;
    assert_eq!(
        LLMProvider::health_check(&health_provider).await,
        HealthStatus::Healthy
    );
    let target = TcpListener::bind(("127.0.0.1", 0)).await?;
    let config = test_openai_config("http://127.0.0.1:1", "sk-test-authority");
    let provider = OpenAIProvider::new(config).await?;
    let target_url = format!("http://{}/models", target.local_addr()?);

    let error = provider
        .pool_manager
        .execute_request(&target_url, HttpMethod::GET, Vec::new(), None)
        .await
        .expect_err("cross-authority request must fail closed");
    assert!(error.to_string().contains("authority"));
    let body = serde_json::json!({"stream": true});
    let streaming_error = provider
        .pool_manager
        .execute_streaming_request(&target_url, Vec::new(), body, "openai")
        .await
        .expect_err("streaming cross-authority request must fail closed");
    assert!(streaming_error.to_string().contains("authority"));
    let accepted =
        tokio::time::timeout(std::time::Duration::from_millis(100), target.accept()).await;
    assert!(accepted.is_err());
    let config = test_openai_config("http://169.254.169.254/v1", "sk-test-metadata");
    let error = OpenAIProvider::new(config)
        .await
        .expect_err("metadata endpoints must remain forbidden");
    assert!(error.to_string().contains("private or reserved"));
    Ok(())
}
