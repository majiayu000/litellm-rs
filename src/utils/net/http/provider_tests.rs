use super::*;
use crate::core::providers::base::{BaseConfig, BaseHttpClient};
use crate::core::providers::unified_provider::ProviderError;
use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use std::task::{Context, Poll};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type ConnectorBoxError = Box<dyn std::error::Error + Send + Sync>;

fn map_preserved_error(error: reqwest::Error) -> ProviderError {
    BaseHttpClient::new_for_provider("test", BaseConfig::default())
        .expect("test mapper should build")
        .map_preserved_request_error(error)
}

fn error_chain_contains(error: &(dyn std::error::Error + 'static), expected: &str) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if error.to_string().contains(expected) {
            return true;
        }
        current = error.source();
    }
    false
}

#[derive(Clone)]
struct TripwireConnectorLayer {
    address: SocketAddr,
}

impl<S> tower_layer::Layer<S> for TripwireConnectorLayer {
    type Service = TripwireConnector<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TripwireConnector {
            _inner: inner,
            address: self.address,
        }
    }
}

#[derive(Clone)]
struct TripwireConnector<S> {
    _inner: S,
    address: SocketAddr,
}

impl<S, Request> tower_service::Service<Request> for TripwireConnector<S>
where
    S: tower_service::Service<Request>,
    S::Response: Send + 'static,
{
    type Response = S::Response;
    type Error = ConnectorBoxError;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _request: Request) -> Self::Future {
        let address = self.address;
        Box::pin(async move {
            let _stream = tokio::net::TcpStream::connect(address)
                .await
                .map_err(ConnectorBoxError::from)?;
            Err(ConnectorBoxError::from(io::Error::other(
                "connector tripwire invoked",
            )))
        })
    }
}

struct SequenceResolver {
    answers: Mutex<VecDeque<Vec<SocketAddr>>>,
    queries: Mutex<Vec<String>>,
}

impl SequenceResolver {
    fn new(answers: Vec<Vec<SocketAddr>>) -> Self {
        Self {
            answers: Mutex::new(answers.into()),
            queries: Mutex::new(Vec::new()),
        }
    }

    fn remaining_answers(&self) -> io::Result<usize> {
        self.answers
            .lock()
            .map(|answers| answers.len())
            .map_err(|_| io::Error::other("sequence resolver lock poisoned"))
    }

    fn queries(&self) -> io::Result<Vec<String>> {
        self.queries
            .lock()
            .map(|queries| queries.clone())
            .map_err(|_| io::Error::other("sequence resolver query lock poisoned"))
    }
}

impl HostResolver for SequenceResolver {
    fn resolve(&self, host: &str) -> io::Result<Vec<SocketAddr>> {
        self.queries
            .lock()
            .map_err(|_| io::Error::other("sequence resolver query lock poisoned"))?
            .push(host.to_string());
        self.answers
            .lock()
            .map_err(|_| io::Error::other("sequence resolver lock poisoned"))?
            .pop_front()
            .ok_or_else(|| io::Error::other("sequence resolver exhausted"))
    }
}

struct TripwireAfterPolicyResolver<R> {
    inner: Arc<R>,
    tripwire: SocketAddr,
}

impl<R> reqwest::dns::Resolve for TripwireAfterPolicyResolver<R>
where
    R: reqwest::dns::Resolve + 'static,
{
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let resolving = self.inner.resolve(name);
        let tripwire = self.tripwire;
        Box::pin(async move {
            drop(resolving.await?);
            Ok(Box::new(std::iter::once(tripwire)) as reqwest::dns::Addrs)
        })
    }
}

async fn assert_listener_did_not_accept(listener: &tokio::net::TcpListener, context: &str) {
    match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
        Err(_) => {}
        Ok(Ok((_stream, peer))) => panic!("{context}: unexpectedly accepted {peer}"),
        Ok(Err(error)) => panic!("{context}: listener failed before timeout: {error}"),
    }
}

