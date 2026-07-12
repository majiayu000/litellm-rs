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
fn shared_dns_filter_rejects_platform_metadata_encodings() {
    for ip in [
        "168.63.129.16",
        "::ffff:168.63.129.16",
        "64:ff9b::a83f:8110",
    ] {
        let address = SocketAddr::new(ip.parse().unwrap(), 80);
        assert!(
            validate_provider_addresses(ProviderEndpointAccess::PublicOnly, vec![address]).is_err(),
            "metadata destination {ip} must be rejected"
        );
    }
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
async fn provider_client_rejects_initial_literal_without_opening_socket()
-> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let client = ProviderHttpClient::new(
        ProviderEndpointPolicy::public_only(),
        Duration::from_secs(1),
    )?;

    assert!(client.get(format!("http://{address}/v1")).is_err());
    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err(),
        "rejected literal must not open the target socket"
    );
    Ok(())
}

#[tokio::test]
async fn provider_client_rejects_metadata_hostname_before_dns_or_socket()
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
    let remaining_answers = resolver
        .answers
        .lock()
        .map_err(|_| io::Error::other("sequence resolver lock poisoned"))?
        .len();
    assert_eq!(remaining_answers, 1, "metadata URL must not invoke DNS");
    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err(),
        "metadata URL must not open the target socket"
    );
    Ok(())
}

#[tokio::test]
async fn public_provider_redirect_rejects_private_target_without_opening_socket()
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
        .redirect(provider_redirect_policy(
            ProviderEndpointPolicy::public_only(),
        ))
        .build()?;

    let result = client
        .get(format!("http://{source_address}/v1"))
        .send()
        .await;
    assert!(result.is_err(), "private redirect target must fail");
    assert!(
        tokio::time::timeout(Duration::from_millis(100), target.accept())
            .await
            .is_err(),
        "provider redirect must not open the target socket"
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
    let policy =
        ProviderEndpointPolicy::for_base_url(ProviderEndpointAccess::PrivateNetwork, &base_url)?;
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
