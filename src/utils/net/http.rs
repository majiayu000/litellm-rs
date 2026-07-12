//! Shared HTTP client for optimal connection pooling
//!
//! This module provides a high-performance shared HTTP client with connection reuse.
//! Using a shared client avoids the overhead of creating new connection pools and
//! DNS resolution caches for each request.
//!
//! # Performance Benefits
//!
//! - **Connection Reuse**: Keeps TCP connections alive across requests
//! - **DNS Caching**: Avoids repeated DNS lookups
//! - **HTTP/2 Multiplexing**: Multiple requests over a single connection
//! - **Reduced Latency**: 20-50% improvement in request latency
//!
//! # Usage
//!
//! ```rust,no_run
//! # use litellm_rs::utils::net::http::get_shared_client;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = get_shared_client();
//! let response = client.get("https://api.openai.com").send().await?;
//! # Ok(())
//! # }
//! ```

use dashmap::DashMap;
use reqwest::{Client, ClientBuilder, redirect};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tracing::{debug, warn};

use crate::core::net::{is_private_or_reserved_ip, validate_outbound_url_without_resolution};

/// DNS resolver that rejects private/reserved IP addresses at resolution time.
///
/// This mitigates DNS-rebinding attacks: even if a hostname resolves to a public IP
/// at config-validation time, every actual request re-validates the resolved address,
/// so a later rebind to an internal IP will be caught and rejected.
struct SsrfSafeDnsResolver;

