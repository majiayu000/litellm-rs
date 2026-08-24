use super::*;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::anthropic::config::AnthropicConfig;
use crate::core::types::message::MessageContent;
use crate::core::types::thinking::ThinkingContent;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

mod message_tool_tests;
mod request_edge_tests;
mod response_tests;
mod setup_error_tests;

async fn read_full_http_request(socket: &mut TcpStream) -> std::io::Result<()> {
    let mut request_bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(());
        }
        request_bytes.extend_from_slice(&buffer[..bytes_read]);
        if let Some(header_end) = request_bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
        {
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

async fn forbidden_anthropic_url(truncated: bool) -> std::io::Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    let body = r#"{"type":"error","error":{"type":"permission_error","message":"workspace access denied"}}"#;
    let declared_length = body.len() + if truncated { 64 } else { 0 };
    let response = format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        declared_length
    );
    tokio::spawn(async move {
        let (mut socket, _) = listener
            .accept()
            .await
            .expect("Anthropic test server should accept request");
        read_full_http_request(&mut socket)
            .await
            .expect("Anthropic test server should read request");
        socket
            .write_all(response.as_bytes())
            .await
            .expect("Anthropic test server should write response");
    });
    Ok(format!("http://{address}"))
}

fn anthropic_request() -> ChatRequest {
    ChatRequest::new("claude-3-haiku-20240307").add_user_message("hello")
}

async fn forbidden_client(truncated: bool) -> Result<AnthropicClient, Box<dyn std::error::Error>> {
    let config = AnthropicConfig::new_test("test-key")
        .with_base_url(forbidden_anthropic_url(truncated).await?)
        .with_endpoint_access(ProviderEndpointAccess::PrivateNetwork);
    Ok(AnthropicClient::new(config)?)
}

fn assert_permission_error(error: ProviderError) {
    assert!(matches!(
        error,
        ProviderError::ApiError {
            status: 403,
            ref message,
            ..
        } if message == "workspace access denied"
    ));
}

#[tokio::test]
async fn chat_preserves_upstream_403() -> Result<(), Box<dyn std::error::Error>> {
    let error = forbidden_client(false)
        .await?
        .chat(anthropic_request())
        .await
        .expect_err("Anthropic chat 403 should be a permission error");
    assert_permission_error(error);
    Ok(())
}

#[tokio::test]
async fn chat_stream_preserves_upstream_403() -> Result<(), Box<dyn std::error::Error>> {
    let error = forbidden_client(false)
        .await?
        .chat_stream(anthropic_request())
        .await
        .expect_err("Anthropic streaming 403 should be a permission error");
    assert_permission_error(error);
    Ok(())
}

#[tokio::test]
async fn chat_preserves_403_when_error_body_is_truncated() -> Result<(), Box<dyn std::error::Error>>
{
    let error = forbidden_client(true)
        .await?
        .chat(anthropic_request())
        .await
        .expect_err("truncated Anthropic error body must not erase HTTP 403");
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
