use super::*;

fn fixture() -> (tempfile::TempDir, GitHubCopilotConfig) {
    let directory = tempfile::tempdir().unwrap();
    let config = GitHubCopilotConfig {
        token_dir: Some(directory.path().to_str().unwrap().into()),
        access_token_file: Some("access-token".into()),
        api_key_file: Some("api-key.json".into()),
        ..Default::default()
    };
    (directory, config)
}

fn key(token: &str, expires_at: u64, endpoint: &str) -> ApiKeyInfo {
    ApiKeyInfo {
        token: token.into(),
        expires_at,
        endpoints: Endpoints {
            api: Some(endpoint.into()),
        },
    }
}

fn write_key(directory: &std::path::Path, info: &ApiKeyInfo) {
    fs::write(
        directory.join("api-key.json"),
        serde_json::to_vec(info).unwrap(),
    )
    .unwrap();
}

#[test]
fn captured_copilot_identity_retains_stable_refresh_authority() {
    let (directory, config) = fixture();
    fs::write(directory.path().join("access-token"), "access-a").unwrap();
    write_key(
        directory.path(),
        &key("api-a", u64::MAX, "https://account-a.example"),
    );
    let (_, identity) = CopilotAuthenticator::capture(&config);

    for change in [
        "API token",
        "expiry",
        "JSON formatting",
        "access whitespace",
    ] {
        fs::write(directory.path().join("access-token"), "access-a").unwrap();
        let mut refreshed = key("api-a", u64::MAX, "https://account-a.example");
        match change {
            "API token" => refreshed.token = "api-refreshed".into(),
            "expiry" => refreshed.expires_at -= 1,
            "JSON formatting" => {}
            "access whitespace" => {
                fs::write(directory.path().join("access-token"), " \taccess-a\r\n").unwrap();
            }
            _ => unreachable!(),
        }
        let bytes = if change == "JSON formatting" {
            serde_json::to_vec_pretty(&refreshed).unwrap()
        } else {
            serde_json::to_vec(&refreshed).unwrap()
        };
        fs::write(directory.path().join("api-key.json"), bytes).unwrap();
        let (captured, next_identity) = CopilotAuthenticator::capture(&config);
        assert_eq!(identity, next_identity, "{change}");
        assert_eq!(captured.cached_access_token().as_deref(), Some("access-a"));
        assert_eq!(captured.cached_api_key().unwrap().token, refreshed.token);
    }
}

#[test]
fn api_only_copilot_identity_uses_the_stable_opaque_key() {
    let (directory, config) = fixture();
    let original = key("opaque-key;exp=1", u64::MAX, "https://account-a.example");
    write_key(directory.path(), &original);
    let (captured, identity) = CopilotAuthenticator::capture(&config);
    assert!(!captured.device_flow_allowed());

    for change in ["unchanged", "expiry", "JSON formatting"] {
        let mut refreshed = original.clone();
        if change == "expiry" {
            refreshed.expires_at -= 1;
        }
        let bytes = if change == "JSON formatting" {
            serde_json::to_vec_pretty(&refreshed).unwrap()
        } else {
            serde_json::to_vec(&refreshed).unwrap()
        };
        fs::write(directory.path().join("api-key.json"), bytes).unwrap();
        assert_eq!(
            identity,
            CopilotAuthenticator::capture(&config).1,
            "{change}"
        );
    }
    for change in ["opaque token", "endpoint"] {
        let mut rotated = original.clone();
        match change {
            "opaque token" => rotated.token = "opaque-key;exp=2".into(),
            "endpoint" => rotated.endpoints.api = Some("https://account-b.example".into()),
            _ => unreachable!(),
        }
        write_key(directory.path(), &rotated);
        assert_ne!(
            identity,
            CopilotAuthenticator::capture(&config).1,
            "{change}"
        );
    }
}