async fn assert_public_rebind_is_blocked(
    mode: ProviderClientMode,
    blocked_ip: IpAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let tripwire = listener.local_addr()?;
    let port = tripwire.port();
    let url = reqwest::Url::parse(&format!("http://rebind.test:{port}/v1"))?;
    let resolver = Arc::new(SequenceResolver::new(vec![
        vec![SocketAddr::from((Ipv4Addr::new(93, 184, 216, 34), port))],
        vec![SocketAddr::new(blocked_ip, port)],
    ]));
    let policy = ProviderEndpointPolicy::public_only();
    assert!(
        validate_provider_addresses(
            ProviderEndpointAccess::PublicOnly,
            vec![SocketAddr::new(blocked_ip, port)],
        )
        .is_err(),
        "the classifier must reject the specific rebinding address {blocked_ip}"
    );

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
    assert_eq!(
        resolver.remaining_answers()?,
        1,
        "configuration validation must consume only the public answer"
    );

    let client = ProviderHttpClient::build_with_rebinding_tripwire_for_test(
        policy,
        Duration::from_secs(1),
        mode,
        resolver.clone(),
        tripwire,
    )?;
    let result = tokio::time::timeout(Duration::from_millis(250), client.get(url)?.send()).await;
    assert_listener_did_not_accept(
        &listener,
        &format!("{mode:?} rebind to {blocked_ip} must not open the target socket"),
    )
    .await;
    let error = match result {
        Ok(Err(error)) => error,
        Ok(Ok(_)) => panic!("{mode:?} must reject connection-time rebind to {blocked_ip}"),
        Err(_) => panic!("{mode:?} did not reject connection-time rebind to {blocked_ip}"),
    };
    assert!(
        error_chain_contains(
            &error,
            "Host resolves to a disallowed address (SSRF protection)"
        ),
        "{mode:?} must fail in the DNS policy for {blocked_ip}, got {error:?}"
    );
    assert!(reqwest_error_is_endpoint_policy(&error), "{error:?}");
    let mapped = map_preserved_error(error);
    assert!(matches!(mapped, ProviderError::Configuration { .. }));
    assert!(!mapped.to_string().contains(&blocked_ip.to_string()));
    assert_eq!(
        resolver.remaining_answers()?,
        0,
        "connection attempt must consume and validate the rebinding answer"
    );
    assert_eq!(
        resolver.queries()?,
        ["rebind.test", "rebind.test"],
        "the injected resolver must exclusively serve validation and connection"
    );
    Ok(())
}

