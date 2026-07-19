//! Webhook tests
//!
//! This module contains unit tests for webhook functionality.

#[cfg(test)]
use super::manager::WebhookManager;
use super::types::{WebhookConfig, WebhookEventType, WebhookPayload};
use crate::core::net::ProviderEndpointPolicy;
use crate::utils::net::http::ProviderHttpClient;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::collections::{HashMap, VecDeque};
use std::io::{self, Write};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
struct SequenceDnsResolver {
    answers: Arc<Mutex<VecDeque<SocketAddr>>>,
}

impl SequenceDnsResolver {
    fn new(answers: impl IntoIterator<Item = SocketAddr>) -> Self {
        Self {
            answers: Arc::new(Mutex::new(answers.into_iter().collect())),
        }
    }
}

impl Resolve for SequenceDnsResolver {
    fn resolve(&self, _name: Name) -> Resolving {
        let answer = self
            .answers
            .lock()
            .expect("answers lock")
            .pop_front()
            .expect("test resolver answer");
        Box::pin(async move { Ok(Box::new(std::iter::once(answer)) as Addrs) })
    }
}

fn manager_with_resolver(resolver: Arc<SequenceDnsResolver>) -> WebhookManager {
    let client = ProviderHttpClient::build_with_dns_resolver_for_test(
        ProviderEndpointPolicy::public_only(),
        Duration::from_secs(30),
        true,
        resolver,
    )
    .expect("policy client");
    WebhookManager::with_client_for_test(client)
}

async fn assert_listener_did_not_accept(listener: &tokio::net::TcpListener, context: &str) {
    match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
        Err(_) => {}
        Ok(Ok((_stream, peer))) => panic!("{context}: unexpectedly accepted {peer}"),
        Ok(Err(error)) => panic!("{context}: listener failed: {error}"),
    }
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> io::Result<String> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut buffer))
            .await
            .map_err(|_| io::Error::other("request read timed out"))??;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            break;
        }
    }
    String::from_utf8(request).map_err(io::Error::other)
}

