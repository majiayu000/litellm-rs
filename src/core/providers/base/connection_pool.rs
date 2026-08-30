use std::borrow::Cow;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;

use futures::StreamExt;
use reqwest::{Client, RequestBuilder, Response};
use serde_json;

use super::config::BaseConfig;
use super::http::{BaseHttpClient, apply_provider_headers};
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::net::http::{
    HttpClientPoolConfig, create_custom_client_with_config, create_streaming_client,
};

/// HTTP headers that borrow static values when possible.
pub type HeaderPair = (Cow<'static, str>, Cow<'static, str>);

#[inline]
pub fn header(key: &'static str, value: String) -> HeaderPair {
    (Cow::Borrowed(key), Cow::Owned(value))
}

#[inline]
pub fn header_static(key: &'static str, value: &'static str) -> HeaderPair {
    (Cow::Borrowed(key), Cow::Borrowed(value))
}

#[inline]
pub fn header_owned(key: String, value: String) -> HeaderPair {
    (Cow::Owned(key), Cow::Owned(value))
}

/// Apply `HeaderPair`s to a raw request builder.
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

pub struct PoolConfig;
impl PoolConfig {
    pub const TIMEOUT_SECS: u64 = 600;
    pub const POOL_SIZE: usize = 80;
    pub const KEEPALIVE_SECS: u64 = 90;
}

pub const STREAMING_HEADER_TIMEOUT_SECS: u64 = PoolConfig::TIMEOUT_SECS;
pub const STREAMING_ERROR_BODY_TIMEOUT_SECS: u64 = 10;
pub const STREAMING_ERROR_BODY_MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum StreamingRequestError {
    #[error("streaming request did not receive response headers within {timeout:?}")]
    HeaderTimeout { timeout: Duration },
    #[error("streaming error body did not finish within {timeout:?}")]
    ErrorBodyTimeout { timeout: Duration },
    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

impl StreamingRequestError {
    pub fn is_timeout(&self) -> bool {
        match self {
            Self::HeaderTimeout { .. } | Self::ErrorBodyTimeout { .. } => true,
            Self::Request(err) => err.is_timeout(),
        }
    }

    pub fn as_reqwest_error(&self) -> Option<&reqwest::Error> {
        match self {
            Self::HeaderTimeout { .. } | Self::ErrorBodyTimeout { .. } => None,
            Self::Request(err) => Some(err),
        }
    }

    pub fn into_provider_error(self, provider: &'static str) -> ProviderError {
        if self.is_timeout() {
            ProviderError::timeout(provider, self.to_string())
        } else {
            ProviderError::network(provider, self.to_string())
        }
    }
}

#[inline]
fn pool_http_config() -> HttpClientPoolConfig {
    HttpClientPoolConfig {
        pool_max_idle_per_host: PoolConfig::POOL_SIZE,
        pool_idle_timeout: Duration::from_secs(PoolConfig::KEEPALIVE_SECS),
        ..HttpClientPoolConfig::default()
    }
}

/// Global shared HTTP client.
static GLOBAL_CLIENT: LazyLock<Arc<Client>> = LazyLock::new(|| {
    let client = create_custom_client_with_config(
        Duration::from_secs(PoolConfig::TIMEOUT_SECS),
        &pool_http_config(),
    )
    .unwrap_or_else(|e| {
        tracing::error!("Failed to create global HTTP client: {}", e);
        crate::core::http::outbound::default_outbound_client().clone()
    });
    Arc::new(client)
});

static STREAMING_CLIENT: LazyLock<Arc<Client>> = LazyLock::new(|| {
    let client = create_streaming_client().unwrap_or_else(|err| {
        tracing::error!("Failed to create streaming HTTP client: {err}");
        crate::core::http::outbound::default_outbound_client().clone()
    });
    Arc::new(client)
});

/// Get the global HTTP client.
#[inline]
pub fn global_client() -> Arc<Client> {
    Arc::clone(&GLOBAL_CLIENT)
}

/// Get the legacy bounded client used by direct streaming callers.
#[inline]
pub fn streaming_client() -> Arc<Client> {
    global_client()
}

/// Get an HTTP client for streaming bodies without a total request timeout.
#[inline]
pub fn streaming_unbounded_client() -> Arc<Client> {
    Arc::clone(&STREAMING_CLIENT)
}

/// Send a streaming request with a bounded pre-header phase.
pub async fn send_streaming_request(
    request_builder: RequestBuilder,
    provider: &'static str,
) -> Result<Response, ProviderError> {
    send_streaming_request_with_timeout(
        request_builder,
        Duration::from_secs(STREAMING_HEADER_TIMEOUT_SECS),
    )
    .await
    .map_err(|err| err.into_provider_error(provider))
}

pub async fn send_streaming_request_with_timeout(
    request_builder: RequestBuilder,
    timeout: Duration,
) -> Result<Response, StreamingRequestError> {
    match tokio::time::timeout(timeout, request_builder.send()).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(err)) => Err(StreamingRequestError::Request(err)),
        Err(_) => Err(StreamingRequestError::HeaderTimeout { timeout }),
    }
}

