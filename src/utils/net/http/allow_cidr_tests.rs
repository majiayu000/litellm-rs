//! #1503: a fake-IP DNS answer (198.18.0.0/15) reaches the socket only when the
//! provider opts in with `endpoint_allow_cidrs`; metadata answers never do.

use super::*;
use crate::core::net::{EndpointAddressRules, EndpointAllowCidrs};

fn rules(cidrs: &[&str]) -> EndpointAddressRules {
    EndpointAddressRules::new(
        ProviderEndpointAccess::PublicOnly,
        EndpointAllowCidrs::parse(cidrs).expect("test CIDRs should parse"),
    )
}

/// Resolve `api.provider.test` to `answer` through the policy-bound client; if the
/// policy admits it, the connection is redirected to a local listener that answers 200.
async fn request_through_fake_ip_dns(
    rules: EndpointAddressRules,
    answer: IpAddr,
) -> Result<(Result<reqwest::Response, reqwest::Error>, bool), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let tripwire = listener.local_addr()?;
    let base = format!("http://api.provider.test:{}/v1", tripwire.port());
    let policy = ProviderEndpointPolicy::for_base_url_with_rules(rules, &base)?;
    let resolver = Arc::new(SequenceResolver::new(vec![vec![SocketAddr::new(
        answer,
        tripwire.port(),
    )]]));
    let client = ProviderHttpClient::build_with_rebinding_tripwire_for_test(
        policy,
        Duration::from_secs(2),
        ProviderClientMode::NoRedirect,
        resolver,
        tripwire,
    )?;
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.ok()?;
        let mut buffer = [0u8; 1024];
        let _ = socket.read(&mut buffer).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
            .await
            .ok()
    });
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        client.get(format!("{base}/models?api_key=secret"))?.send(),
    )
    .await?;
    let accepted = tokio::time::timeout(Duration::from_millis(200), server)
        .await
        .is_ok_and(|joined| joined.ok().flatten().is_some());
    Ok((result, accepted))
}

const FAKE_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(198, 18, 0, 7));

#[tokio::test]
async fn fake_ip_answer_is_rejected_before_the_socket_without_an_opt_in()
-> Result<(), Box<dyn std::error::Error>> {
    let (result, accepted) = request_through_fake_ip_dns(rules(&[]), FAKE_IP).await?;
    assert!(
        !accepted,
        "default policy must not open a socket for {FAKE_IP}"
    );
    let error = result.expect_err("fake-IP answer must be rejected by default");
    assert!(reqwest_error_is_endpoint_policy(&error), "{error:?}");
    let mapped = map_preserved_error(error).to_string();
    for expected in [
        "Provider endpoint rejected by SSRF protection",
        "'api.provider.test' (resolved to 198.18.0.7)",
        "add a CIDR covering 198.18.0.7 to the provider's `endpoint_allow_cidrs`",
    ] {
        assert!(
            mapped.contains(expected),
            "missing {expected:?} in {mapped}"
        );
    }
    assert!(
        !mapped.contains("secret") && !mapped.contains("/models"),
        "{mapped}"
    );
    Ok(())
}

#[tokio::test]
async fn fake_ip_answer_connects_when_the_range_is_allow_listed()
-> Result<(), Box<dyn std::error::Error>> {
    let (result, accepted) =
        request_through_fake_ip_dns(rules(&["198.18.0.0/15"]), FAKE_IP).await?;
    let response = result.expect("allow-listed fake-IP answer must pass the DNS policy");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert!(accepted);

    let cgnat = IpAddr::V4(Ipv4Addr::new(100, 64, 3, 9));
    let (result, accepted) = request_through_fake_ip_dns(rules(&["100.64.0.0/10"]), cgnat).await?;
    assert_eq!(result?.status(), reqwest::StatusCode::OK);
    assert!(accepted);

    // The CGNAT opt-in does not widen to the fake-IP range.
    let (result, accepted) =
        request_through_fake_ip_dns(rules(&["100.64.0.0/10"]), FAKE_IP).await?;
    assert!(result.is_err() && !accepted);
    Ok(())
}

