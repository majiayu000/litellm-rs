use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PROXY_CHILD_TARGET_ENV: &str = "LITELLM_RS_BASE_HTTP_PROXY_CHILD_TARGET";
const PROXY_CHILD_TEST: &str = "core::providers::base::http::network_policy_tests::base_policy_client_ignores_proxy_environment_child";

#[test]
fn migrated_shared_providers_have_no_raw_client_escape() {
    let base_source = include_str!("../http.rs");
    let provider_sources = [
        ("mistral/mod.rs", include_str!("../../mistral/mod.rs")),
        (
            "cohere/provider.rs",
            include_str!("../../cohere/provider.rs"),
        ),
        ("bedrock/client.rs", include_str!("../../bedrock/client.rs")),
        (
            "bedrock/client/target.rs",
            include_str!("../../bedrock/client/target.rs"),
        ),
        (
            "bedrock/agents/mod.rs",
            include_str!("../../bedrock/agents/mod.rs"),
        ),
        (
            "bedrock/batch/mod.rs",
            include_str!("../../bedrock/batch/mod.rs"),
        ),
        (
            "bedrock/guardrails/mod.rs",
            include_str!("../../bedrock/guardrails/mod.rs"),
        ),
        (
            "bedrock/knowledge_bases/mod.rs",
            include_str!("../../bedrock/knowledge_bases/mod.rs"),
        ),
    ];
    let forbidden_base_exports = [
        ["pub fn ", "inner("].concat(),
        ["pub fn ", "into_inner("].concat(),
        ["pub fn ", "client("].concat(),
        ["impl Deref for ", "BaseHttpClient"].concat(),
        ["pub ", "client:"].concat(),
    ];
    let forbidden_provider_patterns = [
        ".inner()",
        ".into_inner()",
        "ProviderHttpClient",
        "reqwest::Client",
        "reqwest::ClientBuilder",
        "use reqwest::{",
        "use reqwest as",
        "extern crate reqwest as",
        "reqwest::get",
        "reqwest::request",
        "core::http",
        "utils::net",
        "base::connection_pool",
        ".client()",
    ];
    let raw_client_api_names: Vec<_> = [
        include_str!("../../../http/outbound.rs"),
        include_str!("../../../../utils/net/http.rs"),
        include_str!("../../../../utils/net/client/utils.rs"),
        include_str!("../connection_pool.rs"),
    ]
    .into_iter()
    .flat_map(|source| {
        ["pub fn ", "pub(crate) fn "]
            .into_iter()
            .flat_map(move |marker| source.split(marker).skip(1))
    })
    .filter_map(|declaration| {
        let signature = declaration.split_once('{')?.0;
        if !signature.contains("Client") {
            return None;
        }
        let name = declaration.split_once('(')?.0.trim();
        (name.contains("client") && name != "client").then_some(name)
    })
    .collect();

    for required in [
        "default_outbound_client",
        "streaming_outbound_client",
        "build_outbound_client",
        "build_streaming_outbound_client",
    ] {
        assert!(raw_client_api_names.contains(&required));
    }
    for pattern in &forbidden_base_exports {
        assert!(
            !base_source.contains(pattern),
            "BaseHttpClient exposes policy-bound internals through {pattern}"
        );
    }
    for (path, source) in provider_sources {
        for pattern in forbidden_provider_patterns {
            assert!(
                !source.contains(pattern),
                "{path} bypasses BaseHttpClient through {pattern}"
            );
        }
        for api_name in &raw_client_api_names {
            assert!(
                !source
                    .split(|character: char| {
                        !(character.is_ascii_alphanumeric() || character == '_')
                    })
                    .any(|token| token == *api_name),
                "{path} uses raw client API {api_name}"
            );
        }
    }
    let no_redirect_constructor = ["ProviderHttpClient::", "no_redirect"].concat();
    assert!(base_source.contains(&no_redirect_constructor));
    assert_eq!(
        include_str!("../../bedrock/client.rs")
            .matches("new_for_provider_no_redirect")
            .count(),
        3
    );
}

