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

        let base_delay = config.initial_delay().as_millis() as f64;
        let exponential_delay = base_delay * config.backoff_multiplier.powi(attempt as i32);

        let delay_ms = if config.jitter {
            let jitter_factor = 0.1; // 10% jitter
            let jitter = exponential_delay * jitter_factor * (rand::random::<f64>() - 0.5);
            exponential_delay + jitter
        } else {
            exponential_delay
        };

        let capped_delay = delay_ms.min(config.max_delay().as_millis() as f64);
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
        if let Some(retry_after) = headers.get("retry-after")
            && let Ok(retry_str) = retry_after.to_str()
            && let Ok(seconds) = retry_str.parse::<u64>()
        {
            return Some(Duration::from_secs(seconds));
        }

        if let Some(rate_limit_reset) = headers.get("x-ratelimit-reset")
            && let Ok(reset_str) = rate_limit_reset.to_str()
            && let Ok(reset_time) = reset_str.parse::<u64>()
        {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if reset_time > current_time {
                return Some(Duration::from_secs(reset_time - current_time));
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
#[path = "utils_tests.rs"]
mod tests;
