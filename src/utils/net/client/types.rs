use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Configuration for HTTP client behavior
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub timeout: Duration,
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub proxy: Option<String>,
    pub user_agent: String,
    pub default_headers: HashMap<String, String>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
            proxy: None,
            user_agent: "litellm-rust/1.0".to_string(),
            default_headers: HashMap::new(),
        }
    }
}

/// Configuration for retry behavior with exponential backoff
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Metrics for tracking request performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRequestMetrics {
    pub start_time: std::time::SystemTime,
    pub end_time: Option<std::time::SystemTime>,
    pub duration: Option<Duration>,
    pub retry_count: u32,
    pub provider: String,
    pub model: String,
    pub status_code: Option<u16>,
}

impl ProviderRequestMetrics {
    pub fn new(provider: String, model: String) -> Self {
        Self {
            start_time: std::time::SystemTime::now(),
            end_time: None,
            duration: None,
            retry_count: 0,
            provider,
            model,
            status_code: None,
        }
    }

    pub fn finish(&mut self, status_code: Option<u16>) {
        let now = std::time::SystemTime::now();
        self.end_time = Some(now);
        self.duration = now.duration_since(self.start_time).ok();
        self.status_code = status_code;
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::SystemTime;

    // ==================== HttpClientConfig Tests ====================

    #[test]
    fn test_http_client_config_default() {
        let config = HttpClientConfig::default();

        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay, Duration::from_millis(1000));
        assert!(config.proxy.is_none());
        assert_eq!(config.user_agent, "litellm-rust/1.0");
        assert!(config.default_headers.is_empty());
    }

    #[test]
    fn test_http_client_config_custom() {
        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "value".to_string());

