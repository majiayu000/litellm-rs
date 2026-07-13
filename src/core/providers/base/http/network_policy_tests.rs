use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PROXY_CHILD_TARGET_ENV: &str = "LITELLM_RS_BASE_HTTP_PROXY_CHILD_TARGET";
const PROXY_CHILD_TEST: &str = "core::providers::base::http::network_policy_tests::base_policy_client_ignores_proxy_environment_child";
const ALLOWED_BASE_SYMBOLS: &[&str] = &[
    "BaseConfig",
    "BaseHttpClient",
    "HttpErrorMapper",
    "OpenAIRequestTransformer",
    "UrlBuilder",
    "apply_provider_headers",
    "create_provider_sse_stream",
    "get_pricing_db",
    "header",
    "header_owned",
    "header_static",
];

fn base_boundary_violation(source: &str) -> Option<String> {
    for forbidden_import in [
        "use crate::{",
        "core::providers::{",
        "use crate as ",
        "use crate::core::providers as ",
        "use crate::core::providers;",
    ] {
        if source.contains(forbidden_import) {
            return Some(format!(
                "unsupported grouped or aliased import {forbidden_import}"
            ));
        }
    }

    let import_prefix = "use crate::core::providers::base";
    for segment in source.split(';') {
        let Some(start) = segment.find(import_prefix) else {
            continue;
        };
        let rest = segment[start + import_prefix.len()..].trim();
        if rest.contains(" as ") {
            return Some("base import aliases are forbidden".to_string());
        }
        if let Some(list) = rest.strip_prefix("::{") {
            let Some(end) = list.rfind('}') else {
                return Some("base import list is missing its closing brace".to_string());
            };
            for symbol in list[..end]
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                if !symbol
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
                    || !ALLOWED_BASE_SYMBOLS.contains(&symbol)
                {
                    return Some(format!("base import {symbol} is not policy-safe"));
                }
            }
        } else if let Some(symbol) = rest.strip_prefix("::") {
            if !ALLOWED_BASE_SYMBOLS.contains(&symbol) {
                return Some(format!("base import {symbol} is not policy-safe"));
            }
        } else {
            return Some("the base module itself cannot be imported".to_string());
        }
    }

    let qualified_prefix = "crate::core::providers::base::";
    for (start, _) in source.match_indices(qualified_prefix) {
        let rest = &source[start + qualified_prefix.len()..];
        if rest.starts_with('{') {
            continue;
        }
        let symbol: String = rest
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        if !ALLOWED_BASE_SYMBOLS.contains(&symbol.as_str()) {
            return Some(format!("qualified base access {symbol} is not policy-safe"));
        }
    }
    None
}

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
    assert!(
        base_boundary_violation(
            "use crate::core::providers::base::{BaseConfig, BaseHttpClient, header};"
        )
        .is_none()
    );
    for bypass in [
        "use crate::core::providers::base::GlobalPoolManager;",
        "use crate::core::providers::base::{BaseConfig, ConnectionPool};",
        "crate::core::providers::base::ConnectionPool::client(&pool);",
        "use crate::core::providers::base as raw_base;",
    ] {
        assert!(base_boundary_violation(bypass).is_some());
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
        if let Some(violation) = base_boundary_violation(source) {
            panic!("{path} bypasses the BaseHttpClient boundary: {violation}");
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