#[tokio::test]
async fn base_no_redirect_client_does_not_reach_redirect_target()
-> Result<(), Box<dyn std::error::Error>> {
    for (status, reason) in [
        (302, "Found"),
        (307, "Temporary Redirect"),
        (308, "Permanent Redirect"),
    ] {
        let source = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let source_address = source.local_addr()?;
        let target = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let target_address = target.local_addr()?;
        let server = tokio::spawn(async move {
            let (mut stream, _) = source.accept().await?;
            let mut request = Vec::new();
            loop {
                let mut chunk = [0_u8; 512];
                let bytes_read = stream.read(&mut chunk).await?;
                if bytes_read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..bytes_read]);
                if request
                    .windows(b"signed-body".len())
                    .any(|window| window == b"signed-body")
                {
                    break;
                }
                if request.len() > 8 * 1024 {
                    return Err(std::io::Error::other("signed request was too large"));
                }
            }
            let request = String::from_utf8(request)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let request = request.to_ascii_lowercase();
            assert!(request.starts_with("post / http/1.1\r\n"));
            assert!(request.contains("\r\nauthorization: signed-request\r\n"));
            assert!(request.contains("\r\nx-amz-security-token: session-token\r\n"));
            assert!(request.contains("\r\n\r\nsigned-body"));
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 {status} {reason}\r\nLocation: http://{target_address}/signed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await?;
            Ok::<(), std::io::Error>(())
        });
        let base_url = format!("http://{source_address}");
        let client = BaseHttpClient::new_for_provider_no_redirect(
            "test",
            BaseConfig {
                api_base: Some(base_url.clone()),
                endpoint_access: ProviderEndpointAccess::PrivateNetwork,
                timeout: 1,
                ..Default::default()
            },
        )?;

        let response = client
            .post(base_url)?
            .header("authorization", "signed-request")
            .header("x-amz-security-token", "session-token")
            .body("signed-body")
            .send()
            .await?;
        assert_eq!(response.status().as_u16(), status);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err(),
            "status {status} must not open the redirect target socket"
        );
        server.await??;
    }
    Ok(())
}

#[tokio::test]
async fn base_policy_client_ignores_proxy_environment() -> Result<(), Box<dyn std::error::Error>> {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let target_address = target.local_addr()?;
    let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let proxy_address = proxy.local_addr()?;
    let server = tokio::spawn(async move {
        let (mut stream, _) = target.accept().await?;
        let mut request = [0_u8; 1024];
        if stream.read(&mut request).await? == 0 {
            return Err(std::io::Error::other("request ended before headers"));
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await?;
        Ok::<(), std::io::Error>(())
    });
    let executable = std::env::current_exe()?;
    let target_url = format!("http://{target_address}");
    let proxy_url = format!("http://{proxy_address}");
    let child = tokio::task::spawn_blocking(move || {
        std::process::Command::new(executable)
            .arg("--exact")
            .arg(PROXY_CHILD_TEST)
            .arg("--ignored")
            .arg("--nocapture")
            .env(PROXY_CHILD_TARGET_ENV, target_url)
            .env("HTTP_PROXY", &proxy_url)
            .env("http_proxy", &proxy_url)
            .env("ALL_PROXY", &proxy_url)
            .env("all_proxy", &proxy_url)
            .env_remove("NO_PROXY")
            .env_remove("no_proxy")
            .output()
    });
    let output = tokio::time::timeout(Duration::from_secs(10), child)
        .await
        .map_err(|_| std::io::Error::other("isolated proxy child test timed out"))?
        .map_err(|error| std::io::Error::other(error.to_string()))??;
    assert!(
        output.status.success(),
        "isolated proxy child failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server.await??;
    assert!(
        tokio::time::timeout(Duration::from_millis(100), proxy.accept())
            .await
            .is_err(),
        "policy client must not connect to the configured proxy"
    );
    Ok(())
}

#[test]
#[ignore = "launched by the isolated proxy parent test"]
fn base_policy_client_ignores_proxy_environment_child() -> Result<(), Box<dyn std::error::Error>> {
    let target_url = std::env::var(PROXY_CHILD_TARGET_ENV)?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            let client = BaseHttpClient::new_for_provider(
                "test",
                BaseConfig {
                    api_base: Some(target_url.clone()),
                    endpoint_access: ProviderEndpointAccess::PrivateNetwork,
                    timeout: 1,
                    ..Default::default()
                },
            )?;
            assert!(client.get(target_url)?.send().await?.status().is_success());
            Ok(())
        })
}