#[test]
fn captured_copilot_identity_uses_the_provider_endpoint_precedence() {
    let (directory, mut config) = fixture();
    fs::write(directory.path().join("access-token"), "access-a").unwrap();
    let (_, default_identity) = CopilotAuthenticator::capture(&config);
    config.api_base = Some(GITHUB_COPILOT_API_BASE.into());
    assert_eq!(default_identity, CopilotAuthenticator::capture(&config).1);
    config.api_base = Some("https://configured.example".into());
    let (_, configured_identity) = CopilotAuthenticator::capture(&config);
    assert_ne!(default_identity, configured_identity);

    let mut info = key("api-a", u64::MAX, "https://credential.example");
    info.endpoints.api = None;
    write_key(directory.path(), &info);
    assert_eq!(
        configured_identity,
        CopilotAuthenticator::capture(&config).1
    );
    info.endpoints.api = Some("https://credential.example".into());
    write_key(directory.path(), &info);
    let (_, credential_identity) = CopilotAuthenticator::capture(&config);
    assert_ne!(configured_identity, credential_identity);
    config.api_base = Some("https://ignored-config.example".into());
    assert_eq!(
        credential_identity,
        CopilotAuthenticator::capture(&config).1
    );
}

#[tokio::test]
async fn captured_copilot_credentials_and_clones_ignore_replaced_files() {
    let (directory, config) = fixture();
    fs::write(directory.path().join("access-token"), "access-a").unwrap();
    write_key(
        directory.path(),
        &key("api-a", u64::MAX, "https://account-a.example"),
    );
    let (first, identity) = CopilotAuthenticator::capture(&config);
    assert_eq!(identity, CopilotAuthenticator::capture(&config).1);

    fs::write(directory.path().join("access-token"), "access-b").unwrap();
    write_key(
        directory.path(),
        &key("api-b", u64::MAX, "https://account-b.example"),
    );
    let cloned = first.clone();
    for captured in [&first, &cloned] {
        assert_eq!(captured.get_access_token().await.unwrap(), "access-a");
        assert_eq!(captured.get_api_key().await.unwrap(), "api-a");
        assert_eq!(
            captured.get_api_base().as_deref(),
            Some("https://account-a.example")
        );
        let debug = format!("{captured:?}");
        assert!(!debug.contains("access-a") && !debug.contains("api-a"));
    }
    let (second, next_identity) = CopilotAuthenticator::capture(&config);
    assert_ne!(identity, next_identity);
    assert_eq!(second.get_access_token().await.unwrap(), "access-b");
    assert_eq!(second.get_api_key().await.unwrap(), "api-b");

    // The public constructor follows main's construction-capture contract too.
    let standalone = CopilotAuthenticator::new(&config);
    fs::write(directory.path().join("access-token"), "access-c").unwrap();
    write_key(
        directory.path(),
        &key("api-c", u64::MAX, "https://account-c.example"),
    );
    assert_eq!(standalone.get_access_token().await.unwrap(), "access-b");
    assert_eq!(standalone.get_api_key().await.unwrap(), "api-b");
    assert_eq!(
        standalone.get_api_base().as_deref(),
        Some("https://account-b.example")
    );
    assert_eq!(standalone.credential_resource_identity(), next_identity);
}

