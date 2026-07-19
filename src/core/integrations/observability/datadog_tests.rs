use super::*;

async fn response_server(
    responses: Vec<(&'static str, u16)>,
) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut paths = Vec::new();
        for _ in 0..responses.len() {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0_u8; 1024];
                let count = socket.read(&mut chunk).await.unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8(request).unwrap();
            let path = request.split_whitespace().nth(1).unwrap().to_string();
            let status = responses
                .iter()
                .find_map(|(expected, status)| (*expected == path).then_some(*status))
                .unwrap();
            let response =
                format!("HTTP/1.1 {status} test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            socket.write_all(response.as_bytes()).await.unwrap();
            paths.push(path);
        }
        paths
    });
    (format!("http://{address}"), server)
}

#[test]
fn test_datadog_config_builder() {
    let config = DataDogConfig::new("test-api-key")
        .site("datadoghq.eu")
        .service("my-service")
        .env("production")
        .version("1.0.0")
        .tag("team", "platform");

    assert_eq!(config.api_key, "test-api-key");
    assert_eq!(config.site, "datadoghq.eu");
    assert_eq!(config.service, "my-service");
    assert_eq!(config.env, Some("production".to_string()));
    assert_eq!(config.version, Some("1.0.0".to_string()));
    assert_eq!(config.tags.get("team"), Some(&"platform".to_string()));
}

#[test]
fn test_datadog_config_urls() {
    let config = DataDogConfig::new("test-key").site("datadoghq.eu");

    assert!(config.metrics_url().contains("datadoghq.eu"));
    assert!(config.logs_url().contains("datadoghq.eu"));
    assert!(config.traces_url().contains("datadoghq.eu"));
}

#[test]
fn test_supported_datadog_sites_are_exact_hostnames() {
    for site in SUPPORTED_DATADOG_SITES {
        assert!(DataDogConfig::is_supported_site(site), "{site}");
    }
    for site in [
        "datadoghq.com@attacker.invalid",
        "datadoghq.com.attacker.invalid",
        "https://datadoghq.com",
        "datadoghq.com/path",
        "datadoghq.com?token=x",
        "datadoghq.com#fragment",
        "datadoghq.com:443",
        "DATADOGHQ.COM",
    ] {
        assert!(!DataDogConfig::is_supported_site(site), "{site}");
    }
}

#[test]
fn test_datadog_config_default() {
    let config = DataDogConfig::default();

    assert_eq!(config.site, "datadoghq.com");
    assert_eq!(config.service, "litellm-gateway");
    assert!(config.enable_metrics);
    assert!(config.enable_traces);
    assert!(config.enable_logs);
}

#[test]
fn test_datadog_integration_requires_api_key() {
    let config = DataDogConfig::default();
    let result = DataDogIntegration::new(config);
    assert!(result.is_err());
}

#[test]
fn test_datadog_integration_rejects_site_host_confusion() {
    let config = DataDogConfig::new("test-api-key").site("datadoghq.com@attacker.invalid");
    let result = DataDogIntegration::new(config);

    assert!(result.is_err());
}

#[test]
fn test_datadog_integration_creation() {
    let config = DataDogConfig::new("test-api-key");
    let result = DataDogIntegration::new(config);
    assert!(result.is_ok());

    let integration = result.unwrap();
    assert_eq!(integration.name(), "datadog");
    assert!(integration.is_enabled());
}

#[tokio::test]
async fn test_datadog_auto_flush_requeues_failed_batch() {
    let mut config = DataDogConfig::new("test-api-key");
    config.batch_size = 1;
    let mut integration = DataDogIntegration::new(config).unwrap();
    integration.http_client = reqwest::Client::builder()
        .no_proxy()
        .resolve(
            "api.datadoghq.com",
            "127.0.0.1:9".parse().expect("test socket address"),
        )
        .timeout(Duration::from_millis(100))
        .build()
        .expect("test HTTP client");

    let result = integration
        .record_metric("test.metric", 1.0, 1, &[], None)
        .await;

    assert!(result.is_err());
    assert_eq!(
        integration.buffer.read().await.len(),
        1,
        "failed automatic flush must retain the event for a later retry"
    );
    integration
        .record_metric("test.metric.two", 2.0, 1, &[], None)
        .await
        .expect("an existing failed batch must not be retried by every event");
    assert_eq!(integration.buffer.read().await.len(), 2);
    assert!(
        integration
            .record_metric("test.metric.three", 3.0, 1, &[], None)
            .await
            .is_err(),
        "the bounded two-batch buffer must report saturation"
    );
}

#[tokio::test]
async fn test_datadog_manager_timeout_preserves_in_flight_batch() {
    use crate::core::integrations::{IntegrationManager, IntegrationManagerConfig};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (_socket, _) = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
    });

    let mut config = DataDogConfig::new("test-api-key");
    config.batch_size = 1;
    config.enable_logs = false;
    let mut integration = DataDogIntegration::new(config).unwrap();
    integration.http_client = reqwest::Client::builder()
        .no_proxy()
        .resolve("api.datadoghq.com", address)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    let integration = Arc::new(integration);
    let manager = IntegrationManager::new(
        IntegrationManagerConfig::new()
            .parallel(false)
            .fail_fast(true)
            .timeout_ms(200),
    );
    manager.register(integration.clone()).await;

    let result = manager
        .on_llm_start(&LlmStartEvent::new("request-id", "test-model"))
        .await;

    assert!(result.is_err(), "the controlled exporter must time out");
    assert_eq!(
        integration.buffer.read().await.len(),
        1,
        "manager cancellation must not discard the in-flight batch"
    );
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn test_datadog_mixed_batch_retries_only_non_successful_endpoint() {
    let (base_url, server) = response_server(vec![("/metrics", 202), ("/logs", 503)]).await;
    let mut config = DataDogConfig::new("test-api-key");
    config.batch_size = 10;
    let mut integration = DataDogIntegration::new(config).unwrap();
    integration.metrics_url = format!("{base_url}/metrics");
    integration.logs_url = format!("{base_url}/logs");
    integration
        .record_metric("test.metric", 1.0, 1, &[], None)
        .await
        .unwrap();
    integration
        .record_log("test log", "info", &[])
        .await
        .unwrap();

    let error = integration.flush().await.unwrap_err().to_string();

    assert!(error.contains("logs API returned 503"));
    let mut paths = server.await.unwrap();
    paths.sort();
    assert_eq!(paths, ["/logs", "/metrics"]);
    {
        let buffer = integration.buffer.read().await;
        assert!(buffer.pending.is_empty());
        assert_eq!(buffer.in_flight.len(), 1);
        assert!(matches!(buffer.in_flight[0], BufferedEvent::Log(_)));
    }

    let (base_url, retry_server) = response_server(vec![("/logs", 202)]).await;
    integration.logs_url = format!("{base_url}/logs");
    integration.flush().await.unwrap();

    assert_eq!(retry_server.await.unwrap(), ["/logs"]);
    assert_eq!(integration.buffer.read().await.len(), 0);
}

#[test]
fn test_build_tags() {
    let config = DataDogConfig::new("test-key")
        .service("test-service")
        .env("test")
        .tag("custom", "value");
    let integration = DataDogIntegration::new(config).unwrap();

    let tags = integration.build_tags(&[("extra", "tag")]);

    assert!(tags.contains(&"service:test-service".to_string()));
    assert!(tags.contains(&"env:test".to_string()));
    assert!(tags.contains(&"custom:value".to_string()));
    assert!(tags.contains(&"extra:tag".to_string()));
}
