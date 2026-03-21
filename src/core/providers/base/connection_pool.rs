use std::borrow::Cow;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;
use serde_json;

use crate::core::providers::unified_provider::ProviderError;
use crate::utils::net::http::{HttpClientPoolConfig, create_custom_client_with_config};

/// Type alias for HTTP headers using Cow to avoid allocations for static strings.
///
/// Use `Cow::Borrowed("Header-Name")` for static strings and `Cow::Owned(value)` for dynamic values.
pub type HeaderPair = (Cow<'static, str>, Cow<'static, str>);

/// Helper to create a header from static key and dynamic value.
#[inline]
pub fn header(key: &'static str, value: String) -> HeaderPair {
    (Cow::Borrowed(key), Cow::Owned(value))
}

/// Helper to create a header from both static key and static value (zero allocation).
#[inline]
pub fn header_static(key: &'static str, value: &'static str) -> HeaderPair {
    (Cow::Borrowed(key), Cow::Borrowed(value))
}

/// Helper to create a header from both dynamic key and value.
#[inline]
pub fn header_owned(key: String, value: String) -> HeaderPair {
    (Cow::Owned(key), Cow::Owned(value))
}

/// Apply a list of `HeaderPair`s to a `reqwest::RequestBuilder`.
///
/// This bridges the `Vec<HeaderPair>` pattern with providers that still use
/// `reqwest::Client` directly instead of `GlobalPoolManager`.
#[inline]
pub fn apply_headers(
    mut builder: reqwest::RequestBuilder,
    headers: Vec<HeaderPair>,
) -> reqwest::RequestBuilder {
    for (key, value) in headers {
        builder = builder.header(key.as_ref(), value.as_ref());
    }
    builder
}

#[derive(Debug, Clone)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
}

/// Unified connection pool configuration
pub struct PoolConfig;
impl PoolConfig {
    pub const TIMEOUT_SECS: u64 = 600;
    pub const POOL_SIZE: usize = 80;
    pub const KEEPALIVE_SECS: u64 = 90;
}

#[inline]
fn pool_http_config() -> HttpClientPoolConfig {
    HttpClientPoolConfig {
        pool_max_idle_per_host: PoolConfig::POOL_SIZE,
        pool_idle_timeout: Duration::from_secs(PoolConfig::KEEPALIVE_SECS),
        ..HttpClientPoolConfig::default()
    }
}

/// Global HTTP client singleton
///
/// This is the actual global singleton that holds the reqwest::Client.
/// All providers share this single client instance for connection pooling efficiency.
static GLOBAL_CLIENT: LazyLock<Arc<Client>> = LazyLock::new(|| {
    let client = create_custom_client_with_config(
        Duration::from_secs(PoolConfig::TIMEOUT_SECS),
        &pool_http_config(),
    )
    .unwrap_or_else(|e| {
        tracing::error!("Failed to create global HTTP client: {}", e);
        // Fallback to a basic client
        Client::new()
    });
    Arc::new(client)
});

/// Get the global HTTP client
///
/// Returns a reference to the shared global HTTP client instance.
/// This is the preferred way to access the HTTP client for connection pooling.
#[inline]
pub fn global_client() -> Arc<Client> {
    Arc::clone(&GLOBAL_CLIENT)
}

/// Get a streaming-ready HTTP client
///
/// Returns the global HTTP client for streaming requests.
/// This should be used instead of `reqwest::Client::new()` for streaming
/// to benefit from connection pooling.
///
/// # Example
///
/// ```ignore
/// use crate::core::providers::base::connection_pool::streaming_client;
///
/// // Instead of: let client = reqwest::Client::new();
/// // Use: let client = streaming_client();
/// let response = streaming_client()
///     .post(&url)
///     .headers(headers)
///     .json(&body)
///     .send()
///     .await?;
/// let stream = response.bytes_stream();
/// ```
#[inline]
pub fn streaming_client() -> Arc<Client> {
    global_client()
}

/// Simplified connection pool without generic complexity
#[derive(Debug, Clone)]
pub struct ConnectionPool {
    client: Arc<Client>,
}

impl ConnectionPool {
    /// Create a new connection pool with optimized settings
    ///
    /// Note: This now uses the global client singleton instead of creating a new client.
    /// For true isolation (rare use cases), use `new_isolated()`.
    pub fn new() -> Result<Self, ProviderError> {
        Ok(Self {
            client: global_client(),
        })
    }