#[derive(Clone)]
struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedLogs {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl Write for CapturedLogs {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("log lock").extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_webhook_manager_creation() {
    let manager = WebhookManager::new().unwrap();
    let webhooks = manager.list_webhooks().await;
    assert!(webhooks.is_empty());
}

#[tokio::test]
async fn test_webhook_registration() {
    let manager = WebhookManager::new().unwrap();

    let config = WebhookConfig {
        url: "https://example.com/webhook".to_string(),
        events: vec![WebhookEventType::RequestCompleted],
        ..Default::default()
    };

    manager
        .register_webhook("test".to_string(), config)
        .await
        .unwrap();

    let webhooks = manager.list_webhooks().await;
    assert_eq!(webhooks.len(), 1);
    assert!(webhooks.contains_key("test"));
}

#[tokio::test]
async fn webhook_registration_rejects_private_and_reserved_targets() {
    let manager = WebhookManager::new().unwrap();

    for url in [
        "",
        " ",
        "not a url",
        "file:///tmp/hook",
        "ftp://example.com/hook",
        "http://127.0.0.1/webhook",
        "http://10.0.0.1/webhook",
        "http://169.254.169.254/latest/meta-data",
        "http://localhost/webhook",
        "http://foo.localhost/webhook",
        "http://internal/webhook",
        "http://foo.internal/webhook",
        "http://local/webhook",
        "http://foo.local/webhook",
        "http://metadata/webhook",
        "http://metadata.google.internal/webhook",
        "http://metadata.goog/webhook",
        "http://[::1]/webhook",
        "http://[fd00::1]/webhook",
        "http://[fe80::1]/webhook",
        "http://[::ffff:169.254.169.254]/webhook",
        "http://[64:ff9b::a9fe:a9fe]/webhook",
    ] {
        let config = WebhookConfig {
            url: url.to_string(),
            events: vec![WebhookEventType::RequestCompleted],
            ..Default::default()
        };
        assert!(
            manager
                .register_webhook("blocked".to_string(), config)
                .await
                .is_err(),
            "unsafe webhook URL was accepted: {url}"
        );
    }
}

#[tokio::test]
async fn legal_delivery_preserves_payload_headers_signature_and_stats() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let resolver = Arc::new(SequenceDnsResolver::new([address]));
    let manager = manager_with_resolver(resolver.clone());
    let (request_tx, request_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let request = read_http_request(&mut stream).await?;
        request_tx
            .send(request)
            .map_err(|_| io::Error::other("request receiver dropped"))?;
        stream
            .write_all(
                b"HTTP/1.1 202 Accepted\r\nContent-Length: 100\r\nConnection: close\r\n\r\nx",
            )
            .await
    });
    let mut headers = HashMap::new();
    headers.insert("X-Custom".to_string(), "expected".to_string());
    manager
        .register_webhook(
            "legal".to_string(),
            WebhookConfig {
                url: format!("http://webhook.test:{}/hook", address.port()),
                events: vec![WebhookEventType::RequestCompleted],
                headers,
                secret: Some("signing-secret".to_string()),
                max_retries: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    manager
        .send_event(
            WebhookEventType::RequestCompleted,
            serde_json::json!({"model": "gpt-test"}),
            None,
        )
        .await
        .unwrap();
    let delivery = manager.get_delivery_history(Some(1)).await.remove(0);
    let expected_signature = manager
        .generate_signature(&delivery.payload, "signing-secret")
        .unwrap();
    manager.process_delivery_queue().await.unwrap();
    server.await.unwrap().unwrap();
    let request = request_rx.await.unwrap().to_ascii_lowercase();
    assert!(request.contains("x-custom: expected"));
    assert!(request.contains(&format!("x-webhook-signature: {}", expected_signature)));
    assert!(request.contains("\"model\":\"gpt-test\""));
    let stats = manager.get_stats().await;
    assert_eq!(stats.successful_deliveries, 1);
    assert_eq!(stats.failed_deliveries, 0);
}

#[tokio::test]
async fn redirect_is_not_followed_and_status_is_recorded() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(CapturedLogs(bytes.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let source = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source_address = source.local_addr().unwrap();
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_address = target.local_addr().unwrap();
    let resolver = Arc::new(SequenceDnsResolver::new([source_address]));
    let manager = manager_with_resolver(resolver.clone());
    let server = tokio::spawn(async move {
        let (mut stream, _) = source.accept().await?;
        let _request = read_http_request(&mut stream).await?;
        stream
            .write_all(
                format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://redirect.test:{}/private?token=redirect-secret\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    target_address.port()
                )
                .as_bytes(),
            )
            .await
    });
    manager
        .register_webhook(
            "redirect".to_string(),
            WebhookConfig {
                url: format!(
                    "http://source-user:source-password@source.test:{}/hook?token=source-secret",
                    source_address.port()
                ),
                events: vec![WebhookEventType::RequestFailed],
                max_retries: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    manager
        .send_event(WebhookEventType::RequestFailed, serde_json::json!({}), None)
        .await
        .unwrap();
    manager.process_delivery_queue().await.unwrap();
    server.await.unwrap().unwrap();
    assert_listener_did_not_accept(&target, "redirect target").await;
    let delivery = manager.get_delivery_history(Some(1)).await.remove(0);
    assert_eq!(delivery.response_status, Some(302));
    assert_eq!(delivery.response_body.as_deref(), Some(""));
    assert_eq!(manager.get_stats().await.failed_deliveries, 1);
    let logs = String::from_utf8(bytes.lock().expect("log lock").clone()).unwrap();
    assert!(logs.contains("failed permanently"), "{logs}");
    for secret in ["source-user", "source-password", "source-secret", "token="] {
        assert!(!logs.contains(secret), "{logs}");
    }
}

#[tokio::test]
async fn retries_revalidate_rebinding_and_never_reach_tripwire() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(CapturedLogs(bytes.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    for blocked_ip in ["127.0.0.1", "10.0.0.1", "169.254.169.254", "fd00::1"] {
        let tripwire = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tripwire_address = tripwire.local_addr().unwrap();
        let blocked_address = SocketAddr::new(blocked_ip.parse().unwrap(), tripwire_address.port());
        let client = ProviderHttpClient::build_public_then_private_tripwire_for_test(
            blocked_address,
            tripwire_address,
        )
        .await
        .unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = tripwire.accept().await?;
            let _request = read_http_request(&mut stream).await?;
            stream
                .write_all(b"HTTP/1.1 503 X\r\nContent-Length: 1\r\nConnection: close\r\n\r\nx")
                .await?;
            Ok::<_, io::Error>(tripwire)
        });
        let manager = WebhookManager::with_client_for_test(client);
        let secret_url = format!(
            "http://rebind-user:rebind-password@rebind.test:{}/hook?token=rebind-secret",
            tripwire_address.port()
        );
        manager
            .register_webhook(
                "retry".to_string(),
                WebhookConfig {
                    url: secret_url,
                    events: vec![WebhookEventType::RequestFailed],
                    max_retries: 2,
                    retry_delay_seconds: 0,
                    timeout_seconds: 1,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        manager
            .send_event(WebhookEventType::RequestFailed, serde_json::json!({}), None)
            .await
            .unwrap();
        let mut probe = manager.get_delivery_history(Some(1)).await.remove(0);
        let config = manager.list_webhooks().await["retry"].clone();
        let error = manager
            .deliver_webhook_internal(&mut probe, &config)
            .await
            .expect_err("private rebind must be rejected during delivery");
        for secret in ["rebind-user", "rebind-password", "rebind-secret", "token="] {
            assert!(!error.to_string().contains(secret), "{error}");
        }
        manager.process_delivery_queue().await.unwrap();
        let tripwire = server.await.unwrap().unwrap();
        let retrying = manager.get_delivery_history(Some(1)).await.remove(0);
        assert_eq!(
            retrying.status,
            super::types::WebhookDeliveryStatus::Retrying
        );
        assert_eq!(retrying.attempts, 1);
        assert_eq!(retrying.response_status, Some(503));
        assert_eq!(retrying.response_body.as_deref(), Some("x"));
        assert_eq!(manager.get_stats().await.failed_deliveries, 0);
        manager.process_delivery_queue().await.unwrap();
        assert_listener_did_not_accept(&tripwire, "rebinding retry target").await;
        let delivery = manager.get_delivery_history(Some(1)).await.remove(0);
        assert_eq!(delivery.status, super::types::WebhookDeliveryStatus::Failed);
        assert_eq!(delivery.attempts, 2);
        assert_eq!(delivery.response_status, None);
        assert_eq!(delivery.response_body, None);
        assert_eq!(manager.get_stats().await.failed_deliveries, 1);
    }
    let logs = String::from_utf8(bytes.lock().expect("log lock").clone()).unwrap();
    assert!(logs.contains("failed permanently"), "{logs}");
    for secret in ["rebind-user", "rebind-password", "rebind-secret", "token="] {
        assert!(!logs.contains(secret), "{logs}");
    }
}

#[test]
fn test_webhook_event_types() {
    let event = WebhookEventType::RequestStarted;
    assert_eq!(event, WebhookEventType::RequestStarted);

    let custom_event = WebhookEventType::Custom("my_event".to_string());
    assert_eq!(
        custom_event,
        WebhookEventType::Custom("my_event".to_string())
    );
}

#[test]
fn test_webhook_payload_serialization() {
    let payload = WebhookPayload {
        event_type: WebhookEventType::RequestCompleted,
        timestamp: chrono::Utc::now(),
        request_context: None,
        data: serde_json::json!({"test": "data"}),
        metadata: HashMap::new(),
    };

    let serialized = serde_json::to_string(&payload).unwrap();
    assert!(serialized.contains("RequestCompleted"));
    assert!(serialized.contains("test"));
}
