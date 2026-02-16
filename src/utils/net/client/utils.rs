use super::types::{HttpClientConfig, RetryConfig};
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::net::http::create_client_builder;
use reqwest::{Client, Proxy};
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tokio::time::{Instant, sleep};

/// Utility functions for HTTP client operations
pub struct ClientUtils;

impl ClientUtils {
    /// Creates an HTTP client with the specified configuration
    pub fn create_http_client(config: &HttpClientConfig) -> Result<Client, ProviderError> {
        let mut client_builder =
            create_client_builder(config.timeout).user_agent(&config.user_agent);

        if let Some(proxy_url) = &config.proxy {
            let proxy = Proxy::all(proxy_url).map_err(|e| ProviderError::InvalidRequest {
                provider: "unknown",
                message: format!("Invalid proxy configuration: {}", e),
            })?;
            client_builder = client_builder.proxy(proxy);
        }

        if !config.default_headers.is_empty() {
            let mut headers = reqwest::header::HeaderMap::new();

            for (key, value) in &config.default_headers {
                headers.insert(
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                        ProviderError::InvalidRequest {
                            provider: "unknown",
                            message: format!("Invalid header name '{}': {}", key, e),
                        }
                    })?,
                    reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                        ProviderError::InvalidRequest {
                            provider: "unknown",
                            message: format!("Invalid header value for '{}': {}", key, e),
                        }
                    })?,
                );
            }

            client_builder = client_builder.default_headers(headers);
        }

        let client = client_builder
            .build()
            .map_err(|e| ProviderError::InvalidRequest {
                provider: "unknown",
                message: format!("Failed to build HTTP client: {}", e),
            })?;

        Ok(client)
    }

    /// Gets environment-configured proxies
    pub fn get_environment_proxies() -> HashMap<String, String> {
        let mut proxies = HashMap::new();

        if let Ok(http_proxy) = env::var("HTTP_PROXY") {
            proxies.insert("http".to_string(), http_proxy);
        }

        if let Ok(https_proxy) = env::var("HTTPS_PROXY") {
            proxies.insert("https".to_string(), https_proxy);
        }

        if let Ok(all_proxy) = env::var("ALL_PROXY") {
            if !proxies.contains_key("http") {
                proxies.insert("http".to_string(), all_proxy.clone());
            }
            if !proxies.contains_key("https") {
                proxies.insert("https".to_string(), all_proxy);
            }
        }

        proxies
    }

    /// Determines if a request should be retried based on status code
    pub fn should_retry_request(status_code: u16, attempt: u32, max_retries: u32) -> bool {
        if attempt >= max_retries {
            return false;
        }

        match status_code {
            429 => true,       // Rate limited
            500..=599 => true, // Server errors
            408 => true,       // Request timeout
            _ => false,
        }
    }

    /// Calculates the delay before the next retry using exponential backoff
    pub fn calculate_retry_delay(
        config: &RetryConfig,
        attempt: u32,
        retry_after: Option<Duration>,
    ) -> Duration {
        if let Some(server_delay) = retry_after {
            return server_delay;
        }

        let base_delay = config.initial_delay.as_millis() as f64;
        let exponential_delay = base_delay * config.backoff_multiplier.powi(attempt as i32);

        let delay_ms = if config.jitter {
            let jitter_factor = 0.1; // 10% jitter
            let jitter = exponential_delay * jitter_factor * (rand::random::<f64>() - 0.5);
            exponential_delay + jitter
        } else {
            exponential_delay
        };

        let capped_delay = delay_ms.min(config.max_delay.as_millis() as f64);
        Duration::from_millis(capped_delay as u64)
    }

    /// Executes an operation with retry logic
    pub async fn execute_with_retry<F, T, E>(
        operation: F,
        config: &RetryConfig,
    ) -> Result<T, ProviderError>
    where
        F: Fn() -> Result<T, E> + Clone,
        E: Into<ProviderError> + Clone,
    {
        let mut last_error: Option<ProviderError> = None;

        for attempt in 0..=config.max_retries {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let error: ProviderError = e.into();
                    last_error = Some(error.clone());

                    if attempt < config.max_retries {
                        let delay = Self::calculate_retry_delay(config, attempt, None);
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ProviderError::Network {
            provider: "unknown",
            message: "Max retries exceeded".to_string(),
        }))
    }

    /// Gets the default timeout for a specific provider
    pub fn get_timeout_for_provider(provider: &str) -> Duration {
        match provider.to_lowercase().as_str() {
            "openai" => Duration::from_secs(120),
            "anthropic" => Duration::from_secs(180),
            "google" => Duration::from_secs(90),
            "azure" => Duration::from_secs(120),
            "cohere" => Duration::from_secs(60),
            _ => Duration::from_secs(60),
        }
    }

    /// Checks if a provider supports httpx timeout
    pub fn supports_httpx_timeout(provider: &str) -> bool {
        let supported_providers = [
            "openai",
            "anthropic",
            "google",
            "azure",
            "cohere",
            "mistral",
            "replicate",
        ];

        supported_providers.contains(&provider.to_lowercase().as_str())
    }

    /// Gets the user agent string for a specific provider
    pub fn get_user_agent_for_provider(provider: &str) -> String {
        match provider.to_lowercase().as_str() {
            "openai" => "litellm-rust-openai/1.0".to_string(),
            "anthropic" => "litellm-rust-anthropic/1.0".to_string(),
            "google" => "litellm-rust-google/1.0".to_string(),
            _ => "litellm-rust/1.0".to_string(),
        }
    }

    /// Appends a path to an API base URL
    pub fn add_path_to_api_base(api_base: &str, ending_path: &str) -> String {
        let base = api_base.trim_end_matches('/');
        let path = ending_path.trim_start_matches('/');
        format!("{}/{}", base, path)
    }

    /// Validates a URL for correctness
    pub fn validate_url(url: &str) -> Result<(), ProviderError> {
        let parsed = url::Url::parse(url).map_err(|e| ProviderError::InvalidRequest {
            provider: "unknown",
            message: format!("Invalid URL '{}': {}", url, e),
        })?;

        match parsed.scheme() {
            "http" | "https" => Ok(()),
            scheme => Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: format!(
                    "Unsupported URL scheme '{}'. Only http and https are supported",
                    scheme
                ),
            }),
        }
    }

    /// Extracts retry-after information from response headers
    pub fn extract_retry_after_from_headers(
        headers: &reqwest::header::HeaderMap,
    ) -> Option<Duration> {
        if let Some(retry_after) = headers.get("retry-after") {
            if let Ok(retry_str) = retry_after.to_str() {
                if let Ok(seconds) = retry_str.parse::<u64>() {
                    return Some(Duration::from_secs(seconds));
                }
            }
        }

        if let Some(rate_limit_reset) = headers.get("x-ratelimit-reset") {
            if let Ok(reset_str) = rate_limit_reset.to_str() {
                if let Ok(reset_time) = reset_str.parse::<u64>() {
                    let current_time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    if reset_time > current_time {
                        return Some(Duration::from_secs(reset_time - current_time));
                    }
                }
            }
        }

        None
    }

    /// Creates a provider-specific HTTP client
    pub fn create_provider_specific_client(provider: &str) -> Result<Client, ProviderError> {
        let mut config = HttpClientConfig {
            timeout: Self::get_timeout_for_provider(provider),
            user_agent: Self::get_user_agent_for_provider(provider),
            ..Default::default()
        };

        if provider == "anthropic" {
            config
                .default_headers
                .insert("anthropic-version".to_string(), "2023-06-01".to_string());
        }

        Self::create_http_client(&config)
    }

    /// Gets default headers for a specific provider
    pub fn get_default_headers_for_provider(provider: &str) -> HashMap<String, String> {
        let mut headers = HashMap::new();

        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Accept".to_string(), "application/json".to_string());

        match provider.to_lowercase().as_str() {
            "anthropic" => {
                headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());
            }
            "google" => {
                headers.insert("x-goog-api-key".to_string(), "placeholder".to_string());
            }
            "azure" => {
                headers.insert("api-key".to_string(), "placeholder".to_string());
            }
            _ => {}
        }

        headers
    }

    /// Tests a connection to a URL
    pub async fn test_connection(
        url: &str,
        timeout: Option<Duration>,
    ) -> Result<bool, ProviderError> {
        Self::validate_url(url)?;

        let client = create_client_builder(timeout.unwrap_or(Duration::from_secs(10)))
            .build()
            .map_err(|e| ProviderError::Network {
                provider: "unknown",
                message: format!("Failed to create test client: {}", e),
            })?;

        let start_time = Instant::now();

        let response = client
            .head(url)
            .send()
            .await
            .map_err(|e| ProviderError::Network {
                provider: "unknown",
                message: format!("Connection test failed: {}", e),
            })?;

        let _duration = start_time.elapsed();

        Ok(response.status().is_success() || response.status().as_u16() == 405) // HEAD might not be allowed
    }

    /// Parses a content-type header into media type and parameters
    pub fn parse_content_type(content_type: &str) -> (String, HashMap<String, String>) {
        let parts: Vec<&str> = content_type.split(';').collect();
        let media_type = parts[0].trim().to_lowercase();

        let mut parameters = HashMap::new();
        for part in parts.iter().skip(1) {
            if let Some(eq_pos) = part.find('=') {
                let key = part[..eq_pos].trim().to_lowercase();
                let value = part[eq_pos + 1..].trim().trim_matches('"');
                parameters.insert(key, value.to_string());
            }
        }

        (media_type, parameters)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== should_retry_request Tests ====================

    #[test]
    fn test_should_retry_rate_limited() {
        assert!(ClientUtils::should_retry_request(429, 0, 3));
        assert!(ClientUtils::should_retry_request(429, 1, 3));
        assert!(ClientUtils::should_retry_request(429, 2, 3));
        assert!(!ClientUtils::should_retry_request(429, 3, 3));
    }

    #[test]
    fn test_should_retry_server_errors() {
        assert!(ClientUtils::should_retry_request(500, 0, 3));
        assert!(ClientUtils::should_retry_request(502, 0, 3));
        assert!(ClientUtils::should_retry_request(503, 0, 3));
        assert!(ClientUtils::should_retry_request(504, 0, 3));
        assert!(ClientUtils::should_retry_request(599, 0, 3));
    }

    #[test]
    fn test_should_retry_request_timeout() {
        assert!(ClientUtils::should_retry_request(408, 0, 3));
    }

    #[test]
    fn test_should_not_retry_client_errors() {
        assert!(!ClientUtils::should_retry_request(400, 0, 3));
        assert!(!ClientUtils::should_retry_request(401, 0, 3));
        assert!(!ClientUtils::should_retry_request(403, 0, 3));
        assert!(!ClientUtils::should_retry_request(404, 0, 3));
    }

    #[test]
    fn test_should_not_retry_success() {
        assert!(!ClientUtils::should_retry_request(200, 0, 3));
        assert!(!ClientUtils::should_retry_request(201, 0, 3));
        assert!(!ClientUtils::should_retry_request(204, 0, 3));
    }

    #[test]
    fn test_should_not_retry_max_attempts() {
        assert!(!ClientUtils::should_retry_request(500, 5, 3));
        assert!(!ClientUtils::should_retry_request(429, 10, 5));
    }

    // ==================== calculate_retry_delay Tests ====================

    #[test]
    fn test_calculate_retry_delay_respects_server_delay() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let server_delay = Duration::from_secs(10);
        let delay = ClientUtils::calculate_retry_delay(&config, 0, Some(server_delay));
        assert_eq!(delay, server_delay);
    }

    #[test]
    fn test_calculate_retry_delay_exponential_backoff() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let delay0 = ClientUtils::calculate_retry_delay(&config, 0, None);
        let delay1 = ClientUtils::calculate_retry_delay(&config, 1, None);
        let delay2 = ClientUtils::calculate_retry_delay(&config, 2, None);

        assert_eq!(delay0, Duration::from_millis(100));
        assert_eq!(delay1, Duration::from_millis(200));
        assert_eq!(delay2, Duration::from_millis(400));
    }

    #[test]
    fn test_calculate_retry_delay_respects_max() {
        let config = RetryConfig {
            max_retries: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(500),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        // At attempt 5: 100 * 2^5 = 3200ms, but max is 500ms
        let delay = ClientUtils::calculate_retry_delay(&config, 5, None);
        assert_eq!(delay, Duration::from_millis(500));
    }

    #[test]
    fn test_calculate_retry_delay_with_jitter() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        };

        // With jitter, delay should be around 100ms +/- 10%
        let delay = ClientUtils::calculate_retry_delay(&config, 0, None);
        assert!(delay >= Duration::from_millis(90));
        assert!(delay <= Duration::from_millis(110));
    }

    // ==================== get_timeout_for_provider Tests ====================

    #[test]
    fn test_get_timeout_openai() {
        let timeout = ClientUtils::get_timeout_for_provider("openai");
        assert_eq!(timeout, Duration::from_secs(120));
    }

    #[test]
    fn test_get_timeout_anthropic() {
        let timeout = ClientUtils::get_timeout_for_provider("anthropic");
        assert_eq!(timeout, Duration::from_secs(180));
    }

    #[test]
    fn test_get_timeout_google() {
        let timeout = ClientUtils::get_timeout_for_provider("google");
        assert_eq!(timeout, Duration::from_secs(90));
    }

    #[test]
    fn test_get_timeout_azure() {
        let timeout = ClientUtils::get_timeout_for_provider("azure");
        assert_eq!(timeout, Duration::from_secs(120));
    }

    #[test]
    fn test_get_timeout_cohere() {
        let timeout = ClientUtils::get_timeout_for_provider("cohere");
        assert_eq!(timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_get_timeout_unknown() {
        let timeout = ClientUtils::get_timeout_for_provider("unknown-provider");
        assert_eq!(timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_get_timeout_case_insensitive() {
        assert_eq!(
            ClientUtils::get_timeout_for_provider("OpenAI"),
            ClientUtils::get_timeout_for_provider("openai")
        );
        assert_eq!(
            ClientUtils::get_timeout_for_provider("ANTHROPIC"),
            ClientUtils::get_timeout_for_provider("anthropic")
        );
    }

    // ==================== supports_httpx_timeout Tests ====================

    #[test]
    fn test_supports_httpx_timeout_openai() {
        assert!(ClientUtils::supports_httpx_timeout("openai"));
    }

    #[test]
    fn test_supports_httpx_timeout_anthropic() {
        assert!(ClientUtils::supports_httpx_timeout("anthropic"));
    }

    #[test]
    fn test_supports_httpx_timeout_google() {
        assert!(ClientUtils::supports_httpx_timeout("google"));
    }

    #[test]
    fn test_supports_httpx_timeout_azure() {
        assert!(ClientUtils::supports_httpx_timeout("azure"));
    }

    #[test]
    fn test_supports_httpx_timeout_cohere() {
        assert!(ClientUtils::supports_httpx_timeout("cohere"));
    }

    #[test]
    fn test_supports_httpx_timeout_mistral() {
        assert!(ClientUtils::supports_httpx_timeout("mistral"));
    }

    #[test]
    fn test_supports_httpx_timeout_replicate() {
        assert!(ClientUtils::supports_httpx_timeout("replicate"));
    }

    #[test]
    fn test_supports_httpx_timeout_unknown() {
        assert!(!ClientUtils::supports_httpx_timeout("unknown"));
    }

    #[test]
    fn test_supports_httpx_timeout_case_insensitive() {
        assert!(ClientUtils::supports_httpx_timeout("OPENAI"));
        assert!(ClientUtils::supports_httpx_timeout("Anthropic"));
    }

    // ==================== get_user_agent_for_provider Tests ====================

    #[test]
    fn test_user_agent_openai() {
        assert_eq!(
            ClientUtils::get_user_agent_for_provider("openai"),
            "litellm-rust-openai/1.0"
        );
    }

    #[test]
    fn test_user_agent_anthropic() {
        assert_eq!(
            ClientUtils::get_user_agent_for_provider("anthropic"),
            "litellm-rust-anthropic/1.0"
        );
    }

    #[test]
    fn test_user_agent_google() {
        assert_eq!(
            ClientUtils::get_user_agent_for_provider("google"),
            "litellm-rust-google/1.0"
        );
    }

    #[test]
    fn test_user_agent_unknown() {
        assert_eq!(
            ClientUtils::get_user_agent_for_provider("unknown"),
            "litellm-rust/1.0"
        );
    }

    // ==================== add_path_to_api_base Tests ====================

    #[test]
    fn test_add_path_basic() {
        assert_eq!(
            ClientUtils::add_path_to_api_base("https://api.example.com", "v1/chat"),
            "https://api.example.com/v1/chat"
        );
    }

    #[test]
    fn test_add_path_with_trailing_slash() {
        assert_eq!(
            ClientUtils::add_path_to_api_base("https://api.example.com/", "v1/chat"),
            "https://api.example.com/v1/chat"
        );
    }

    #[test]
    fn test_add_path_with_leading_slash() {
        assert_eq!(
            ClientUtils::add_path_to_api_base("https://api.example.com", "/v1/chat"),
            "https://api.example.com/v1/chat"
        );
    }

    #[test]
    fn test_add_path_with_both_slashes() {
        assert_eq!(
            ClientUtils::add_path_to_api_base("https://api.example.com/", "/v1/chat"),
            "https://api.example.com/v1/chat"
        );
    }

    #[test]
    fn test_add_path_multiple_trailing_slashes() {
        // trim_end_matches('/') removes all trailing slashes
        // trim_start_matches('/') removes all leading slashes
        assert_eq!(
            ClientUtils::add_path_to_api_base("https://api.example.com///", "///v1/chat"),
            "https://api.example.com/v1/chat"
        );
    }

    // ==================== validate_url Tests ====================

    #[test]
    fn test_validate_url_https() {
        assert!(ClientUtils::validate_url("https://api.example.com").is_ok());
    }

    #[test]
    fn test_validate_url_http() {
        assert!(ClientUtils::validate_url("http://api.example.com").is_ok());
    }

    #[test]
    fn test_validate_url_with_path() {
        assert!(ClientUtils::validate_url("https://api.example.com/v1/chat").is_ok());
    }

    #[test]
    fn test_validate_url_with_query() {
        assert!(ClientUtils::validate_url("https://api.example.com?key=value").is_ok());
    }

    #[test]
    fn test_validate_url_invalid_scheme() {
        let result = ClientUtils::validate_url("ftp://files.example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_url_invalid_format() {
        let result = ClientUtils::validate_url("not a valid url");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_url_empty() {
        let result = ClientUtils::validate_url("");
        assert!(result.is_err());
    }

    // ==================== parse_content_type Tests ====================

    #[test]
    fn test_parse_content_type_simple() {
        let (media_type, params) = ClientUtils::parse_content_type("application/json");
        assert_eq!(media_type, "application/json");
        assert!(params.is_empty());
    }

    #[test]
    fn test_parse_content_type_with_charset() {
        let (media_type, params) =
            ClientUtils::parse_content_type("application/json; charset=utf-8");
        assert_eq!(media_type, "application/json");
        assert_eq!(params.get("charset"), Some(&"utf-8".to_string()));
    }

    #[test]
    fn test_parse_content_type_multiple_params() {
        let (media_type, params) =
            ClientUtils::parse_content_type("text/html; charset=utf-8; boundary=something");
        assert_eq!(media_type, "text/html");
        assert_eq!(params.get("charset"), Some(&"utf-8".to_string()));
        assert_eq!(params.get("boundary"), Some(&"something".to_string()));
    }

    #[test]
    fn test_parse_content_type_quoted_value() {
        let (media_type, params) = ClientUtils::parse_content_type(
            "multipart/form-data; boundary=\"----WebKitFormBoundary\"",
        );
        assert_eq!(media_type, "multipart/form-data");
        assert_eq!(
            params.get("boundary"),
            Some(&"----WebKitFormBoundary".to_string())
        );
    }

    #[test]
    fn test_parse_content_type_case_insensitive() {
        let (media_type, _) = ClientUtils::parse_content_type("Application/JSON");
        assert_eq!(media_type, "application/json");
    }

    #[test]
    fn test_parse_content_type_with_spaces() {
        let (media_type, params) =
            ClientUtils::parse_content_type("  application/json ;  charset = utf-8  ");
        assert_eq!(media_type, "application/json");
        assert_eq!(params.get("charset"), Some(&"utf-8".to_string()));
    }

    // ==================== get_default_headers_for_provider Tests ====================

    #[test]
    fn test_default_headers_has_content_type() {
        let headers = ClientUtils::get_default_headers_for_provider("openai");
        assert_eq!(
            headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
    }

    #[test]
    fn test_default_headers_has_accept() {
        let headers = ClientUtils::get_default_headers_for_provider("openai");
        assert_eq!(headers.get("Accept"), Some(&"application/json".to_string()));
    }

    #[test]
    fn test_default_headers_anthropic_version() {
        let headers = ClientUtils::get_default_headers_for_provider("anthropic");
        assert_eq!(
            headers.get("anthropic-version"),
            Some(&"2023-06-01".to_string())
        );
    }

    #[test]
    fn test_default_headers_google_api_key() {
        let headers = ClientUtils::get_default_headers_for_provider("google");
        assert!(headers.contains_key("x-goog-api-key"));
    }

    #[test]
    fn test_default_headers_azure_api_key() {
        let headers = ClientUtils::get_default_headers_for_provider("azure");
        assert!(headers.contains_key("api-key"));
    }

    // ==================== extract_retry_after_from_headers Tests ====================

    #[test]
    fn test_extract_retry_after_seconds() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("retry-after", "30".parse().unwrap());

        let delay = ClientUtils::extract_retry_after_from_headers(&headers);
        assert_eq!(delay, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_extract_retry_after_missing() {
        let headers = reqwest::header::HeaderMap::new();
        let delay = ClientUtils::extract_retry_after_from_headers(&headers);
        assert!(delay.is_none());
    }

    #[test]
    fn test_extract_retry_after_invalid() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("retry-after", "not-a-number".parse().unwrap());

        let delay = ClientUtils::extract_retry_after_from_headers(&headers);
        assert!(delay.is_none());
    }

    // ==================== create_http_client Tests ====================

    #[test]
    fn test_create_http_client_default_config() {
        let config = HttpClientConfig::default();
        let result = ClientUtils::create_http_client(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_http_client_with_timeout() {
        let config = HttpClientConfig {
            timeout: Duration::from_secs(30),
            ..Default::default()
        };
        let result = ClientUtils::create_http_client(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_http_client_with_user_agent() {
        let config = HttpClientConfig {
            user_agent: "test-agent/1.0".to_string(),
            ..Default::default()
        };
        let result = ClientUtils::create_http_client(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_http_client_with_headers() {
        let mut config = HttpClientConfig::default();
        config
            .default_headers
            .insert("X-Custom-Header".to_string(), "value".to_string());
        let result = ClientUtils::create_http_client(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_provider_specific_client_openai() {
        let result = ClientUtils::create_provider_specific_client("openai");
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_provider_specific_client_anthropic() {
        let result = ClientUtils::create_provider_specific_client("anthropic");
        assert!(result.is_ok());
    }

    // ==================== get_environment_proxies Tests ====================

    #[test]
    fn test_get_environment_proxies_empty() {
        // This test just ensures the function doesn't panic
        // Actual proxy values depend on environment
        let _proxies = ClientUtils::get_environment_proxies();
    }
}