#[tokio::test]
async fn captured_copilot_refresh_uses_bound_access_and_preserves_external_rotation() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    for rotation in ["none", "access", "api", "both"] {
        let (directory, config) = fixture();
        let access_path = directory.path().join("access-token");
        let api_path = directory.path().join("api-key.json");
        fs::write(&access_path, "captured-access").unwrap();
        write_key(
            directory.path(),
            &key("expired-key", 0, "https://initial.example"),
        );
        let (mut captured, _) = CopilotAuthenticator::capture(&config);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        captured.api_key_url = Some(format!("http://{}/token", listener.local_addr().unwrap()));
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for index in 1..=2 {
                let (mut socket, _) =
                    tokio::time::timeout(Duration::from_secs(5), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut bytes = Vec::new();
                while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    let mut buffer = [0; 4096];
                    let count =
                        tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buffer))
                            .await
                            .unwrap()
                            .unwrap();
                    assert_ne!(count, 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    assert!(bytes.len() < 32_768);
                }
                requests.push(String::from_utf8(bytes).unwrap());
                let body = serde_json::to_string(&key(
                    &format!("refreshed-{index}"),
                    u64::MAX,
                    &format!("https://refreshed-{index}.example"),
                ))
                .unwrap();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                );
                socket.write_all(response.as_bytes()).await.unwrap();
                socket.shutdown().await.unwrap();
            }
            requests
        });

        let concurrent = captured.clone();
        let (first, same_refresh) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(captured.get_api_key_info(), concurrent.get_api_key_info())
        })
        .await
        .unwrap();
        let first = first.unwrap();
        let same_refresh = same_refresh.unwrap();
        assert_eq!(first.token, "refreshed-1");
        assert_eq!(
            same_refresh.token, first.token,
            "clones must share one refresh"
        );
        assert_eq!(
            first.endpoints.api.as_deref(),
            Some("https://refreshed-1.example")
        );
        assert_eq!(same_refresh.endpoints.api, first.endpoints.api);
        assert_eq!(
            serde_json::from_slice::<ApiKeyInfo>(&fs::read(&api_path).unwrap())
                .unwrap()
                .token,
            "refreshed-1"
        );
        if matches!(rotation, "access" | "both") {
            fs::write(&access_path, "external-access").unwrap();
        }
        if matches!(rotation, "api" | "both") {
            write_key(
                directory.path(),
                &key("external-key", u64::MAX, "https://external.example"),
            );
        }
        let access_before = fs::read(&access_path).unwrap();
        let api_before = fs::read(&api_path).unwrap();
        captured.invalidate_api_key();
        let cloned = captured.clone();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), cloned.get_api_key())
                .await
                .unwrap()
                .unwrap(),
            "refreshed-2"
        );
        assert_eq!(captured.get_api_key().await.unwrap(), "refreshed-2");
        assert_eq!(
            captured.get_api_base().as_deref(),
            Some("https://refreshed-2.example")
        );
        assert_eq!(first.token, "refreshed-1");
        assert_eq!(
            first.endpoints.api.as_deref(),
            Some("https://refreshed-1.example")
        );
        assert_eq!(fs::read(&access_path).unwrap(), access_before);
        if rotation == "none" {
            assert_eq!(
                serde_json::from_slice::<ApiKeyInfo>(&fs::read(&api_path).unwrap())
                    .unwrap()
                    .token,
                "refreshed-2"
            );
        } else {
            assert_eq!(
                fs::read(&api_path).unwrap(),
                api_before,
                "rotation: {rotation}"
            );
        }
        for request in server.await.unwrap() {
            assert!(request.starts_with("GET /token HTTP/1.1\r\n"));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: token captured-access\r\n")
            );
            assert!(!request.contains("external-access"));
        }
    }
}

#[tokio::test]
async fn unbound_copilot_constructions_remain_lazy_and_do_not_share_identity() {
    let (_directory, config) = fixture();
    let (first, identity) = CopilotAuthenticator::capture(&config);
    let (_, second_identity) = CopilotAuthenticator::capture(&config);
    assert_ne!(identity, second_identity);
    assert!(first.device_flow_allowed());
    let provider = crate::core::providers::github_copilot::GitHubCopilotProvider::new(config)
        .await
        .unwrap();
    assert_eq!(
        provider.credential_resource_identity(),
        provider.clone().credential_resource_identity()
    );
    let clone = first.clone();
    first
        .cache_credential(CredentialUpdate::AccessToken("first-acquired-token"))
        .unwrap();
    assert_eq!(
        clone.get_access_token().await.unwrap(),
        "first-acquired-token"
    );
}

#[tokio::test]
async fn api_only_copilot_snapshot_requires_reload_for_new_refresh_authority() {
    let (directory, config) = fixture();
    write_key(
        directory.path(),
        &key("initial-key", u64::MAX, "https://initial.example"),
    );
    let (captured, identity) = CopilotAuthenticator::capture(&config);
    assert_eq!(captured.get_api_key().await.unwrap(), "initial-key");
    fs::write(directory.path().join("access-token"), "new-access").unwrap();
    write_key(
        directory.path(),
        &key("new-key", u64::MAX, "https://new.example"),
    );
    captured.invalidate_api_key();
    let error = tokio::time::timeout(Duration::from_secs(1), captured.get_api_key())
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(error, ProviderError::Authentication { .. }));
    assert!(error.to_string().contains("reload the provider"));
    let (reloaded, reloaded_identity) = CopilotAuthenticator::capture(&config);
    assert_ne!(identity, reloaded_identity);
    assert_eq!(reloaded.get_access_token().await.unwrap(), "new-access");
    assert_eq!(reloaded.get_api_key().await.unwrap(), "new-key");
}
