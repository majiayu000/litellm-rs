use super::*;
use crate::core::budget::types::{BudgetScope, BudgetStatus};
use crate::core::net::ProviderEndpointPolicy;
use crate::utils::net::http::ProviderHttpClient;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone)]
struct SequenceDnsResolver(Arc<Mutex<VecDeque<SocketAddr>>>);

impl SequenceDnsResolver {
    fn new(answers: impl IntoIterator<Item = SocketAddr>) -> Self {
        Self(Arc::new(Mutex::new(answers.into_iter().collect())))
    }
}

impl Resolve for SequenceDnsResolver {
    fn resolve(&self, _name: Name) -> Resolving {
        let answer = self
            .0
            .lock()
            .expect("resolver lock")
            .pop_front()
            .expect("test resolver answer");
        Box::pin(async move { Ok(Box::new(std::iter::once(answer)) as Addrs) })
    }
}

fn policy_client(
    timeout: Duration,
    answers: impl IntoIterator<Item = SocketAddr>,
) -> ProviderHttpClient {
    ProviderHttpClient::build_with_dns_resolver_for_test(
        ProviderEndpointPolicy::public_only(),
        timeout,
        true,
        Arc::new(SequenceDnsResolver::new(answers)),
    )
    .expect("policy client")
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> io::Result<String> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut buffer))
            .await
            .map_err(|_| io::Error::other("request read timed out"))??;
        request.extend_from_slice(&buffer[..read]);
        let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if read == 0 || request.len() >= end + 4 + content_length {
            return String::from_utf8(request).map_err(io::Error::other);
        }
    }
}

async fn assert_listener_did_not_accept(listener: &tokio::net::TcpListener, context: &str) {
    match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
        Err(_) => {}
        Ok(Ok((_stream, peer))) => panic!("{context}: unexpectedly accepted {peer}"),
        Ok(Err(error)) => panic!("{context}: listener failed: {error}"),
    }
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

fn create_test_budget() -> Budget {
    Budget::new("test-budget", "Test Budget", BudgetScope::Global, 100.0)
}

#[tokio::test]
async fn test_alert_manager_creation() {
    let manager = BudgetAlertManager::new();
    assert!(manager.is_enabled().await);
}

#[tokio::test]
async fn test_create_soft_limit_alert() {
    let manager = BudgetAlertManager::new();
    let mut budget = create_test_budget();
    budget.current_spend = 85.0;

    let result = SpendResult {
        budget_id: budget.id.clone(),
        scope: budget.scope.clone(),
        previous_status: BudgetStatus::Ok,
        new_status: BudgetStatus::Warning,
        current_spend: 85.0,
        max_budget: 100.0,
        remaining: 15.0,
        should_alert_soft_limit: true,
        should_alert_exceeded: false,
    };

    manager.process_spend_result(&result, &budget).await;

    let alerts = manager.get_alerts_for_budget(&budget.id).await;
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, BudgetAlertType::SoftLimitReached);
    assert_eq!(alerts[0].severity, AlertSeverity::Warning);
}

#[tokio::test]
async fn test_create_exceeded_alert() {
    let manager = BudgetAlertManager::new();
    let mut budget = create_test_budget();
    budget.current_spend = 110.0;

    let result = SpendResult {
        budget_id: budget.id.clone(),
        scope: budget.scope.clone(),
        previous_status: BudgetStatus::Warning,
        new_status: BudgetStatus::Exceeded,
        current_spend: 110.0,
        max_budget: 100.0,
        remaining: 0.0,
        should_alert_soft_limit: false,
        should_alert_exceeded: true,
    };

    manager.process_spend_result(&result, &budget).await;

    let alerts = manager.get_alerts_for_budget(&budget.id).await;
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, BudgetAlertType::BudgetExceeded);
    assert_eq!(alerts[0].severity, AlertSeverity::Critical);
}

#[tokio::test]
async fn test_create_reset_alert() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    manager.create_reset_alert(&budget).await;

    let alerts = manager.get_alerts_for_budget(&budget.id).await;
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, BudgetAlertType::BudgetReset);
    assert_eq!(alerts[0].severity, AlertSeverity::Info);
}