#[tokio::test]
async fn metadata_answer_stays_blocked_with_a_covering_allow_list()
-> Result<(), Box<dyn std::error::Error>> {
    for (cidr, metadata) in [
        ("168.0.0.0/7", "169.254.169.254"),
        ("168.0.0.0/7", "169.254.170.2"),
        ("168.63.0.0/16", "168.63.129.16"),
        ("100.64.0.0/10", "100.100.100.200"),
        ("fd00::/8", "fd00:ec2::254"),
        ("fd00::/8", "fd00:ec2::23"),
        ("::ffff:0:0/96", "::ffff:169.254.169.254"),
        ("::ffff:0:0/96", "::ffff:100.100.100.200"),
    ] {
        let metadata: IpAddr = metadata.parse()?;
        let (result, accepted) = request_through_fake_ip_dns(rules(&[cidr]), metadata).await?;
        assert!(!accepted, "{metadata} must not open a socket");
        let mapped =
            map_preserved_error(result.expect_err("metadata must be rejected")).to_string();
        assert!(mapped.contains("always blocked"), "{mapped}");
        assert!(!mapped.contains("endpoint_allow_cidrs"), "{mapped}");
    }
    Ok(())
}

#[test]
fn base_http_client_threads_the_allow_list_into_its_policy() {
    let mut config = BaseConfig {
        api_base: Some("https://198.18.0.7/v1".to_string()),
        ..BaseConfig::default()
    };
    let error = BaseHttpClient::new_for_provider("openai_compatible", config.clone())
        .expect_err("literal fake-IP endpoint must be rejected by default")
        .to_string();
    assert!(
        error.contains("198.18.0.7") && error.contains("endpoint_allow_cidrs"),
        "{error}"
    );

    config.endpoint_allow_cidrs = EndpointAllowCidrs::parse(["198.18.0.0/15"]).unwrap();
    let client = BaseHttpClient::new_for_provider("openai_compatible", config)
        .expect("allow-listed literal endpoint should build");
    assert!(
        client
            .post("https://198.18.0.7/v1/chat/completions")
            .is_ok()
    );
    assert!(client.post("https://169.254.169.254/latest").is_err());

    // Without a configured API base the allow list still applies to request URLs.
    let unbound = BaseConfig {
        api_base: None,
        endpoint_allow_cidrs: EndpointAllowCidrs::parse(["198.18.0.0/15"]).unwrap(),
        ..BaseConfig::default()
    };
    let client = BaseHttpClient::new_for_provider("openai", unbound)
        .expect("unbound allow-listed client should build");
    assert!(
        client
            .post("https://198.18.0.7/v1/chat/completions")
            .is_ok()
    );
    assert!(client.post("https://169.254.169.254/latest").is_err());
    let default = BaseHttpClient::new_for_provider("openai", BaseConfig::default())
        .expect("default client should build");
    assert!(
        default
            .post("https://198.18.0.7/v1/chat/completions")
            .is_err()
    );
}

#[test]
fn allow_listed_clients_do_not_share_a_pool_with_default_clients()
-> Result<(), Box<dyn std::error::Error>> {
    let base = "https://api.provider.test/v1";
    let timeout = Duration::from_secs(41);
    let default = ProviderHttpClient::new(
        ProviderEndpointPolicy::for_base_url_with_rules(rules(&[]), base)?,
        timeout,
    )?;
    let opted = ProviderHttpClient::new(
        ProviderEndpointPolicy::for_base_url_with_rules(rules(&["198.18.0.0/15"]), base)?,
        timeout,
    )?;
    let opted_again = ProviderHttpClient::new(
        ProviderEndpointPolicy::for_base_url_with_rules(rules(&["198.18.0.0/15"]), base)?,
        timeout,
    )?;
    // A shared pool would carry the other policy's DNS resolver.
    assert!(!Arc::ptr_eq(&default.client, &opted.client));
    assert!(Arc::ptr_eq(&opted.client, &opted_again.client));
    Ok(())
}