impl ProviderHttpClient {
    pub(crate) fn build_with_dns_resolver_for_test<R: reqwest::dns::Resolve + 'static>(
        policy: ProviderEndpointPolicy,
        timeout: Duration,
        no_redirect: bool,
        resolver: Arc<R>,
    ) -> Result<Self, ProviderHttpClientError> {
        let mode = if no_redirect {
            ProviderClientMode::NoRedirect
        } else {
            ProviderClientMode::Request
        };
        let client = Arc::new(Self::build_client_with_dns_resolver(
            &policy, timeout, mode, resolver,
        )?);
        Ok(Self { client, policy })
    }

    pub(crate) async fn build_public_then_private_tripwire_for_test(
        blocked_address: SocketAddr,
        tripwire: SocketAddr,
    ) -> Result<Self, ProviderHttpClientError> {
        let policy = ProviderEndpointPolicy::public_only();
        let mut answers = vec![vec![blocked_address]; 3];
        answers.insert(
            0,
            vec![SocketAddr::from(([93, 184, 216, 34], tripwire.port()))],
        );
        let resolver = Arc::new(SequenceResolver::new(answers));
        let priming_resolver = PolicyDnsResolver {
            access: policy.access(),
            resolver: resolver.clone(),
        };
        drop(
            reqwest::dns::Resolve::resolve(
                &priming_resolver,
                "rebind.test".parse().expect("valid test hostname"),
            )
            .await
            .expect("public priming answer must pass endpoint policy"),
        );
        Self::build_with_rebinding_tripwire_for_test(
            policy,
            Duration::from_secs(1),
            ProviderClientMode::NoRedirect,
            resolver,
            tripwire,
        )
    }

    fn build_with_connector_tripwire_for_test(
        policy: ProviderEndpointPolicy,
        address: SocketAddr,
    ) -> Result<Self, ProviderHttpClientError> {
        let client = ClientBuilder::new()
            .no_proxy()
            .connector_layer(TripwireConnectorLayer { address })
            .build()?;
        Ok(Self {
            client: Arc::new(client),
            policy,
        })
    }

    fn build_with_rebinding_tripwire_for_test(
        policy: ProviderEndpointPolicy,
        timeout: Duration,
        mode: ProviderClientMode,
        resolver: Arc<dyn HostResolver>,
        tripwire: SocketAddr,
    ) -> Result<Self, ProviderHttpClientError> {
        let policy_resolver = Arc::new(PolicyDnsResolver {
            access: policy.access(),
            resolver,
        });
        let resolver = Arc::new(TripwireAfterPolicyResolver {
            inner: policy_resolver,
            tripwire,
        });
        let client = Arc::new(Self::build_client_with_dns_resolver(
            &policy, timeout, mode, resolver,
        )?);
        Ok(Self { client, policy })
    }
}

#[test]
fn provider_clients_reuse_pools_for_identical_configurations()
-> Result<(), Box<dyn std::error::Error>> {
    let policy = ProviderEndpointPolicy::public_only();
    let timeout = Duration::from_secs(37);

    let first = ProviderHttpClient::new(policy.clone(), timeout)?;
    let second = ProviderHttpClient::new(policy.clone(), timeout)?;
    assert!(Arc::ptr_eq(&first.client, &second.client));

    let streaming_first = ProviderHttpClient::streaming(policy.clone())?;
    let streaming_second = ProviderHttpClient::streaming(policy.clone())?;
    assert!(Arc::ptr_eq(
        &streaming_first.client,
        &streaming_second.client
    ));

    let no_redirect_first = ProviderHttpClient::no_redirect(policy.clone(), timeout)?;
    let no_redirect_second = ProviderHttpClient::no_redirect(policy, timeout)?;
    assert!(Arc::ptr_eq(
        &no_redirect_first.client,
        &no_redirect_second.client
    ));
    assert!(!Arc::ptr_eq(&first.client, &streaming_first.client));
    assert!(!Arc::ptr_eq(&first.client, &no_redirect_first.client));
    Ok(())
}

#[test]
fn security_evidence_private_client_cache_isolated_by_authority()
-> Result<(), Box<dyn std::error::Error>> {
    let timeout = Duration::from_secs(38);
    let first_policy = ProviderEndpointPolicy::for_base_url(
        ProviderEndpointAccess::PrivateNetwork,
        "http://127.0.0.1:11434",
    )?;
    let second_policy = ProviderEndpointPolicy::for_base_url(
        ProviderEndpointAccess::PrivateNetwork,
        "http://127.0.0.1:11435",
    )?;
    let first = ProviderHttpClient::new(first_policy.clone(), timeout)?;
    let identical = ProviderHttpClient::new(first_policy, timeout)?;
    let other_authority = ProviderHttpClient::new(second_policy, timeout)?;
    let public = ProviderHttpClient::new(ProviderEndpointPolicy::public_only(), timeout)?;

    assert!(Arc::ptr_eq(&first.client, &identical.client));
    assert!(!Arc::ptr_eq(&first.client, &other_authority.client));
    assert!(!Arc::ptr_eq(&first.client, &public.client));
    Ok(())
}