#[tokio::test]
async fn test_acknowledge_alert() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    manager.create_reset_alert(&budget).await;

    let alerts = manager.get_unacknowledged_alerts().await;
    assert_eq!(alerts.len(), 1);

    let alert_id = &alerts[0].id;
    assert!(manager.acknowledge_alert(alert_id).await);

    let unacked = manager.get_unacknowledged_alerts().await;
    assert_eq!(unacked.len(), 0);
}

#[tokio::test]
async fn test_acknowledge_alerts_for_budget() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    // Create multiple alerts
    manager.create_reset_alert(&budget).await;

    let mut budget2 = budget.clone();
    budget2.current_spend = 85.0;
    let result = SpendResult {
        budget_id: budget.id.clone(),
        scope: budget.scope.clone(),
        previous_status: BudgetStatus::Ok,
        new_status: BudgetStatus::Warning,
        current_spend: 85.0,
        max_budget: 100.0,
        remaining: 15.0,
        should_alert_soft_limit: true,
        should_alert_exceeded: false,
    };
    manager.process_spend_result(&result, &budget2).await;

    let unacked_before = manager.get_unacknowledged_alerts().await;
    assert_eq!(unacked_before.len(), 2);

    let count = manager.acknowledge_alerts_for_budget(&budget.id).await;
    assert_eq!(count, 2);

    let unacked_after = manager.get_unacknowledged_alerts().await;
    assert_eq!(unacked_after.len(), 0);
}

#[tokio::test]
async fn test_get_alerts_by_severity() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    // Create a reset alert (Info)
    manager.create_reset_alert(&budget).await;

    // Create a soft limit alert (Warning)
    let mut budget2 = budget.clone();
    budget2.current_spend = 85.0;
    let result = SpendResult {
        budget_id: budget.id.clone(),
        scope: budget.scope.clone(),
        previous_status: BudgetStatus::Ok,
        new_status: BudgetStatus::Warning,
        current_spend: 85.0,
        max_budget: 100.0,
        remaining: 15.0,
        should_alert_soft_limit: true,
        should_alert_exceeded: false,
    };
    manager.process_spend_result(&result, &budget2).await;

    let info_alerts = manager.get_alerts_by_severity(AlertSeverity::Info).await;
    assert_eq!(info_alerts.len(), 1);

    let warning_alerts = manager.get_alerts_by_severity(AlertSeverity::Warning).await;
    assert_eq!(warning_alerts.len(), 1);

    let critical_alerts = manager
        .get_alerts_by_severity(AlertSeverity::Critical)
        .await;
    assert_eq!(critical_alerts.len(), 0);
}

#[tokio::test]
async fn test_get_alert_stats() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    manager.create_reset_alert(&budget).await;

    let stats = manager.get_alert_stats().await;

    assert_eq!(stats.total_alerts, 1);
    assert_eq!(stats.unacknowledged, 1);
    assert_eq!(stats.info_count, 1);
    assert_eq!(stats.reset_alerts, 1);
}

#[tokio::test]
async fn test_clear_alerts() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    manager.create_reset_alert(&budget).await;
    assert_eq!(manager.get_all_alerts().await.len(), 1);

    manager.clear_alerts().await;
    assert_eq!(manager.get_all_alerts().await.len(), 0);
}

#[tokio::test]
async fn test_clear_acknowledged_alerts() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    // Create two alerts
    manager.create_reset_alert(&budget).await;

    let mut budget2 = budget.clone();
    budget2.current_spend = 85.0;
    let result = SpendResult {
        budget_id: budget.id.clone(),
        scope: budget.scope.clone(),
        previous_status: BudgetStatus::Ok,
        new_status: BudgetStatus::Warning,
        current_spend: 85.0,
        max_budget: 100.0,
        remaining: 15.0,
        should_alert_soft_limit: true,
        should_alert_exceeded: false,
    };
    manager.process_spend_result(&result, &budget2).await;

    // Acknowledge one
    let alerts = manager.get_all_alerts().await;
    manager.acknowledge_alert(&alerts[0].id).await;

    // Clear acknowledged
    let cleared = manager.clear_acknowledged_alerts().await;
    assert_eq!(cleared, 1);

    // Should have 1 remaining
    assert_eq!(manager.get_all_alerts().await.len(), 1);
}

