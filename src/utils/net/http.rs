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
use reqwest::{Client, ClientBuilder, IntoUrl, Method, RequestBuilder, redirect};
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tracing::{debug, warn};

use crate::core::net::{
    ProviderEndpointAccess, ProviderEndpointPolicy, SsrfError, is_provider_endpoint_ip_allowed,
    validate_outbound_url_without_resolution,
};

/// DNS resolver that rejects private/reserved IP addresses at resolution time.
///
/// This mitigates DNS-rebinding attacks: even if a hostname resolves to a public IP
/// at config-validation time, every actual request re-validates the resolved address,
/// so a later rebind to an internal IP will be caught and rejected.
trait HostResolver: Send + Sync {
    fn resolve(&self, host: &str) -> io::Result<Vec<SocketAddr>>;
}

struct SystemHostResolver;

impl HostResolver for SystemHostResolver {
    fn resolve(&self, host: &str) -> io::Result<Vec<SocketAddr>> {
        (host, 0u16)
            .to_socket_addrs()
            .map(|addresses| addresses.collect())
    }
}

struct PolicyDnsResolver {
    access: ProviderEndpointAccess,
    resolver: Arc<dyn HostResolver>,
}

impl reqwest::dns::Resolve for PolicyDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        let resolver = Arc::clone(&self.resolver);
        let access = self.access;
        Box::pin(async move {
            let addrs = tokio::task::spawn_blocking(move || resolver.resolve(&host))
                .await
                .map_err(io::Error::other)??;
            let safe = validate_provider_addresses(access, addrs)?;

            Ok(Box::new(safe.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

fn validate_provider_addresses(
    access: ProviderEndpointAccess,
    addrs: Vec<SocketAddr>,
) -> io::Result<Vec<SocketAddr>> {
    if addrs.is_empty() {
        return Err(io::Error::other(
            "Host resolution returned no addresses (SSRF protection)",
        ));
    }
    if addrs
        .iter()
        .any(|addr| !is_provider_endpoint_ip_allowed(access, &addr.ip()))
    {
        return Err(io::Error::other(
            "Host resolves to a disallowed address (SSRF protection)",
        ));
    }
    Ok(addrs)
}

fn ssrf_safe_redirect_policy() -> redirect::Policy {
    redirect::Policy::custom(|attempt| {
        if let Err(error) = validate_outbound_url_without_resolution(attempt.url()) {
            return attempt.error(format!("Redirect target failed SSRF validation: {error}"));
        }

        redirect::Policy::limited(10).redirect(attempt)
    })
}

fn provider_redirect_policy(policy: ProviderEndpointPolicy) -> redirect::Policy {
    if policy.access() == ProviderEndpointAccess::PrivateNetwork {
        return redirect::Policy::none();
    }

    redirect::Policy::custom(move |attempt| {
        if let Err(error) = policy.validate_url_without_resolution(attempt.url()) {
            return attempt.error(format!("Redirect target failed SSRF validation: {error}"));
        }
        redirect::Policy::limited(10).redirect(attempt)
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderHttpClientError {
    #[error(transparent)]
    Endpoint(#[from] SsrfError),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

/// Request builder that cannot expose or replace its policy-bound HTTP client.
pub struct ProviderRequestBuilder {
    inner: RequestBuilder,
}

impl ProviderRequestBuilder {
    pub async fn send(self) -> Result<reqwest::Response, reqwest::Error> {
        self.inner.send().await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderClientMode {
    Request,
    Streaming,
    NoRedirect,
}

/// HTTP client that enforces one provider endpoint policy at every request boundary.
#[derive(Clone)]
pub struct ProviderHttpClient {
    client: Client,
    policy: ProviderEndpointPolicy,
}

impl std::fmt::Debug for ProviderHttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderHttpClient")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl ProviderHttpClient {
    pub fn new(
        policy: ProviderEndpointPolicy,
        timeout: Duration,
    ) -> Result<Self, ProviderHttpClientError> {
        Self::build(
            policy,
            timeout,
            ProviderClientMode::Request,
            Arc::new(SystemHostResolver),
        )
    }

    pub fn streaming(policy: ProviderEndpointPolicy) -> Result<Self, ProviderHttpClientError> {
        Self::build(
            policy,
            Duration::from_secs(0),
            ProviderClientMode::Streaming,
            Arc::new(SystemHostResolver),
        )
    }

    pub fn no_redirect(
        policy: ProviderEndpointPolicy,
        timeout: Duration,
    ) -> Result<Self, ProviderHttpClientError> {
        Self::build(
            policy,
            timeout,
            ProviderClientMode::NoRedirect,
            Arc::new(SystemHostResolver),
        )
    }

    fn build(
        policy: ProviderEndpointPolicy,
        timeout: Duration,
        mode: ProviderClientMode,
        resolver: Arc<dyn HostResolver>,
    ) -> Result<Self, ProviderHttpClientError> {
        let config = HttpClientPoolConfig::default();
        let builder = match mode {
            ProviderClientMode::Streaming => ClientBuilder::new()
                .pool_max_idle_per_host(config.pool_max_idle_per_host)
                .pool_idle_timeout(config.pool_idle_timeout)
                .connect_timeout(config.connect_timeout)
                .tcp_keepalive(config.tcp_keepalive)
                .tcp_nodelay(true)
                .user_agent(config.user_agent),
            ProviderClientMode::Request | ProviderClientMode::NoRedirect => {
                create_client_builder_with_config(timeout, &config)
            }
        };
        let redirect_policy = if mode == ProviderClientMode::NoRedirect {
            redirect::Policy::none()
        } else {
            provider_redirect_policy(policy.clone())
        };
        let client = builder
            .no_proxy()
            .dns_resolver(Arc::new(PolicyDnsResolver {
                access: policy.access(),
                resolver,
            }))
            .redirect(redirect_policy)
            .build()?;
        Ok(Self { client, policy })
    }

    pub fn request<U: IntoUrl>(
        &self,
        method: Method,
        url: U,
    ) -> Result<ProviderRequestBuilder, ProviderHttpClientError> {
        let url = url.into_url()?;
        self.policy.validate_url_without_resolution(&url)?;
        Ok(ProviderRequestBuilder {
            inner: self.client.request(method, url),
        })
    }

    pub fn get<U: IntoUrl>(
        &self,
        url: U,
    ) -> Result<ProviderRequestBuilder, ProviderHttpClientError> {
        self.request(Method::GET, url)
    }

    pub fn post<U: IntoUrl>(
        &self,
        url: U,
    ) -> Result<ProviderRequestBuilder, ProviderHttpClientError> {
        self.request(Method::POST, url)
    }
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
        .dns_resolver(Arc::new(PolicyDnsResolver {
            access: ProviderEndpointAccess::PublicOnly,
            resolver: Arc::new(SystemHostResolver),
        }))
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
    use std::collections::VecDeque;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct SequenceResolver {
        answers: Mutex<VecDeque<Vec<SocketAddr>>>,
    }

    impl SequenceResolver {
        fn new(answers: Vec<Vec<SocketAddr>>) -> Self {
            Self {
                answers: Mutex::new(answers.into()),
            }
        }
    }

    impl HostResolver for SequenceResolver {
        fn resolve(&self, _host: &str) -> io::Result<Vec<SocketAddr>> {
            self.answers
                .lock()
                .map_err(|_| io::Error::other("sequence resolver lock poisoned"))?
                .pop_front()
                .ok_or_else(|| io::Error::other("sequence resolver exhausted"))
        }
    }

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

        assert!(
            validate_provider_addresses(ProviderEndpointAccess::PublicOnly, addrs).is_err(),
            "a mixed DNS answer must fail as a whole"
        );
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
        let source = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let source_address = source.local_addr()?;
        let target = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let target_address = target.local_addr()?;

        let server = tokio::spawn(async move {
            let (mut stream, _) = source.accept().await?;
            let mut buffer = [0_u8; 1024];
            let bytes_read = stream.read(&mut buffer).await?;
            assert!(bytes_read > 0);

            let location = format!("http://{target_address}/private");
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
            .get(format!("http://{source_address}/redirect"))
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
        assert!(
            tokio::time::timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err(),
            "blocked redirect must not open the target socket"
        );
        server.await??;
        Ok(())
    }

    #[tokio::test]
    async fn provider_client_rechecks_dns_at_connection_time_without_opening_socket()
    -> Result<(), Box<dyn std::error::Error>> {
        for mode in [
            ProviderClientMode::Request,
            ProviderClientMode::Streaming,
            ProviderClientMode::NoRedirect,
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let url = reqwest::Url::parse(&format!("http://rebind.test:{}/v1", address.port()))?;
            let resolver = Arc::new(SequenceResolver::new(vec![
                vec![SocketAddr::from((
                    Ipv4Addr::new(93, 184, 216, 34),
                    address.port(),
                ))],
                vec![address],
            ]));
            let policy = ProviderEndpointPolicy::public_only();

            crate::core::net::ssrf_guard::validate_provider_endpoint_url_with_resolver(
                &url,
                policy.access(),
                |host, _port| {
                    resolver
                        .resolve(host)
                        .map(|addresses| addresses.into_iter().map(|addr| addr.ip()).collect())
                        .map_err(|error| SsrfError::HostResolutionFailed {
                            host: host.to_string(),
                            message: error.to_string(),
                        })
                },
            )?;

            let client = ProviderHttpClient::build(policy, Duration::from_secs(1), mode, resolver)?;
            let result = client.get(url)?.send().await;
            assert!(result.is_err(), "{mode:?} loopback rebind must fail");
            assert!(
                tokio::time::timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err(),
                "{mode:?} blocked rebind must not open the target socket"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn private_provider_connects_to_its_authority_but_does_not_follow_redirects()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let source_address = source.local_addr()?;
        let target = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let target_address = target.local_addr()?;
        let server = tokio::spawn(async move {
            let (mut stream, _) = source.accept().await?;
            let mut request = [0_u8; 1024];
            let bytes_read = stream.read(&mut request).await?;
            if bytes_read == 0 {
                return Err(io::Error::other("provider request ended before headers"));
            }
            let location = format!("http://{target_address}/private");
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await?;
            Ok::<(), io::Error>(())
        });
        let base_url = format!("http://{source_address}/v1");
        let policy = ProviderEndpointPolicy::for_base_url(
            ProviderEndpointAccess::PrivateNetwork,
            &base_url,
        )?;
        let client = ProviderHttpClient::new(policy, Duration::from_secs(1))?;
        assert!(
            client
                .get("http://169.254.169.254/latest/meta-data/")
                .is_err()
        );

        let response = client.get(base_url)?.send().await?;
        assert_eq!(response.status(), reqwest::StatusCode::FOUND);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err(),
            "private-network mode must not follow redirects"
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