#[test]
fn security_evidence_shared_dns_filter_rejects_platform_metadata_encodings()
-> Result<(), Box<dyn std::error::Error>> {
    for ip in [
        "168.63.129.16",
        "::ffff:168.63.129.16",
        "64:ff9b::a83f:8110",
    ] {
        let address = SocketAddr::new(ip.parse()?, 80);
        assert!(
            validate_provider_addresses(ProviderEndpointAccess::PublicOnly, vec![address]).is_err(),
            "metadata destination {ip} must be rejected"
        );
    }
    Ok(())
}

#[test]
fn security_evidence_private_dns_filter_allows_only_explicit_private_network_ranges()
-> Result<(), Box<dyn std::error::Error>> {
    for ip in [
        "127.0.0.1",
        "10.0.0.1",
        "172.16.0.1",
        "192.168.0.1",
        "::1",
        "fd00::1",
    ] {
        let address = SocketAddr::new(ip.parse()?, 80);
        assert!(
            validate_provider_addresses(ProviderEndpointAccess::PrivateNetwork, vec![address])
                .is_ok(),
            "private-network destination {ip} must be allowed"
        );
    }

    for ip in [
        "100.64.0.1",
        "168.63.129.16",
        "169.254.169.254",
        "198.18.0.1",
        "203.0.113.1",
        "fd00:ec2::254",
        "fe80::1",
    ] {
        let address = SocketAddr::new(ip.parse()?, 80);
        assert!(
            validate_provider_addresses(ProviderEndpointAccess::PrivateNetwork, vec![address])
                .is_err(),
            "permanently blocked destination {ip} must remain rejected"
        );
    }
    Ok(())
}

#[test]
fn provider_request_builder_supports_provider_request_configuration()
-> Result<(), Box<dyn std::error::Error>> {
    let policy = ProviderEndpointPolicy::for_base_url(
        ProviderEndpointAccess::PrivateNetwork,
        "http://127.0.0.1:11434",
    )?;
    let client = ProviderHttpClient::new(policy, Duration::from_secs(1))?;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-provider-mode",
        reqwest::header::HeaderValue::from_static("test"),
    );

    let _request = client
        .post("http://127.0.0.1:11434/v1/chat")?
        .header("x-request-id", "request-1")
        .headers(headers)
        .basic_auth("user", Some("password"))
        .bearer_auth("token")
        .query(&[("stream", "false")])
        .body("raw body")
        .timeout(Duration::from_secs(2))
        .version(reqwest::Version::HTTP_11)
        .form(&[("form", "value")])
        .json(&serde_json::json!({"model": "test"}))
        .multipart(reqwest::multipart::Form::new().text("field", "value"));
    Ok(())
}

#[test]
fn provider_request_builder_has_no_public_raw_client_escape_hatch() {
    let source = include_str!("../http.rs");
    for forbidden in [
        "pub fn build_split",
        "pub fn into_inner",
        "pub fn inner",
        "Result<RequestBuilder",
    ] {
        assert!(!source.contains(forbidden), "forbidden API: {forbidden}");
    }
}

#[tokio::test]
async fn security_evidence_initial_blocked_literals_fail_before_dns()
-> Result<(), Box<dyn std::error::Error>> {
    for blocked_ip in [
        "127.0.0.1",
        "10.0.0.1",
        "169.254.169.254",
        "168.63.129.16",
        "100.64.0.1",
        "198.18.0.1",
        "203.0.113.1",
        "fd00::1",
        "fd00:ec2::254",
        "::ffff:168.63.129.16",
        "fe80::1",
    ] {
        let resolver = Arc::new(SequenceResolver::new(vec![vec![SocketAddr::from((
            Ipv4Addr::new(93, 184, 216, 34),
            80,
        ))]]));
        let client = ProviderHttpClient::build(
            ProviderEndpointPolicy::public_only(),
            Duration::from_secs(1),
            ProviderClientMode::Request,
            resolver.clone(),
        )?;
        let target = SocketAddr::new(blocked_ip.parse()?, 80);
        assert!(
            client.get(format!("http://{target}/v1")).is_err(),
            "public-only request must reject literal {blocked_ip}"
        );
        assert_eq!(
            resolver.remaining_answers()?,
            1,
            "blocked literal {blocked_ip} must fail before DNS"
        );
        assert!(
            resolver.queries()?.is_empty(),
            "blocked literal {blocked_ip} must not invoke the resolver"
        );
    }
    Ok(())
}