impl reqwest::dns::Resolve for SsrfSafeDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addrs: std::io::Result<Vec<SocketAddr>> = tokio::task::spawn_blocking(move || {
                (host.as_str(), 0u16)
                    .to_socket_addrs()
                    .map(|iter| iter.collect())
            })
            .await
            .map_err(std::io::Error::other)?;

            let addrs = addrs?;
            let safe = filter_ssrf_safe_addresses(addrs);

            if safe.is_empty() {
                return Err(
                    "Host resolves to private/reserved IP address (SSRF protection)"
                        .to_string()
                        .into(),
                );
            }

            Ok(Box::new(safe.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

fn filter_ssrf_safe_addresses(addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    addrs
        .into_iter()
        .filter(|addr| !is_private_or_reserved_ip(&addr.ip()))
        .collect()
}

fn ssrf_safe_redirect_policy() -> redirect::Policy {
    redirect::Policy::custom(|attempt| {
        if let Err(error) = validate_outbound_url_without_resolution(attempt.url()) {
            return attempt.error(format!("Redirect target failed SSRF validation: {error}"));
        }

        redirect::Policy::limited(10).redirect(attempt)
    })
}

/// Configuration for the HTTP client pool
#[derive(Debug, Clone)]
pub struct HttpClientPoolConfig {
    /// Maximum idle connections per host
    pub pool_max_idle_per_host: usize,
    /// Idle connection timeout
    pub pool_idle_timeout: Duration,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// TCP keepalive interval
    pub tcp_keepalive: Duration,
    /// User agent string
    pub user_agent: &'static str,
}

impl Default for HttpClientPoolConfig {
    fn default() -> Self {
        Self {
            pool_max_idle_per_host: 100, // Increased for high throughput
            pool_idle_timeout: Duration::from_secs(90),
            connect_timeout: Duration::from_secs(10),
            tcp_keepalive: Duration::from_secs(60),
            user_agent: "LiteLLM-RS/0.1.0",
        }
    }
}

/// Shared HTTP client instance with optimized settings
static SHARED_HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Timeout-specific client cache (keyed by milliseconds)
static TIMEOUT_CLIENT_CACHE: OnceLock<DashMap<u64, Arc<Client>>> = OnceLock::new();

/// Timeout-specific SSRF-safe client cache (keyed by milliseconds)
static SSRF_SAFE_TIMEOUT_CLIENT_CACHE: OnceLock<DashMap<u64, Arc<Client>>> = OnceLock::new();

/// Timeout-specific SSRF-safe client cache for requests that must observe redirects.
static SSRF_SAFE_NO_REDIRECT_TIMEOUT_CLIENT_CACHE: OnceLock<DashMap<u64, Arc<Client>>> =
    OnceLock::new();

/// Create a reqwest client builder with unified pool/timeout defaults.
pub fn create_client_builder_with_config(
    timeout: Duration,
    config: &HttpClientPoolConfig,
) -> ClientBuilder {
    ClientBuilder::new()
        // Connection pool settings
        .pool_max_idle_per_host(config.pool_max_idle_per_host)
        .pool_idle_timeout(config.pool_idle_timeout)
        // Request timeouts
        .timeout(timeout)
        .connect_timeout(config.connect_timeout)
        // TCP optimizations
        .tcp_keepalive(config.tcp_keepalive)
        .tcp_nodelay(true)
        // User agent
        .user_agent(config.user_agent)
}

/// Create a reqwest client builder with default pool configuration.
pub fn create_client_builder(timeout: Duration) -> ClientBuilder {
    create_client_builder_with_config(timeout, &HttpClientPoolConfig::default())
}

/// Get the shared HTTP client instance
///
/// This client uses a default timeout of 30 seconds. For custom timeouts,
/// use `get_client_with_timeout`.
pub fn get_shared_client() -> &'static Client {
    SHARED_HTTP_CLIENT.get_or_init(|| {
        debug!("Initializing shared HTTP client with optimized settings");
        create_optimized_client(Duration::from_secs(30))
    })
}

/// Get or create a client with a specific timeout
///
/// Clients are cached by timeout duration (in milliseconds) to avoid creating
/// multiple clients with the same configuration.
pub fn get_client_with_timeout(timeout: Duration) -> Arc<Client> {
    let cache = TIMEOUT_CLIENT_CACHE.get_or_init(DashMap::new);
    let timeout_millis = timeout.as_millis().min(u64::MAX as u128) as u64;

    cache
        .entry(timeout_millis)
        .or_insert_with(|| {
            debug!(timeout_millis, "Creating cached HTTP client for timeout");
            Arc::new(create_optimized_client(timeout))
        })
        .clone()
}

/// Get or create a client with a specific timeout, returning errors on failure
///
/// This is useful when caller error semantics must be preserved.
pub fn get_client_with_timeout_fallible(timeout: Duration) -> Result<Arc<Client>, reqwest::Error> {
    let cache = TIMEOUT_CLIENT_CACHE.get_or_init(DashMap::new);
    let timeout_millis = timeout.as_millis().min(u64::MAX as u128) as u64;

    if let Some(existing) = cache.get(&timeout_millis) {
        return Ok(existing.clone());
    }

    let client = Arc::new(create_custom_client(timeout)?);
    cache.insert(timeout_millis, client.clone());
    Ok(client)
}

/// Create an optimized HTTP client with the given timeout
fn create_optimized_client(timeout: Duration) -> Client {
    let config = HttpClientPoolConfig::default();

    create_client_builder_with_config(timeout, &config)
        .build()
        .unwrap_or_else(|e| {
            warn!(
                "Failed to create optimized HTTP client, falling back to default: {}",
                e
            );
            Client::builder()
                .build()
                .expect("default HTTP client should build")
        })
}

/// Create a custom HTTP client with specific timeout and pool configuration.
pub fn create_custom_client_with_config(
    timeout: Duration,
    config: &HttpClientPoolConfig,
) -> Result<Client, reqwest::Error> {
    create_client_builder_with_config(timeout, config).build()
}

/// Create a custom HTTP client with specific timeout
///
/// Use this when you need a one-off client that won't be reused.
/// For reusable clients, prefer `get_client_with_timeout`.
pub fn create_custom_client(timeout: Duration) -> Result<Client, reqwest::Error> {
    create_custom_client_with_config(timeout, &HttpClientPoolConfig::default())
}

/// Create an HTTP client for long-running SSE streams.
///
/// Unlike `create_custom_client`, this client does not set a total request timeout
/// so streams lasting longer than the configured timeout value won't be cut off.
/// Only the initial TCP connection is time-bounded via `connect_timeout`.
pub fn create_streaming_client() -> Result<Client, reqwest::Error> {
    let config = HttpClientPoolConfig::default();
    ClientBuilder::new()
        .pool_max_idle_per_host(config.pool_max_idle_per_host)
        .pool_idle_timeout(config.pool_idle_timeout)
        .connect_timeout(config.connect_timeout)
        .tcp_keepalive(config.tcp_keepalive)
        .tcp_nodelay(true)
        .user_agent(config.user_agent)
        .build()
}

/// Get or create an HTTP client with SSRF-safe DNS resolution for the given timeout.
///
/// Unlike `get_client_with_timeout_fallible`, this client installs `SsrfSafeDnsResolver`
/// so every request re-validates the resolved IP against private/reserved ranges.
/// Use this for providers whose endpoint URL is user-controlled to prevent DNS-rebinding attacks.
pub fn get_ssrf_safe_client_with_timeout_fallible(
    timeout: Duration,
) -> Result<Arc<Client>, reqwest::Error> {
    let cache = SSRF_SAFE_TIMEOUT_CLIENT_CACHE.get_or_init(DashMap::new);
    let timeout_millis = timeout.as_millis().min(u64::MAX as u128) as u64;

    if let Some(existing) = cache.get(&timeout_millis) {
        return Ok(existing.clone());
    }

    let client = Arc::new(create_ssrf_safe_client(
        timeout,
        ssrf_safe_redirect_policy(),
    )?);
    cache.insert(timeout_millis, client.clone());
    Ok(client)
}

/// Get or create an SSRF-safe HTTP client that returns redirect responses unchanged.
pub(crate) fn get_ssrf_safe_no_redirect_client_with_timeout_fallible(
    timeout: Duration,
) -> Result<Arc<Client>, reqwest::Error> {
    let cache = SSRF_SAFE_NO_REDIRECT_TIMEOUT_CLIENT_CACHE.get_or_init(DashMap::new);
    let timeout_millis = timeout.as_millis().min(u64::MAX as u128) as u64;

    if let Some(existing) = cache.get(&timeout_millis) {
        return Ok(existing.clone());
    }

    let client = Arc::new(create_ssrf_safe_client(timeout, redirect::Policy::none())?);
    cache.insert(timeout_millis, client.clone());
    Ok(client)
}

fn create_ssrf_safe_client(
    timeout: Duration,
    redirect_policy: redirect::Policy,
) -> Result<Client, reqwest::Error> {
    create_client_builder_with_config(timeout, &HttpClientPoolConfig::default())
        .no_proxy()
        .dns_resolver(Arc::new(SsrfSafeDnsResolver))
        .redirect(redirect_policy)
        .build()
}

/// Create a custom HTTP client with specific timeout and default headers
pub fn create_custom_client_with_headers(
    timeout: Duration,
    default_headers: reqwest::header::HeaderMap,
) -> Result<Client, reqwest::Error> {
    create_client_builder(timeout)
        .default_headers(default_headers)
        .build()
}

/// Get statistics about the client cache
pub fn get_cache_stats() -> HttpClientCacheStats {
    let cache = TIMEOUT_CLIENT_CACHE.get_or_init(DashMap::new);
    HttpClientCacheStats {
        cached_clients: cache.len(),
        timeout_configs: cache.iter().map(|e| *e.key()).collect(),
    }
}

/// Statistics about the HTTP client cache
#[derive(Debug, Clone)]
pub struct HttpClientCacheStats {
    /// Number of cached clients
    pub cached_clients: usize,
    /// List of cached timeout configurations (in milliseconds)
    pub timeout_configs: Vec<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn test_shared_client_creation() {
        let client = get_shared_client();
        // Just verify we can get the client without panicking
        assert!(std::ptr::addr_of!(*client) == std::ptr::addr_of!(*get_shared_client()));
    }

    #[test]
    fn test_custom_client_creation() {
        let client = create_custom_client(Duration::from_secs(15));
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_with_timeout_caching() {
        let client1 = get_client_with_timeout(Duration::from_secs(60));
        let client2 = get_client_with_timeout(Duration::from_secs(60));

        // Same timeout should return the same cached client
        assert!(Arc::ptr_eq(&client1, &client2));

        // Different timeout should return different client
        let client3 = get_client_with_timeout(Duration::from_secs(120));
        assert!(!Arc::ptr_eq(&client1, &client3));
    }

    #[test]
    fn test_client_with_timeout_fallible_caching() {
        let client1 = get_client_with_timeout_fallible(Duration::from_millis(1500)).unwrap();
        let client2 = get_client_with_timeout_fallible(Duration::from_millis(1500)).unwrap();

        assert!(Arc::ptr_eq(&client1, &client2));
    }

    #[test]
    fn test_ssrf_safe_dns_filter_rejects_private_and_reserved_addresses() {
        let addrs = vec![
            SocketAddr::from((Ipv4Addr::new(93, 184, 216, 34), 443)),
            SocketAddr::from((Ipv4Addr::new(198, 18, 0, 1), 443)),
            SocketAddr::from((Ipv4Addr::new(224, 0, 0, 1), 443)),
            SocketAddr::from((Ipv4Addr::new(240, 0, 0, 1), 443)),
        ];

        let safe = filter_ssrf_safe_addresses(addrs);

        assert_eq!(safe.len(), 1);
        assert_eq!(safe[0].ip(), IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)));
    }

    #[test]
    fn test_ssrf_safe_client_with_timeout_fallible_caching() {
        let client1 = match get_ssrf_safe_client_with_timeout_fallible(Duration::from_millis(1500))
        {
            Ok(client) => client,
            Err(error) => panic!("SSRF-safe client should build: {error}"),
        };
        let client2 = match get_ssrf_safe_client_with_timeout_fallible(Duration::from_millis(1500))
        {
            Ok(client) => client,
            Err(error) => panic!("SSRF-safe client should build: {error}"),
        };

        assert!(Arc::ptr_eq(&client1, &client2));
    }

    #[test]
    fn test_ssrf_safe_no_redirect_client_with_timeout_fallible_caching() {
        let client1 = match get_ssrf_safe_no_redirect_client_with_timeout_fallible(
            Duration::from_millis(1500),
        ) {
            Ok(client) => client,
            Err(error) => panic!("SSRF-safe no-redirect client should build: {error}"),
        };
        let client2 = match get_ssrf_safe_no_redirect_client_with_timeout_fallible(
            Duration::from_millis(1500),
        ) {
            Ok(client) => client,
            Err(error) => panic!("SSRF-safe no-redirect client should build: {error}"),
        };

        assert!(Arc::ptr_eq(&client1, &client2));
    }

    #[tokio::test]
    async fn test_ssrf_safe_redirect_policy_rejects_private_redirect_targets()
    -> Result<(), Box<dyn std::error::Error>> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await?;
            let mut buffer = [0_u8; 1024];
            let bytes_read = stream.read(&mut buffer).await?;
            assert!(bytes_read > 0);

            let location = format!("http://127.0.0.1:{}/private", address.port());
            let body = "redirect";
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await?;
            Ok::<(), std::io::Error>(())
        });

        let client = ClientBuilder::new()
            .redirect(ssrf_safe_redirect_policy())
            .build()?;
        let result = client
            .get(format!("http://127.0.0.1:{}/redirect", address.port()))
            .send()
            .await;

        let error = match result {
            Ok(response) => panic!(
                "private redirect target should be rejected, got status {}",
                response.status()
            ),
            Err(error) => error,
        };
        assert!(error.is_redirect(), "{error:?}");
        assert!(
            format!("{error:?}").contains("SSRF validation"),
            "{error:?}"
        );
        server.await??;
        Ok(())
    }

    #[test]
    fn test_cache_stats() {
        // Ensure some clients are cached
        let _ = get_client_with_timeout(Duration::from_secs(30));
        let _ = get_client_with_timeout(Duration::from_secs(45));

        let stats = get_cache_stats();
        assert!(stats.cached_clients >= 2);
        assert!(stats.timeout_configs.contains(&30_000));
        assert!(stats.timeout_configs.contains(&45_000));
    }

    #[test]
    fn test_pool_config_defaults() {
        let config = HttpClientPoolConfig::default();
        assert_eq!(config.pool_max_idle_per_host, 100);
        assert_eq!(config.pool_idle_timeout, Duration::from_secs(90));
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.tcp_keepalive, Duration::from_secs(60));
        assert_eq!(config.user_agent, "LiteLLM-RS/0.1.0");
    }
}