/// Read a non-success streaming response body with bounded time and memory.
pub async fn read_streaming_error_body(
    response: Response,
) -> Result<String, StreamingRequestError> {
    read_streaming_error_body_with_limits(
        response,
        Duration::from_secs(STREAMING_ERROR_BODY_TIMEOUT_SECS),
        STREAMING_ERROR_BODY_MAX_BYTES,
    )
    .await
}

pub async fn read_streaming_error_body_with_limits(
    response: Response,
    timeout: Duration,
    max_bytes: usize,
) -> Result<String, StreamingRequestError> {
    if max_bytes == 0 {
        return Ok(String::new());
    }

    let bytes = tokio::time::timeout(timeout, async move {
        let mut stream = response.bytes_stream();
        let mut body = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let remaining = max_bytes.saturating_sub(body.len());
            if remaining == 0 {
                break;
            }

            if chunk.len() > remaining {
                body.extend_from_slice(&chunk[..remaining]);
                break;
            }

            body.extend_from_slice(&chunk);
            if body.len() >= max_bytes {
                break;
            }
        }

        Ok::<_, reqwest::Error>(body)
    })
    .await
    .map_err(|_| StreamingRequestError::ErrorBodyTimeout { timeout })??;

    let body = String::from_utf8(bytes)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned());
    Ok(body)
}

/// Simplified connection pool without generic complexity
#[derive(Debug, Clone)]
pub struct ConnectionPool {
    client: Arc<Client>,
}

impl ConnectionPool {
    /// Create a connection pool backed by the global client.
    pub fn new() -> Result<Self, ProviderError> {
        Ok(Self {
            client: global_client(),
        })
    }

    /// Create an isolated connection pool with its own client.
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

/// Manager for the global or provider-policy-bound clients.
#[derive(Debug, Clone)]
pub struct GlobalPoolManager {
    pool: Arc<ConnectionPool>,
    policy: Option<ProviderPool>,
}

#[derive(Debug, Clone)]
struct ProviderPool {
    provider: &'static str,
    ordinary: BaseHttpClient,
    streaming: BaseHttpClient,
    streaming_header_timeout: Duration,
}

impl GlobalPoolManager {
    /// Create a manager backed by the global client.
    pub fn new() -> Result<Self, ProviderError> {
        Ok(Self {
            pool: Arc::new(ConnectionPool::new()?),
            policy: None,
        })
    }

