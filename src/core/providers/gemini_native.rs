use reqwest::{Response, header::RETRY_AFTER};

use super::ProviderError;
use super::base::read_streaming_error_body;
use super::shared::parse_retry_after_from_body;

pub(crate) async fn gemini_response_or_provider_error(
    response: Response,
    api_key: &str,
) -> Result<Response, ProviderError> {
    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let body = read_streaming_error_body(response)
        .await
        .map_err(|error| error.into_provider_error("gemini_proxy"))?;
    let body = sanitize_gemini_error_body(body, api_key);
    Err(gemini_upstream_status_provider_error(
        status,
        body,
        retry_after,
    ))
}

fn gemini_upstream_status_provider_error(
    status: u16,
    body: String,
    retry_after: Option<u64>,
) -> ProviderError {
    let message = if body.trim().is_empty() {
        format!("Gemini upstream returned HTTP {status}")
    } else {
        format!("Gemini upstream returned HTTP {status}: {body}")
    };
    if status == 429 {
        ProviderError::rate_limit_with_retry(
            "gemini_proxy",
            message,
            retry_after.or_else(|| parse_retry_after_from_body(&body)),
        )
    } else {
        ProviderError::api_error("gemini_proxy", status, message)
    }
}

fn sanitize_gemini_error_body(body: String, api_key: &str) -> String {
    if api_key.is_empty() || body.is_empty() {
        return body;
    }

    let encoded_key: String = url::form_urlencoded::byte_serialize(api_key.as_bytes()).collect();
    if !body.contains(api_key) && !body.contains(&encoded_key) {
        return body;
    }

    let mut sanitized = body.replace(api_key, "[REDACTED]");
    if encoded_key != api_key {
        sanitized = sanitized.replace(&encoded_key, "[REDACTED]");
    }
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::base::STREAMING_ERROR_BODY_MAX_BYTES;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn raw_error_response(
        body: Vec<u8>,
        content_length: usize,
        stall: bool,
    ) -> (reqwest::Response, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("error server should bind");
        let address = listener.local_addr().expect("error server address");
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("request should connect");
            let mut request = [0_u8; 4096];
            let request_bytes = socket
                .read(&mut request)
                .await
                .expect("request should be readable");
            assert!(request_bytes > 0, "request should not be empty");
            let headers = format!(
                "HTTP/1.1 500 Internal Server Error\r\ncontent-length: {content_length}\r\nconnection: close\r\n\r\n"
            );
            socket
                .write_all(headers.as_bytes())
                .await
                .expect("headers should write");
            socket
                .write_all(&body)
                .await
                .expect("error body should write");
            if stall {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
        let response = reqwest::Client::new()
            .get(format!("http://{address}"))
            .send()
            .await
            .expect("response headers should arrive");
        (response, task)
    }

    #[tokio::test]
    async fn streaming_error_body_stall_times_out() {
        let (response, task) = raw_error_response(b"partial".to_vec(), 4096, true).await;
        let error = tokio::time::timeout(
            Duration::from_secs(12),
            gemini_response_or_provider_error(response, ""),
        )
        .await
        .expect("bounded error reader should return")
        .expect_err("stalled error body must fail");

        assert!(matches!(error, ProviderError::Timeout { .. }));
        task.abort();
    }

    #[tokio::test]
    async fn streaming_error_body_is_capped() {
        let mut body = vec![b'a'; STREAMING_ERROR_BODY_MAX_BYTES];
        body.extend_from_slice(b"TAIL_MARKER");
        let (response, task) = raw_error_response(body.clone(), body.len(), false).await;
        let error = gemini_response_or_provider_error(response, "")
            .await
            .expect_err("upstream error must fail");
        let message = error.to_string();

        assert!(!message.contains("TAIL_MARKER"));
        assert!(message.len() < STREAMING_ERROR_BODY_MAX_BYTES + 256);
        task.await.expect("error server should finish");
    }

    #[test]
    fn error_body_redacts_raw_and_form_encoded_keys() {
        let key = "secret/key+value";
        let encoded: String = url::form_urlencoded::byte_serialize(key.as_bytes()).collect();
        let body = format!("raw={key}&encoded={encoded}");
        let sanitized = sanitize_gemini_error_body(body, key);

        assert_eq!(sanitized, "raw=[REDACTED]&encoded=[REDACTED]");
        assert!(!sanitized.contains(key));
        assert!(!sanitized.contains(&encoded));
    }
}