    /// Create an isolated connection pool with its own client
    ///
    /// Use this only when you need a separate connection pool from the global one.
    /// Most use cases should use `new()` which shares the global client.
    pub fn new_isolated() -> Result<Self, ProviderError> {
        let client = create_custom_client_with_config(
            Duration::from_secs(PoolConfig::TIMEOUT_SECS),
            &pool_http_config(),
        )
        .map_err(|e| ProviderError::configuration("Failed to create HTTP client", e.to_string()))?;

        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Get the underlying reqwest client
    pub fn client(&self) -> &Client {
        &self.client
    }
}

/// Global pool manager - single instance for all providers
///
/// This manager wraps the global connection pool singleton.
/// Multiple `GlobalPoolManager` instances all share the same underlying HTTP client.
#[derive(Debug, Clone)]
pub struct GlobalPoolManager {
    pool: Arc<ConnectionPool>,
}

impl GlobalPoolManager {
    /// Create a new global pool manager
    ///
    /// Note: This returns a manager that uses the global client singleton.
    /// Creating multiple managers is cheap as they all share the same client.
    pub fn new() -> Result<Self, ProviderError> {
        Ok(Self {
            pool: Arc::new(ConnectionPool::new()?),
        })
    }

    /// Get a shared instance of the global pool manager
    ///
    /// This is the most efficient way to get a pool manager as it reuses
    /// the global singleton without any additional allocations.
    pub fn shared() -> Self {
        // Use the global client directly
        Self {
            pool: Arc::new(ConnectionPool {
                client: global_client(),
            }),
        }
    }

    /// Execute an HTTP request
    ///
    /// Uses `HeaderPair` (Cow-based) for headers to avoid allocations for static strings.
    /// Use `header("Key", value)` for static keys or `header_owned(key, value)` for dynamic keys.
    pub async fn execute_request(
        &self,
        url: &str,
        method: HttpMethod,
        headers: Vec<HeaderPair>,
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response, ProviderError> {
        let client = self.pool.client();

        let mut request_builder = match method {
            HttpMethod::GET => client.get(url),
            HttpMethod::POST => client.post(url),
            HttpMethod::PUT => client.put(url),
            HttpMethod::DELETE => client.delete(url),
        };

        // Add headers - Cow allows zero-copy for static strings
        for (key, value) in headers {
            request_builder = request_builder.header(key.as_ref(), value.as_ref());
        }

        // Add body if present
        if let Some(body_data) = body {
            request_builder = request_builder
                .header("Content-Type", "application/json")
                .json(&body_data);
        }

        request_builder
            .send()
            .await
            .map_err(|e| ProviderError::network("common", e.to_string()))
    }

    /// Get the underlying client for direct use
    pub fn client(&self) -> &Client {
        self.pool.client()
    }
}

impl Default for GlobalPoolManager {
    /// Create a default GlobalPoolManager
    ///
    /// Uses the global client singleton, so this is always cheap and fast.
    fn default() -> Self {
        Self::shared()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_creation() {
        let pool = ConnectionPool::new();
        assert!(pool.is_ok());
    }

    #[tokio::test]
    async fn test_global_manager() {
        let manager = GlobalPoolManager::new();
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_global_client_singleton() {
        // Get the global client twice
        let client1 = global_client();
        let client2 = global_client();

        // They should point to the same underlying Arc (same pointer)
        assert!(Arc::ptr_eq(&client1, &client2));
    }

    #[tokio::test]
    async fn test_multiple_managers_share_client() {
        // Create multiple managers
        let manager1 = GlobalPoolManager::new().unwrap();
        let manager2 = GlobalPoolManager::new().unwrap();
        let manager3 = GlobalPoolManager::shared();

        // All should share the same underlying client
        let client1 = manager1.pool.client.clone();
        let client2 = manager2.pool.client.clone();
        let client3 = manager3.pool.client.clone();

        assert!(Arc::ptr_eq(&client1, &client2));
        assert!(Arc::ptr_eq(&client2, &client3));
    }

    #[tokio::test]
    async fn test_isolated_pool_is_different() {
        // Get the global client
        let global = global_client();

        // Create an isolated pool
        let isolated = ConnectionPool::new_isolated().unwrap();

        // The isolated pool should have a different client
        assert!(!Arc::ptr_eq(&global, &isolated.client));
    }

    #[test]
    fn test_default_manager() {
        let manager = GlobalPoolManager::default();
        // Should work without panic
        let _client = manager.client();
    }
}