#[tokio::test]
async fn security_evidence_initial_loopback_literal_does_not_reach_exact_listener()
-> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let target = listener.local_addr()?;
    let client = ProviderHttpClient::new(
        ProviderEndpointPolicy::public_only(),
        Duration::from_secs(1),
    )?;

    assert!(client.get(format!("http://{target}/v1")).is_err());
    assert_listener_did_not_accept(
        &listener,
        "rejected loopback literal must not open its exact target socket",
    )
    .await;
    Ok(())
}

#[tokio::test]
async fn security_evidence_metadata_literals_do_not_reach_connector_tripwire()
-> Result<(), Box<dyn std::error::Error>> {
    for metadata_ip in [
        "169.254.169.254",
        "168.63.129.16",
        "fd00:ec2::254",
        "::ffff:168.63.129.16",
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let client = ProviderHttpClient::build_with_connector_tripwire_for_test(
            ProviderEndpointPolicy::public_only(),
            listener.local_addr()?,
        )?;
        let target = SocketAddr::new(metadata_ip.parse()?, 80);

        if let Ok(request) = client.get(format!("http://{target}/latest/meta-data/")) {
            assert!(
                request.send().await.is_err(),
                "connector tripwire must terminate an unsafe metadata request"
            );
        }
        assert_listener_did_not_accept(
            &listener,
            &format!("metadata literal {metadata_ip} must not reach the connector"),
        )
        .await;
    }
    Ok(())
}

#[tokio::test]
async fn security_evidence_metadata_hostname_fails_before_dns_or_socket()
-> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let resolver = Arc::new(SequenceResolver::new(vec![vec![address]]));
    let client = ProviderHttpClient::build(
        ProviderEndpointPolicy::public_only(),
        Duration::from_secs(1),
        ProviderClientMode::Request,
        resolver.clone(),
    )?;

    assert!(
        client
            .get(format!("http://metadata.goog:{}/v1", address.port()))
            .is_err()
    );
    let remaining_answers = resolver.remaining_answers()?;
    assert_eq!(remaining_answers, 1, "metadata URL must not invoke DNS");
    assert!(resolver.queries()?.is_empty());
    assert_listener_did_not_accept(&listener, "metadata URL must not open the target socket").await;
    Ok(())
}

