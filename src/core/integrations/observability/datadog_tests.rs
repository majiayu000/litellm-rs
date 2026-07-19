use super::*;

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