#[tokio::test]
async fn test_add_webhook() {
    let manager = BudgetAlertManager::new();

    let webhook = WebhookConfig {
        url: "https://example.com/webhook".to_string(),
        ..Default::default()
    };

    manager.add_webhook(webhook).await.unwrap();

    // Webhook is added (internal state)
    let webhooks = manager.webhooks.read().await;
    assert_eq!(webhooks.len(), 1);
}

#[tokio::test]
async fn add_webhook_rejects_complete_unsafe_url_table() {
    let manager = BudgetAlertManager::new();
    for url in "\n \nnot a url\nfile:///tmp/hook\nftp://example.com/hook\nhttp://admission-user:admission-password@127.0.0.1/hook?token=admission-secret\nhttp://10.0.0.1/hook\nhttp://169.254.169.254/latest/meta-data\nhttp://localhost/hook\nhttp://foo.localhost/hook\nhttp://internal/hook\nhttp://foo.internal/hook\nhttp://local/hook\nhttp://foo.local/hook\nhttp://metadata/hook\nhttp://metadata.google.internal/hook\nhttp://metadata.goog/hook\nhttp://[::1]/hook\nhttp://[fd00::1]/hook\nhttp://[fe80::1]/hook\nhttp://[::ffff:169.254.169.254]/hook\nhttp://[64:ff9b::a9fe:a9fe]/hook"
    .lines()
    {
        let error = manager
            .add_webhook(WebhookConfig {
                url: url.to_string(),
                ..Default::default()
            })
            .await
            .expect_err("unsafe URL must fail closed");
        assert_eq!(error, BudgetWebhookError::InvalidUrl, "{url}");
        assert!(!error.to_string().contains("admission-secret"), "{error}");
    }
    assert!(manager.webhooks.read().await.is_empty());
}