        let config = HttpClientConfig {
            timeout: Duration::from_secs(30),
            max_retries: 5,
            retry_delay: Duration::from_millis(500),
            proxy: Some("http://proxy.example.com:8080".to_string()),
            user_agent: "custom-agent/2.0".to_string(),
            default_headers: headers,
        };

        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 5);
        assert!(config.proxy.is_some());
        assert_eq!(config.default_headers.len(), 1);
    }

    #[test]
    fn test_http_client_config_clone() {
        let config = HttpClientConfig::default();
        let cloned = config.clone();

        assert_eq!(cloned.timeout, config.timeout);
        assert_eq!(cloned.max_retries, config.max_retries);
        assert_eq!(cloned.user_agent, config.user_agent);
    }

    #[test]
    fn test_http_client_config_debug() {
        let config = HttpClientConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("HttpClientConfig"));
        assert!(debug_str.contains("timeout"));
        assert!(debug_str.contains("litellm-rust"));
    }

    #[test]
    fn test_http_client_config_with_headers() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token".to_string());
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let config = HttpClientConfig {
            default_headers: headers,
            ..HttpClientConfig::default()
        };

        assert_eq!(config.default_headers.len(), 2);
        assert_eq!(
            config.default_headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
    }

    // ==================== RetryConfig Tests ====================

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();

        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(1000));
        assert_eq!(config.max_delay, Duration::from_secs(60));
        assert!((config.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert!(config.jitter);
    }

    #[test]
    fn test_retry_config_custom() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 1.5,
            jitter: false,
        };

        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_delay, Duration::from_millis(500));
        assert!(!config.jitter);
    }

    #[test]
    fn test_retry_config_clone() {
        let config = RetryConfig::default();
        let cloned = config.clone();

        assert_eq!(cloned.max_retries, config.max_retries);
        assert_eq!(cloned.backoff_multiplier, config.backoff_multiplier);
    }

    #[test]
    fn test_retry_config_debug() {
        let config = RetryConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("RetryConfig"));
        assert!(debug_str.contains("max_retries"));
    }

    #[test]
    fn test_retry_config_backoff_calculation() {
        let config = RetryConfig::default();

        // Simulate backoff delays
        let mut delay = config.initial_delay;
        let delays: Vec<Duration> = (0..config.max_retries)
            .map(|_| {
                let current = delay;
                delay = Duration::from_secs_f64(
                    (delay.as_secs_f64() * config.backoff_multiplier)
                        .min(config.max_delay.as_secs_f64()),
                );
                current
            })
            .collect();

        assert_eq!(delays[0], Duration::from_millis(1000));
        assert_eq!(delays[1], Duration::from_millis(2000));
        assert_eq!(delays[2], Duration::from_millis(4000));
    }

    #[test]
    fn test_retry_config_no_retries() {
        let config = RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        };

        assert_eq!(config.max_retries, 0);
    }

    #[test]
    fn test_retry_config_aggressive() {
        let config = RetryConfig {
            max_retries: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 1.2,
            jitter: true,
        };

        assert_eq!(config.max_retries, 10);
        assert!(config.initial_delay < Duration::from_secs(1));
    }

    // ==================== ProviderRequestMetrics Tests ====================

    #[test]
    fn test_request_metrics_new() {
        let metrics = ProviderRequestMetrics::new("openai".to_string(), "gpt-4".to_string());

        assert_eq!(metrics.provider, "openai");
        assert_eq!(metrics.model, "gpt-4");
        assert_eq!(metrics.retry_count, 0);
        assert!(metrics.end_time.is_none());
        assert!(metrics.duration.is_none());
        assert!(metrics.status_code.is_none());
    }

    #[test]
    fn test_request_metrics_finish() {
        let mut metrics =
            ProviderRequestMetrics::new("anthropic".to_string(), "claude-3".to_string());

        // Small delay to ensure measurable duration
        thread::sleep(Duration::from_millis(10));

        metrics.finish(Some(200));

        assert!(metrics.end_time.is_some());
        assert!(metrics.duration.is_some());
        assert!(metrics.duration.unwrap() >= Duration::from_millis(10));
        assert_eq!(metrics.status_code, Some(200));
    }

    #[test]
    fn test_request_metrics_finish_no_status() {
        let mut metrics = ProviderRequestMetrics::new("azure".to_string(), "gpt-4".to_string());
        metrics.finish(None);

        assert!(metrics.end_time.is_some());
        assert!(metrics.status_code.is_none());
    }

    #[test]
    fn test_request_metrics_increment_retry() {
        let mut metrics =
            ProviderRequestMetrics::new("openai".to_string(), "gpt-3.5-turbo".to_string());

        assert_eq!(metrics.retry_count, 0);

        metrics.increment_retry();
        assert_eq!(metrics.retry_count, 1);

        metrics.increment_retry();
        assert_eq!(metrics.retry_count, 2);

        metrics.increment_retry();
        assert_eq!(metrics.retry_count, 3);
    }

    #[test]
    fn test_request_metrics_clone() {
        let mut metrics = ProviderRequestMetrics::new("openai".to_string(), "gpt-4".to_string());
        metrics.increment_retry();
        metrics.finish(Some(200));

        let cloned = metrics.clone();
        assert_eq!(cloned.provider, metrics.provider);
        assert_eq!(cloned.retry_count, metrics.retry_count);
        assert_eq!(cloned.status_code, metrics.status_code);
    }

    #[test]
    fn test_request_metrics_debug() {
        let metrics = ProviderRequestMetrics::new("openai".to_string(), "gpt-4".to_string());
        let debug_str = format!("{:?}", metrics);

        assert!(debug_str.contains("ProviderRequestMetrics"));
        assert!(debug_str.contains("openai"));
        assert!(debug_str.contains("gpt-4"));
    }

    #[test]
    fn test_request_metrics_serialization() {
        let mut metrics = ProviderRequestMetrics::new("openai".to_string(), "gpt-4".to_string());
        metrics.finish(Some(200));

        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("openai"));
        assert!(json.contains("gpt-4"));
        assert!(json.contains("200"));

        let parsed: ProviderRequestMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.provider, "openai");
        assert_eq!(parsed.status_code, Some(200));
    }

    #[test]
    fn test_request_metrics_start_time() {
        let before = SystemTime::now();
        let metrics = ProviderRequestMetrics::new("provider".to_string(), "model".to_string());
        let after = SystemTime::now();

        assert!(metrics.start_time >= before);
        assert!(metrics.start_time <= after);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_full_request_lifecycle() {
        let _config = HttpClientConfig::default();
        let retry_config = RetryConfig::default();

        let mut metrics = ProviderRequestMetrics::new("openai".to_string(), "gpt-4".to_string());

        // Simulate retries
        for _ in 0..retry_config.max_retries {
            metrics.increment_retry();
            // Would normally wait here
        }

        metrics.finish(Some(200));

        assert_eq!(metrics.retry_count, retry_config.max_retries);
        assert!(metrics.duration.is_some());
        assert_eq!(metrics.status_code, Some(200));
    }

    #[test]
    fn test_failed_request_metrics() {
        let mut metrics = ProviderRequestMetrics::new("provider".to_string(), "model".to_string());

        metrics.increment_retry();
        metrics.increment_retry();
        metrics.finish(Some(500));

        assert_eq!(metrics.retry_count, 2);
        assert_eq!(metrics.status_code, Some(500));
    }

    #[test]
    fn test_timeout_config_relationship() {
        let http_config = HttpClientConfig {
            timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_secs(5),
            ..HttpClientConfig::default()
        };

        let retry_config = RetryConfig {
            max_retries: http_config.max_retries,
            initial_delay: http_config.retry_delay,
            ..RetryConfig::default()
        };

        assert_eq!(http_config.max_retries, retry_config.max_retries);
        assert_eq!(http_config.retry_delay, retry_config.initial_delay);
    }

    #[test]
    fn test_http_client_config_zero_timeout() {
        let config = HttpClientConfig {
            timeout: Duration::ZERO,
            ..HttpClientConfig::default()
        };

        assert_eq!(config.timeout, Duration::ZERO);
    }

    #[test]
    fn test_different_providers_metrics() {
        let providers = vec![
            ("openai", "gpt-4"),
            ("anthropic", "claude-3"),
            ("azure", "gpt-4-turbo"),
            ("google", "gemini-pro"),
        ];

        for (provider, model) in providers {
            let metrics = ProviderRequestMetrics::new(provider.to_string(), model.to_string());
            assert_eq!(metrics.provider, provider);
            assert_eq!(metrics.model, model);
        }
    }

    #[test]
    fn test_http_status_codes() {
        let status_codes = vec![200, 201, 400, 401, 403, 404, 429, 500, 502, 503];

        for code in status_codes {
            let mut metrics =
                ProviderRequestMetrics::new("provider".to_string(), "model".to_string());
            metrics.finish(Some(code));
            assert_eq!(metrics.status_code, Some(code));
        }
    }
}
