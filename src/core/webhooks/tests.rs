//! Webhook tests
//!
//! This module contains unit tests for webhook functionality.

#[cfg(test)]
use super::manager::WebhookManager;
use super::types::{WebhookConfig, WebhookEventType, WebhookPayload};
use crate::core::net::{
    ProviderEndpointAccess, ProviderEndpointPolicy, is_provider_endpoint_ip_allowed,
};
use crate::utils::net::http::ProviderHttpClient;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::collections::{HashMap, VecDeque};
use std::io::{self, Write};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug)]
enum DnsAnswer {
    Allow(SocketAddr),
    Reject(IpAddr),
}

#[derive(Clone)]
struct SequenceDnsResolver {
    answers: Arc<Mutex<VecDeque<DnsAnswer>>>,
    queries: Arc<Mutex<Vec<String>>>,
}

impl SequenceDnsResolver {
    fn new(answers: impl IntoIterator<Item = DnsAnswer>) -> Self {
        Self {
            answers: Arc::new(Mutex::new(answers.into_iter().collect())),
            queries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn queries(&self) -> Vec<String> {
        self.queries.lock().expect("queries lock").clone()
    }
}

impl Resolve for SequenceDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        self.queries
            .lock()
            .expect("queries lock")
            .push(name.as_str().to_string());
        let answer = self
            .answers
            .lock()
            .expect("answers lock")
            .pop_front()
            .expect("test resolver answer");
        Box::pin(async move {
            match answer {
                DnsAnswer::Allow(address) => Ok(Box::new(std::iter::once(address)) as Addrs),
                DnsAnswer::Reject(ip) => {
                    assert!(!is_provider_endpoint_ip_allowed(
                        ProviderEndpointAccess::PublicOnly,
                        &ip
                    ));
                    Err(Box::new(io::Error::other(
                        "Host resolves to a disallowed address (SSRF protection)",
                    ))
                        as Box<dyn std::error::Error + Send + Sync>)
                }
            }
        })
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

async fn write_http_response(
    stream: &mut tokio::net::TcpStream,
    status: &str,
    headers: &str,
    body: &str,
) -> io::Result<()> {
    stream
        .write_all(
            format!(
                "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await
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
async fn registration_errors_and_logs_redact_url_secrets() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(CapturedLogs(bytes.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let manager = WebhookManager::new().unwrap();
    let secret_url = "http://admin:password@127.0.0.1/hook?token=query-secret";
    let error = manager
        .register_webhook(
            "redacted".to_string(),
            WebhookConfig {
                url: secret_url.to_string(),
                ..Default::default()
            },
        )
        .await
        .expect_err("private URL with secrets must be rejected");
    let logs = String::from_utf8(bytes.lock().expect("log lock").clone()).unwrap();
    for secret in ["admin", "password", "query-secret", "token="] {
        assert!(!error.to_string().contains(secret), "{error}");
        assert!(!logs.contains(secret), "{logs}");
    }
}

#[tokio::test]
async fn legal_delivery_preserves_payload_headers_signature_and_stats() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let resolver = Arc::new(SequenceDnsResolver::new([DnsAnswer::Allow(address)]));
    let manager = manager_with_resolver(resolver.clone());
    let (request_tx, request_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let request = read_http_request(&mut stream).await?;
        request_tx
            .send(request)
            .map_err(|_| io::Error::other("request receiver dropped"))?;
        write_http_response(&mut stream, "202 Accepted", "", "accepted").await
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
    assert_eq!(resolver.queries(), ["webhook.test"]);
    let stats = manager.get_stats().await;
    assert_eq!(stats.successful_deliveries, 1);
    assert_eq!(stats.failed_deliveries, 0);
}

#[tokio::test]
async fn redirect_is_not_followed_and_status_is_recorded() {
    let source = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source_address = source.local_addr().unwrap();
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_address = target.local_addr().unwrap();
    let resolver = Arc::new(SequenceDnsResolver::new([DnsAnswer::Allow(source_address)]));
    let manager = manager_with_resolver(resolver.clone());
    let server = tokio::spawn(async move {
        let (mut stream, _) = source.accept().await?;
        let _request = read_http_request(&mut stream).await?;
        write_http_response(
            &mut stream,
            "302 Found",
            &format!(
                "Location: http://redirect.test:{}/private?token=redirect-secret\r\n",
                target_address.port()
            ),
            "",
        )
        .await
    });
    manager
        .register_webhook(
            "redirect".to_string(),
            WebhookConfig {
                url: format!("http://source.test:{}/hook", source_address.port()),
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
    assert_eq!(resolver.queries(), ["source.test"]);
    let delivery = manager.get_delivery_history(Some(1)).await.remove(0);
    assert_eq!(delivery.response_status, Some(302));
    let stats = manager.get_stats().await;
    assert_eq!(stats.failed_deliveries, 1);
}

#[tokio::test]
async fn retries_revalidate_rebinding_and_never_reach_tripwire() {
    for blocked_ip in ["127.0.0.1", "10.0.0.1", "169.254.169.254", "fd00::1"] {
        let source = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let source_address = source.local_addr().unwrap();
        let tripwire = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let resolver = Arc::new(SequenceDnsResolver::new([
            DnsAnswer::Allow(source_address),
            DnsAnswer::Reject(blocked_ip.parse().unwrap()),
        ]));
        let manager = manager_with_resolver(resolver.clone());
        let server = tokio::spawn(async move {
            let (mut stream, _) = source.accept().await?;
            let _request = read_http_request(&mut stream).await?;
            write_http_response(&mut stream, "503 Unavailable", "", "retry").await
        });
        manager
            .register_webhook(
                "retry".to_string(),
                WebhookConfig {
                    url: format!("http://rebind.test:{}/hook", source_address.port()),
                    events: vec![WebhookEventType::RequestFailed],
                    max_retries: 2,
                    retry_delay_seconds: 0,
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
        manager.process_delivery_queue().await.unwrap();
        assert_listener_did_not_accept(&tripwire, "rebinding retry target").await;
        assert_eq!(resolver.queries(), ["rebind.test", "rebind.test"]);
        let delivery = manager.get_delivery_history(Some(1)).await.remove(0);
        assert_eq!(delivery.status, super::types::WebhookDeliveryStatus::Failed);
        assert_eq!(manager.get_stats().await.failed_deliveries, 1);
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