#[tokio::test]
async fn legal_delivery_preserves_payload_headers_timeout_and_retries() -> TestResult {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for attempt in 0..2 {
            let (mut stream, _) = listener.accept().await?;
            requests.push(read_http_request(&mut stream).await?);
            if attempt == 0 {
                tokio::time::sleep(Duration::from_millis(1200)).await;
            } else {
                stream
                    .write_all(
                        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await?;
            }
        }
        Ok::<_, io::Error>(requests)
    });
    let manager = BudgetAlertManager::new();
    let mut headers = HashMap::new();
    headers.insert("X-Budget-Hook".to_string(), "preserved".to_string());
    manager
        .add_webhook_with_client_for_test(
            WebhookConfig {
                url: format!("http://budget.test:{}/hook", address.port()),
                headers,
                severities: vec![AlertSeverity::Info],
                timeout_secs: 1,
                max_retries: 2,
                ..Default::default()
            },
            policy_client(Duration::from_secs(1), [address, address]),
        )
        .await?;
    let started = Instant::now();
    manager.create_reset_alert(&create_test_budget()).await;
    assert!(started.elapsed() < Duration::from_millis(1800));
    let requests = server.await??;
    assert_eq!(requests.len(), 2, "existing retry count must be preserved");
    for request in requests {
        let request = request.to_ascii_lowercase();
        for expected in [
            "x-budget-hook: preserved",
            "\"type\":\"budget_alert\"",
            "\"budget_id\":\"test-budget\"",
        ] {
            assert!(request.contains(expected), "{request}");
        }
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn redirect_is_not_followed_and_logs_are_secret_safe() -> TestResult {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(CapturedLogs(bytes.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let source = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let source_address = source.local_addr()?;
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let target_address = target.local_addr()?;
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
    let manager = BudgetAlertManager::new();
    manager
        .add_webhook_with_client_for_test(
            WebhookConfig {
                url: format!(
                    "http://source-user:source-password@source.test:{}/hook?token=source-secret",
                    source_address.port()
                ),
                severities: vec![AlertSeverity::Info],
                max_retries: 1,
                ..Default::default()
            },
            policy_client(Duration::from_secs(1), [source_address]),
        )
        .await?;
    manager.create_reset_alert(&create_test_budget()).await;
    server.await??;
    assert_listener_did_not_accept(&target, "redirect target").await;
    let logs = String::from_utf8(bytes.lock().expect("log lock").clone())?;
    assert!(logs.contains("error status"), "{logs}");
    for secret in ["source-user", "source-password", "source-secret", "token="] {
        assert!(!logs.contains(secret), "{logs}");
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn retry_new_connection_rebind_is_rejected_without_accept() -> TestResult {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(CapturedLogs(bytes.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    for blocked_ip in ["127.0.0.1", "10.0.0.1", "169.254.169.254", "fd00::1"] {
        let tripwire = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let tripwire_address = tripwire.local_addr()?;
        let blocked = SocketAddr::new(blocked_ip.parse()?, tripwire_address.port());
        let client = ProviderHttpClient::build_public_then_private_tripwire_for_test(
            blocked,
            tripwire_address,
        )
        .await?;
        let url = format!(
            "http://rebind-user:rebind-password@rebind.test:{}/hook?token=rebind-secret",
            tripwire_address.port()
        );
        let probe = client.post(&url)?.send().await.expect_err("private rebind");
        assert!(ProviderHttpClient::request_error_is_endpoint_policy(&probe));
        assert_listener_did_not_accept(&tripwire, "initial private rebind").await;
        let server = tokio::spawn(async move {
            let (mut stream, _) = tripwire.accept().await?;
            let _request = read_http_request(&mut stream).await?;
            stream
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await?;
            Ok::<_, io::Error>(tripwire)
        });
        let manager = BudgetAlertManager::new();
        manager
            .add_webhook_with_client_for_test(
                WebhookConfig {
                    url,
                    severities: vec![AlertSeverity::Info],
                    timeout_secs: 1,
                    max_retries: 2,
                    ..Default::default()
                },
                client,
            )
            .await?;
        manager.create_reset_alert(&create_test_budget()).await;
        assert_listener_did_not_accept(&server.await??, "retry private rebind").await;
    }
    let logs = String::from_utf8(bytes.lock().expect("log lock").clone())?;
    assert!(logs.contains("outbound endpoint policy"), "{logs}");
    for secret in ["rebind-user", "rebind-password", "rebind-secret", "token="] {
        assert!(!logs.contains(secret), "{logs}");
    }
    Ok(())
}

#[tokio::test]
async fn test_config_management() {
    let manager = BudgetAlertManager::new();

    let config = manager.get_config().await;
    assert!(config.enabled);

    manager.set_enabled(false).await;
    assert!(!manager.is_enabled().await);

    let new_config = AlertConfig {
        enabled: true,
        soft_limit_percentage: 0.9,
        warning_thresholds: vec![0.95],
        max_history_size: 500,
        duplicate_suppression_secs: 1800,
    };

    manager.update_config(new_config).await;

    let updated_config = manager.get_config().await;
    assert_eq!(updated_config.soft_limit_percentage, 0.9);
    assert_eq!(updated_config.max_history_size, 500);
}

#[tokio::test]
async fn test_alert_history() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    // Create multiple alerts
    for _ in 0..5 {
        manager.create_reset_alert(&budget).await;
    }

    let history = manager.get_alert_history(Some(3)).await;
    assert_eq!(history.len(), 3);

    let full_history = manager.get_alert_history(None).await;
    assert_eq!(full_history.len(), 5);
}

#[tokio::test]
async fn test_disabled_alerting() {
    let manager = BudgetAlertManager::new();
    manager.set_enabled(false).await;

    let budget = create_test_budget();
    manager.create_reset_alert(&budget).await;

    // No alerts should be created when disabled
    let alerts = manager.get_all_alerts().await;
    assert_eq!(alerts.len(), 0);
}