    /// Create a manager whose ordinary and streaming clients are bound to one provider policy.
    pub fn new_for_provider(
        provider: &'static str,
        config: BaseConfig,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            pool: Arc::new(ConnectionPool::new()?),
            policy: Some(ProviderPool {
                provider,
                ordinary: BaseHttpClient::new_for_provider(provider, config.clone())?,
                streaming: BaseHttpClient::new_for_provider_streaming(provider, config)?,
                streaming_header_timeout: Duration::from_secs(STREAMING_HEADER_TIMEOUT_SECS),
            }),
        })
    }

    pub(crate) fn new_for_provider_no_redirect(
        provider: &'static str,
        config: BaseConfig,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            pool: Arc::new(ConnectionPool::new()?),
            policy: Some(ProviderPool {
                provider,
                ordinary: BaseHttpClient::new_for_provider_no_redirect(provider, config.clone())?,
                streaming: BaseHttpClient::new_for_provider_streaming_no_redirect(
                    provider, config,
                )?,
                streaming_header_timeout: Duration::from_secs(STREAMING_HEADER_TIMEOUT_SECS),
            }),
        })
    }

    /// Get a shared instance backed by the global client.
    pub fn shared() -> Self {
        // Use the global client directly
        Self {
            pool: Arc::new(ConnectionPool {
                client: global_client(),
            }),
            policy: None,
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
        if let Some(policy) = &self.policy {
            let method = match method {
                HttpMethod::GET => reqwest::Method::GET,
                HttpMethod::POST => reqwest::Method::POST,
                HttpMethod::PUT => reqwest::Method::PUT,
                HttpMethod::DELETE => reqwest::Method::DELETE,
            };
            let mut request_builder = policy.ordinary.request(method, url)?;
            request_builder = apply_provider_headers(request_builder, headers);
            if let Some(body_data) = body {
                request_builder = request_builder
                    .header("Content-Type", "application/json")
                    .json(&body_data);
            }
            return request_builder
                .send()
                .await
                .map_err(|error| policy.ordinary.map_preserved_request_error(error));
        }

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

    pub(crate) async fn execute_request_preserving_endpoint_policy(
        &self,
        url: &str,
        method: HttpMethod,
        headers: Vec<HeaderPair>,
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response, ProviderError> {
        let Some(policy) = &self.policy else {
            return self.execute_request(url, method, headers, body).await;
        };
        let method = match method {
            HttpMethod::GET => reqwest::Method::GET,
            HttpMethod::POST => reqwest::Method::POST,
            HttpMethod::PUT => reqwest::Method::PUT,
            HttpMethod::DELETE => reqwest::Method::DELETE,
        };
        let mut request = policy
            .ordinary
            .request_preserving_endpoint_policy(method, url)?;
        request = apply_provider_headers(request, headers);
        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/json")
                .json(&body);
        }
        request
            .send()
            .await
            .map_err(|error| policy.ordinary.map_preserved_request_error(error))
    }

    /// Execute a policy-bound request while bounding only the response-header phase.
    pub async fn execute_streaming_request(
        &self,
        url: &str,
        headers: Vec<HeaderPair>,
        body: serde_json::Value,
        legacy_provider: &'static str,
    ) -> Result<reqwest::Response, ProviderError> {
        if let Some(policy) = &self.policy {
            let request = apply_provider_headers(policy.streaming.post(url)?, headers).json(&body);
            let timeout = policy.streaming_header_timeout;
            let response = tokio::time::timeout(timeout, request.send())
                .await
                .map_err(|_| {
                    ProviderError::timeout(policy.provider, "Provider response header timeout")
                })?;
            return response.map_err(|error| policy.streaming.map_preserved_request_error(error));
        }

        let request = apply_headers(streaming_unbounded_client().post(url).json(&body), headers);
        send_streaming_request(request, legacy_provider).await
    }

    pub(crate) async fn execute_streaming_request_preserving_endpoint_policy(
        &self,
        url: &str,
        headers: Vec<HeaderPair>,
        body: serde_json::Value,
        legacy_provider: &'static str,
    ) -> Result<reqwest::Response, ProviderError> {
        let Some(policy) = &self.policy else {
            return self
                .execute_streaming_request(url, headers, body, legacy_provider)
                .await;
        };
        let request = policy
            .streaming
            .request_preserving_endpoint_policy(reqwest::Method::POST, url)?;
        let request = apply_provider_headers(request, headers).json(&body);
        let timeout = policy.streaming_header_timeout;
        match tokio::time::timeout(timeout, request.send()).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(error)) => Err(policy.streaming.map_preserved_request_error(error)),
            Err(_) => Err(ProviderError::timeout(
                policy.provider,
                "Provider response header timeout",
            )),
        }
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn delayed_response_url(
        header_delay: Duration,
        body_delay: Duration,
    ) -> std::io::Result<String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;

        tokio::spawn(async move {
            let (mut socket, _) = match listener.accept().await {
                Ok(connection) => connection,
                Err(err) => panic!("test server accept failed: {err}"),
            };
            let mut buffer = [0_u8; 1024];
            if let Err(err) = socket.read(&mut buffer).await {
                panic!("test server failed to read request: {err}");
            }

            tokio::time::sleep(header_delay).await;
            if let Err(err) = socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n")
                .await
            {
                if !matches!(
                    err.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                ) {
                    panic!("test server failed to write headers: {err}");
                }
                return;
            }

            tokio::time::sleep(body_delay).await;
            match socket.write_all(b"hello").await {
                Ok(()) => {}
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                    ) => {}
                Err(err) => panic!("test server failed to write body: {err}"),
            }
        });

        Ok(format!("http://{addr}"))
    }

    async fn delayed_error_body_url(body_delay: Duration) -> std::io::Result<String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;

        tokio::spawn(async move {
            let (mut socket, _) = match listener.accept().await {
                Ok(connection) => connection,
                Err(err) => panic!("test server accept failed: {err}"),
            };
            let mut buffer = [0_u8; 1024];
            if let Err(err) = socket.read(&mut buffer).await {
                panic!("test server failed to read request: {err}");
            }

            if let Err(err) = socket
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\nConnection: close\r\n\r\n")
                .await
            {
                if !matches!(
                    err.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                ) {
                    panic!("test server failed to write headers: {err}");
                }
                return;
            }

            tokio::time::sleep(body_delay).await;
            match socket.write_all(b"error").await {
                Ok(()) => {}
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                    ) => {}
                Err(err) => panic!("test server failed to write body: {err}"),
            }
        });

        Ok(format!("http://{addr}"))
    }

    async fn error_body_then_stall_url(body: &'static [u8]) -> std::io::Result<String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;

        tokio::spawn(async move {
            let (mut socket, _) = match listener.accept().await {
                Ok(connection) => connection,
                Err(err) => panic!("test server accept failed: {err}"),
            };
            let mut buffer = [0_u8; 1024];
            if let Err(err) = socket.read(&mut buffer).await {
                panic!("test server failed to read request: {err}");
            }

            if let Err(err) = socket
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\n\r\n")
                .await
            {
                panic!("test server failed to write headers: {err}");
            }
            if let Err(err) = socket.write_all(body).await {
                panic!("test server failed to write body: {err}");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        });

        Ok(format!("http://{addr}"))
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let pool = ConnectionPool::new();
        assert!(pool.is_ok());
    }

    #[tokio::test]
    async fn test_global_manager() {
        let manager = GlobalPoolManager::new();
        assert!(manager.is_ok());
        let config = BaseConfig {
            timeout: 1,
            ..Default::default()
        };
        let policy = GlobalPoolManager::new_for_provider("test", config)
            .unwrap()
            .policy
            .unwrap();
        let expected = Duration::from_secs(STREAMING_HEADER_TIMEOUT_SECS);
        assert_eq!(policy.streaming_header_timeout, expected);
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
    async fn test_streaming_clients_keep_legacy_and_unbounded_semantics() {
        let legacy = streaming_client();
        let stream1 = streaming_unbounded_client();
        let stream2 = streaming_unbounded_client();
        let global = global_client();

        assert!(Arc::ptr_eq(&legacy, &global));
        assert!(Arc::ptr_eq(&stream1, &stream2));
        assert!(!Arc::ptr_eq(&stream1, &global));
    }

    #[tokio::test]
    async fn test_streaming_send_times_out_before_headers() -> Result<(), Box<dyn std::error::Error>>
    {
        let url =
            delayed_response_url(Duration::from_millis(150), Duration::from_millis(0)).await?;

        let err = send_streaming_request_with_timeout(
            streaming_unbounded_client().get(url),
            Duration::from_millis(25),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, StreamingRequestError::HeaderTimeout { .. }));
        Ok(())
    }

    #[tokio::test]
    async fn test_streaming_send_timeout_does_not_bound_body()
    -> Result<(), Box<dyn std::error::Error>> {
        let url =
            delayed_response_url(Duration::from_millis(0), Duration::from_millis(150)).await?;

        let response = send_streaming_request_with_timeout(
            streaming_unbounded_client().get(url),
            Duration::from_millis(25),
        )
        .await?;
        let body = response.text().await?;

        assert_eq!(body, "hello");
        Ok(())
    }

    #[rustfmt::skip]
    #[tokio::test]
    async fn preserved_policy_requests_keep_timeout_variants()
    -> Result<(), Box<dyn std::error::Error>> {
        let url = delayed_response_url(Duration::from_millis(1_100), Duration::ZERO).await?;
        let config = BaseConfig {
            api_base: Some(url.clone()),
            endpoint_access: crate::core::net::ProviderEndpointAccess::PrivateNetwork,
            timeout: 1,
            ..Default::default()
        };
        let mut manager = GlobalPoolManager::new_for_provider("test", config)?;
        let error = manager
            .execute_request_preserving_endpoint_policy(&url, HttpMethod::GET, Vec::new(), None)
            .await
            .expect_err("ordinary request must time out");
        assert!(matches!(error, ProviderError::Timeout { .. }));

        let url = delayed_response_url(Duration::from_millis(100), Duration::ZERO).await?;
        manager = GlobalPoolManager::new_for_provider("test", BaseConfig { api_base: Some(url.clone()), endpoint_access: crate::core::net::ProviderEndpointAccess::PrivateNetwork, ..Default::default() })?;
        manager
            .policy
            .as_mut()
            .ok_or_else(|| std::io::Error::other("test manager must have endpoint policy"))?
            .streaming_header_timeout = Duration::from_millis(20);
        let error = manager
            .execute_streaming_request_preserving_endpoint_policy(
                &url,
                Vec::new(),
                serde_json::json!({}),
                "test",
            )
            .await
            .expect_err("streaming header request must time out");
        assert!(matches!(error, ProviderError::Timeout { .. }));
        Ok(())
    }

    #[tokio::test]
    async fn test_streaming_error_body_read_is_bounded() -> Result<(), Box<dyn std::error::Error>> {
        let url = delayed_error_body_url(Duration::from_millis(150)).await?;

        let response = send_streaming_request_with_timeout(
            streaming_unbounded_client().get(url),
            Duration::from_secs(1),
        )
        .await?;
        assert_eq!(
            response.status(),
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        );

        let err = read_streaming_error_body_with_limits(
            response,
            Duration::from_millis(25),
            STREAMING_ERROR_BODY_MAX_BYTES,
        )
        .await
        .unwrap_err();

        assert!(matches!(
            err,
            StreamingRequestError::ErrorBodyTimeout { .. }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn test_streaming_error_body_returns_at_exact_byte_cap()
    -> Result<(), Box<dyn std::error::Error>> {
        let url = error_body_then_stall_url(b"error").await?;

        let response = send_streaming_request_with_timeout(
            streaming_unbounded_client().get(url),
            Duration::from_secs(1),
        )
        .await?;

        let body =
            read_streaming_error_body_with_limits(response, Duration::from_millis(25), 5).await?;

        assert_eq!(body, "error");
        Ok(())
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