#[tokio::test]
async fn security_evidence_public_redirect_to_private_literal_does_not_reach_target()
-> Result<(), Box<dyn std::error::Error>> {
    let source = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let source_address = source.local_addr()?;
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let target_address = target.local_addr()?;
    let server = tokio::spawn(async move {
        let (mut stream, _) = source.accept().await?;
        let mut request = [0_u8; 1024];
        if stream.read(&mut request).await? == 0 {
            return Err(io::Error::other("request ended before headers"));
        }
        stream
            .write_all(
                format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .await?;
        Ok::<(), io::Error>(())
    });
    let client = ClientBuilder::new()
        .no_proxy()
        .redirect(provider_redirect_policy(
            ProviderEndpointPolicy::public_only(),
        ))
        .build()?;

    let error = client
        .get(format!("http://{source_address}/v1"))
        .send()
        .await
        .expect_err("private redirect target must fail");
    assert!(reqwest_error_is_endpoint_policy(&error), "{error:?}");
    let mapped = map_preserved_error(error);
    assert!(matches!(mapped, ProviderError::Configuration { .. }));
    assert!(!mapped.to_string().contains(&target_address.to_string()));
    assert_listener_did_not_accept(&target, "provider redirect must not open the target socket")
        .await;
    server.await??;
    Ok(())
}

#[tokio::test]
async fn ordinary_redirect_loop_is_not_an_endpoint_policy_error()
-> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}/loop");
    let redirect_url = url.clone();
    let server = tokio::spawn(async move {
        for _ in 0..4 {
            let accepted = tokio::time::timeout(Duration::from_secs(1), listener.accept()).await;
            let Ok(Ok((mut stream, _))) = accepted else {
                break;
            };
            let mut request = [0_u8; 1024];
            if stream.read(&mut request).await? == 0 {
                return Err(io::Error::other("request ended before headers"));
            }
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: {redirect_url}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await?;
        }
        Ok::<(), io::Error>(())
    });
    let client = ClientBuilder::new()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::limited(2))
        .build()?;
    let error = client
        .get(url)
        .send()
        .await
        .expect_err("redirect must loop");

    assert!(error.is_redirect(), "{error:?}");
    assert!(!reqwest_error_is_endpoint_policy(&error), "{error:?}");
    assert!(matches!(
        map_preserved_error(error),
        ProviderError::Network { .. }
    ));
    server.await??;
    Ok(())
}
#[tokio::test]
async fn security_evidence_public_rebinding_matrix_fails_before_socket()
-> Result<(), Box<dyn std::error::Error>> {
    for blocked_ip in ["127.0.0.1", "10.0.0.1", "169.254.169.254", "fd00::1"] {
        for mode in [
            ProviderClientMode::Request,
            ProviderClientMode::Streaming,
            ProviderClientMode::NoRedirect,
        ] {
            assert_public_rebind_is_blocked(mode, blocked_ip.parse()?).await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn security_evidence_private_metadata_answer_does_not_fallback_to_allowed_loopback()
-> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let tripwire = listener.local_addr()?;
    let port = tripwire.port();
    let url = reqwest::Url::parse(&format!("http://private-rebind.test:{port}/v1"))?;
    let resolver = Arc::new(SequenceResolver::new(vec![
        vec![SocketAddr::from((Ipv4Addr::new(93, 184, 216, 34), port))],
        vec![
            tripwire,
            SocketAddr::from((Ipv4Addr::new(169, 254, 169, 254), port)),
        ],
    ]));
    let policy =
        ProviderEndpointPolicy::for_base_url(ProviderEndpointAccess::PrivateNetwork, url.as_str())?;

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

    let client = ProviderHttpClient::build(
        policy,
        Duration::from_secs(1),
        ProviderClientMode::Request,
        resolver.clone(),
    )?;
    assert!(
        client.get(url)?.send().await.is_err(),
        "metadata in a mixed private-network answer must reject the whole answer set"
    );
    assert_eq!(resolver.remaining_answers()?, 0);
    assert_eq!(
        resolver.queries()?,
        ["private-rebind.test", "private-rebind.test"]
    );
    assert_listener_did_not_accept(
        &listener,
        "private-network resolver must not filter metadata and continue with loopback",
    )
    .await;
    Ok(())
}

#[tokio::test]
async fn security_evidence_private_loopback_connects_but_redirect_and_metadata_stay_blocked()
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
    let policy =
        ProviderEndpointPolicy::for_base_url(ProviderEndpointAccess::PrivateNetwork, &base_url)?;
    let client = ProviderHttpClient::new(policy, Duration::from_secs(1))?;
    assert!(
        client
            .get(format!(
                "http://169.254.169.254:{}/latest/meta-data/",
                target_address.port()
            ))
            .is_err()
    );

    let response = client.get(base_url)?.send().await?;
    assert_eq!(response.status(), reqwest::StatusCode::FOUND);
    assert_listener_did_not_accept(&target, "private-network mode must not follow redirects").await;
    server.await??;
    Ok(())
}
