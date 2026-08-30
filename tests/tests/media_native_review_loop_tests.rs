use super::*;

async fn truncated_error_responses(
    responses: Vec<(u16, &'static str)>,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock error listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for (status, reason) in responses {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let _request = read_http_request(&mut socket).await;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\ntruncated"
                    )
                    .as_bytes(),
                )
                .await
                .expect("truncated error response should write");
        }
    });
    (address, server)
}

fn retry_policy_allows(error: &ProviderError) -> bool {
    litellm_rs::core::router::retry_policy::RetryPolicy
        .decide(
            &litellm_rs::core::router::RouterConfig::default(),
            error,
            litellm_rs::core::router::retry_policy::RetryContext::unary(1, 2),
        )
        .should_retry
}

#[tokio::test]
async fn truncated_native_error_bodies_preserve_nonretryable_statuses() {
    let (bfl_address, bfl_server) = truncated_error_responses(vec![(400, "Bad Request")]).await;
    let mut bfl_config = BflConfig::with_api_key("bfl-secret");
    bfl_config.base.api_base = Some(format!("http://{bfl_address}"));
    bfl_config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let bfl = BflProvider::new(bfl_config).expect("BFL provider should initialize");
    let bfl_error = bfl
        .generate_native(
            BflImageRequest::new("flux-pro-1.1", "a glass city"),
            &CancellationToken::new(),
        )
        .await
        .expect_err("truncated BFL 400 response should fail");
    bfl_server.await.expect("BFL server should finish");

    assert!(
        matches!(bfl_error, ProviderError::InvalidRequest { .. }),
        "{bfl_error:?}"
    );
    assert!(!retry_policy_allows(&bfl_error));

    let (stability_address, stability_server) =
        truncated_error_responses(vec![(401, "Unauthorized"), (400, "Bad Request")]).await;
    let mut stability_config = StabilityConfig::with_api_key("stability-secret");
    stability_config.base.api_base = Some(format!("http://{stability_address}"));
    stability_config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let stability =
        StabilityProvider::new(stability_config).expect("Stability provider should initialize");
    let generation_error = stability
        .image_generation(
            ImageGenerationRequest {
                prompt: "paint a lighthouse".to_string(),
                model: Some("stable-image-core".to_string()),
                n: Some(1),
                size: Some("1024x1024".to_string()),
                quality: None,
                response_format: Some("png".to_string()),
                style: None,
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("truncated Stability 401 generation should fail");
    let edit_error = stability
        .image_edit(
            ImageEditRequest {
                image: b"source-image".to_vec(),
                mask: None,
                prompt: "replace the background".to_string(),
                model: Some("inpaint".to_string()),
                n: Some(1),
                size: None,
                response_format: Some("png".to_string()),
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("truncated Stability 400 edit should fail");
    stability_server
        .await
        .expect("Stability server should finish");

    assert!(
        matches!(generation_error, ProviderError::Authentication { .. }),
        "{generation_error:?}"
    );
    assert!(!retry_policy_allows(&generation_error));
    assert!(
        matches!(edit_error, ProviderError::InvalidRequest { .. }),
        "{edit_error:?}"
    );
    assert!(!retry_policy_allows(&edit_error));
}

#[tokio::test]
async fn truncated_native_5xx_error_body_remains_retryable() {
    let (address, server) = truncated_error_responses(vec![(503, "Service Unavailable")]).await;
    let mut config = BflConfig::with_api_key("bfl-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = BflProvider::new(config).expect("BFL provider should initialize");
    let error = provider
        .generate_native(
            BflImageRequest::new("flux-pro-1.1", "a glass city"),
            &CancellationToken::new(),
        )
        .await
        .expect_err("truncated BFL 503 response should fail");
    server.await.expect("BFL server should finish");

    assert!(
        matches!(error, ProviderError::ProviderUnavailable { .. }),
        "{error:?}"
    );
    assert!(retry_policy_allows(&error));
}

#[cfg(feature = "runway-media")]
#[tokio::test]
async fn runway_truncated_error_bodies_preserve_status_for_every_operation() {
    let (address, server) = truncated_error_responses(vec![
        (400, "Bad Request"),
        (401, "Unauthorized"),
        (503, "Service Unavailable"),
    ])
    .await;
    let mut config = RunwayConfig::with_api_key("runway-secret");
    config.base.api_base = Some(format!("http://{address}/v1"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = RunwayProvider::new(config).expect("Runway provider should initialize");

    let submit_error = provider
        .submit_text_to_video(
            litellm_rs::core::providers::runway::RunwayTextToVideoRequest {
                model: "veo3.1".to_string(),
                prompt_text: "a glass city".to_string(),
                ratio: None,
                duration: None,
                seed: None,
                extra: serde_json::Map::new(),
            },
        )
        .await
        .expect_err("truncated Runway submit error should fail");
    let query_error = provider
        .get_task("task-1")
        .await
        .expect_err("truncated Runway query error should fail");
    let cancel_error = provider
        .cancel_task("task-1")
        .await
        .expect_err("truncated Runway cancel error should fail");
    server.await.expect("Runway server should finish");

    assert!(
        matches!(submit_error, ProviderError::InvalidRequest { .. }),
        "{submit_error:?}"
    );
    assert!(!retry_policy_allows(&submit_error));
    assert!(
        matches!(query_error, ProviderError::Authentication { .. }),
        "{query_error:?}"
    );
    assert!(!retry_policy_allows(&query_error));
    assert!(
        matches!(cancel_error, ProviderError::ProviderUnavailable { .. }),
        "{cancel_error:?}"
    );
    assert!(retry_policy_allows(&cancel_error));
}

#[tokio::test]
async fn stability_dns_policy_failure_remains_pre_dispatch_configuration_error() {
    let mut config = StabilityConfig::with_api_key("stability-secret");
    config.base.api_base = Some("http://native-media-does-not-exist.invalid".to_string());
    let provider = StabilityProvider::new(config).expect("provider should initialize");

    let error = provider
        .image_generation(
            ImageGenerationRequest {
                prompt: "paint a lighthouse".to_string(),
                model: Some("stable-image-core".to_string()),
                n: Some(1),
                size: Some("1024x1024".to_string()),
                quality: None,
                response_format: Some("png".to_string()),
                style: None,
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("reserved invalid DNS name must fail before dispatch");

    assert!(
        matches!(error, ProviderError::Configuration { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn stability_rejects_sizes_its_model_cannot_express_before_network_access() {
    for (model, size) in [
        ("stable-image-core", "512x512"),
        ("stable-image-ultra", "512x512"),
        ("sd3.5-large", "1792x1024"),
    ] {
        let mut config = StabilityConfig::with_api_key("stability-secret");
        config.base.api_base = Some("http://127.0.0.1:1".to_string());
        config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        let provider = StabilityProvider::new(config).expect("provider should initialize");

        let error = provider
            .image_generation(
                ImageGenerationRequest {
                    prompt: "paint a lighthouse".to_string(),
                    model: Some(model.to_string()),
                    n: Some(1),
                    size: Some(size.to_string()),
                    quality: None,
                    response_format: Some("png".to_string()),
                    style: None,
                    user: None,
                },
                RequestContext::default(),
            )
            .await
            .expect_err("inexact Stability size must fail before network access");

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        assert!(error.to_string().contains(size));
    }
}

#[tokio::test]
async fn bfl_ratio_only_models_reject_noncanonical_exact_sizes_before_network_access() {
    for model in ["flux-pro-1.1-ultra", "flux-kontext-pro"] {
        let mut config = BflConfig::with_api_key("bfl-secret");
        config.base.api_base = Some("http://127.0.0.1:1".to_string());
        config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        let provider = BflProvider::new(config).expect("BFL provider should initialize");

        let error = provider
            .image_generation(
                ImageGenerationRequest {
                    prompt: "a glass city".to_string(),
                    model: Some(model.to_string()),
                    n: Some(1),
                    size: Some("1024x768".to_string()),
                    quality: None,
                    response_format: Some("url".to_string()),
                    style: None,
                    user: None,
                },
                RequestContext::default(),
            )
            .await
            .expect_err("ratio-only BFL model must reject an exact size");

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        assert!(error.to_string().contains("1024x768"));
    }
}

#[cfg(feature = "runway-media")]
#[test]
fn runway_env_uses_official_secret_with_legacy_fallback() {
    for case in [
        "secret-only",
        "trimmed-secret",
        "both",
        "blank-secret-with-legacy",
        "legacy-only",
        "trimmed-legacy",
        "blank-key-only",
        "both-blank",
    ] {
        let status = std::process::Command::new(
            std::env::current_exe().expect("test executable should exist"),
        )
        .args([
            "--exact",
            "followup_tests::review_loop_tests::runway_env_precedence_child",
            "--nocapture",
        ])
        .env("RUNWAY_ENV_PRECEDENCE_CHILD", case)
        .env_remove("RUNWAYML_API_SECRET")
        .env_remove("RUNWAYML_API_KEY")
        .envs(match case {
            "secret-only" => vec![("RUNWAYML_API_SECRET", "official-secret")],
            "trimmed-secret" => vec![("RUNWAYML_API_SECRET", "  official-secret  ")],
            "both" => vec![
                ("RUNWAYML_API_SECRET", "official-secret"),
                ("RUNWAYML_API_KEY", "legacy-key"),
            ],
            "blank-secret-with-legacy" => vec![
                ("RUNWAYML_API_SECRET", "   "),
                ("RUNWAYML_API_KEY", "legacy-key"),
            ],
            "legacy-only" => vec![("RUNWAYML_API_KEY", "legacy-key")],
            "trimmed-legacy" => vec![("RUNWAYML_API_KEY", "  legacy-key  ")],
            "blank-key-only" => vec![("RUNWAYML_API_KEY", "   ")],
            "both-blank" => vec![("RUNWAYML_API_SECRET", "   "), ("RUNWAYML_API_KEY", "   ")],
            _ => unreachable!(),
        })
        .status()
        .expect("isolated Runway environment test should run");

        assert!(status.success(), "Runway environment case failed: {case}");
    }
}

#[cfg(feature = "runway-media")]
#[test]
fn runway_env_precedence_child() {
    let Ok(case) = std::env::var("RUNWAY_ENV_PRECEDENCE_CHILD") else {
        return;
    };
    let config = RunwayConfig::from_env();
    let expected = match case.as_str() {
        "legacy-only" | "trimmed-legacy" | "blank-secret-with-legacy" => Some("legacy-key"),
        "blank-key-only" | "both-blank" => None,
        _ => Some("official-secret"),
    };
    assert!(
        config.base.api_key.as_deref() == expected,
        "Runway selected the wrong environment credential"
    );
    if expected.is_none() {
        let error = RunwayProvider::from_env()
            .expect_err("blank Runway environment credentials must fail configuration");
        assert!(matches!(error, ProviderError::Configuration { .. }));
    }
    assert!(
        RunwayConfig::with_api_key("explicit-key")
            .base
            .api_key
            .as_deref()
            == Some("explicit-key"),
        "explicit Runway configuration must remain highest precedence"
    );
}

#[test]
fn every_bfl_x_key_client_is_wired_without_redirects() {
    let source = include_str!("../../src/core/providers/black_forest_labs/mod.rs");

    assert!(source.contains("client: BaseHttpClient::new_for_provider_no_redirect"));
    assert!(source.contains("GenerationLifecycle::new_no_redirect"));
}

#[tokio::test]
async fn bfl_submit_redirects_do_not_forward_api_key_to_another_origin() {
    for (status, reason) in [(307, "Temporary Redirect"), (308, "Permanent Redirect")] {
        let source_listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("mock BFL listener should bind");
        let source_address = source_listener
            .local_addr()
            .expect("source address should exist");
        let target_listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("redirect target should bind");
        let target_address = target_listener
            .local_addr()
            .expect("target address should exist");
        let source_server = tokio::spawn(async move {
            let (mut socket, _) = source_listener
                .accept()
                .await
                .expect("submit should arrive");
            let submit = read_http_request(&mut socket).await;
            assert!(submit.to_ascii_lowercase().contains("x-key: bfl-secret"));
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nLocation: http://{target_address}/foreign\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("redirect response should write");
        });
        let target_server = tokio::spawn(async move {
            tokio::time::timeout(
                std::time::Duration::from_millis(150),
                target_listener.accept(),
            )
            .await
        });

        let mut config = BflConfig::with_api_key("bfl-secret");
        config.base.api_base = Some(format!("http://{source_address}"));
        config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        let provider = BflProvider::new(config).expect("BFL provider should initialize");
        provider
            .generate_native(
                BflImageRequest::new("flux-pro-1.1", "a glass city"),
                &CancellationToken::new(),
            )
            .await
            .expect_err("submit redirect must not be followed");
        source_server.await.expect("source server should finish");
        let target_result = target_server.await.expect("target server should finish");

        assert!(target_result.is_err(), "foreign origin received BFL x-key");
    }
}

#[tokio::test]
async fn bfl_poll_redirect_does_not_forward_api_key_to_another_origin() {
    let source_listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL listener should bind");
    let source_address = source_listener
        .local_addr()
        .expect("source address should exist");
    let target_listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("redirect target should bind");
    let target_address = target_listener
        .local_addr()
        .expect("target address should exist");
    let source_server = tokio::spawn(async move {
        let (mut socket, _) = source_listener
            .accept()
            .await
            .expect("submit should arrive");
        let _submit = read_http_request(&mut socket).await;
        let body =
            format!(r#"{{"id":"task-1","polling_url":"http://{source_address}/poll/task-1"}}"#);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("submit response should write");

        let (mut socket, _) = source_listener.accept().await.expect("poll should arrive");
        let poll = read_http_request(&mut socket).await;
        assert!(poll.to_ascii_lowercase().contains("x-key: bfl-secret"));
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/foreign\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("redirect response should write");
    });
    let target_server = tokio::spawn(async move {
        tokio::time::timeout(
            std::time::Duration::from_millis(150),
            target_listener.accept(),
        )
        .await
    });

    let mut config = BflConfig::with_api_key("bfl-secret");
    config.base.api_base = Some(format!("http://{source_address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config.poll_policy = PollPolicy::from_millis(1, 4, 200);
    let provider = BflProvider::new(config).expect("BFL provider should initialize");
    let error = provider
        .generate_native(
            BflImageRequest::new("flux-pro-1.1", "a glass city"),
            &CancellationToken::new(),
        )
        .await
        .expect_err("redirect response must not be followed");
    source_server.await.expect("source server should finish");
    let target_result = target_server.await.expect("target server should finish");

    assert!(matches!(error, ProviderError::Other { .. }));
    assert!(target_result.is_err(), "foreign origin received BFL x-key");
}

#[tokio::test]
async fn bfl_invalid_successful_submit_bodies_are_not_safe_to_resubmit() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for body in ["not-json", "{}", r#"{"polling_url":42}"#] {
            let (mut socket, _) = listener.accept().await.expect("submit should arrive");
            let _request = read_http_request(&mut socket).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("submit response should write");
        }
    });

    let mut config = BflConfig::with_api_key("bfl-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = BflProvider::new(config).expect("BFL provider should initialize");
    for _ in 0..3 {
        let error = provider
            .generate_native(
                BflImageRequest::new("flux-pro-1.1", "a glass city"),
                &CancellationToken::new(),
            )
            .await
            .expect_err("invalid accepted submit response should fail");
        assert!(matches!(error, ProviderError::Other { .. }));
        assert!(error.to_string().contains("already accepted"));
    }
    server.await.expect("mock server should finish");
}

#[derive(Debug, Clone, Copy)]
enum NativeEndpointInput {
    DirectBaseUrl,
    DirectApiBase,
    SettingsBaseUrl,
    SettingsApiBase,
    TopLevelPrecedence,
}

#[tokio::test]
async fn native_image_factory_normalizes_all_endpoint_aliases() {
    for provider_type in [ProviderType::Stability, ProviderType::BlackForestLabs] {
        for input in [
            NativeEndpointInput::DirectBaseUrl,
            NativeEndpointInput::DirectApiBase,
            NativeEndpointInput::SettingsBaseUrl,
            NativeEndpointInput::SettingsApiBase,
            NativeEndpointInput::TopLevelPrecedence,
        ] {
            assert_native_endpoint(provider_type.clone(), input).await;
        }
    }
}

async fn assert_native_endpoint(provider_type: ProviderType, input: NativeEndpointInput) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock native listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let endpoint = format!("http://{address}");
    let server = tokio::spawn(async move {
        let accepted = tokio::time::timeout(std::time::Duration::from_secs(2), listener.accept())
            .await
            .expect("configured native endpoint was not reached")
            .expect("native request should arrive");
        let (mut socket, _) = accepted;
        let request = read_http_request(&mut socket).await;
        if request.starts_with("POST /v2beta/") {
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 3\r\nConnection: close\r\n\r\npng",
                )
                .await
                .expect("Stability response should write");
        } else {
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 8\r\nConnection: close\r\n\r\nnot-json",
                )
                .await
                .expect("BFL response should write");
        }
        request
    });

    let provider = match input {
        NativeEndpointInput::DirectBaseUrl | NativeEndpointInput::DirectApiBase => {
            let key = match input {
                NativeEndpointInput::DirectBaseUrl => "base_url",
                NativeEndpointInput::DirectApiBase => "api_base",
                _ => unreachable!(),
            };
            let mut direct = serde_json::json!({
                "api_key": "native-secret",
                "endpoint_access": "private_network",
            });
            direct
                .as_object_mut()
                .expect("direct config should be an object")
                .insert(key.to_string(), endpoint.clone().into());
            Provider::from_config_async(provider_type.clone(), direct)
                .await
                .expect("direct native provider should initialize")
        }
        NativeEndpointInput::SettingsBaseUrl
        | NativeEndpointInput::SettingsApiBase
        | NativeEndpointInput::TopLevelPrecedence => {
            let mut config = litellm_rs::config::models::provider::ProviderConfig {
                name: provider_type.to_string(),
                provider_type: provider_type.to_string(),
                api_key: "native-secret".to_string(),
                endpoint_access: ProviderEndpointAccess::PrivateNetwork,
                ..Default::default()
            };
            match input {
                NativeEndpointInput::SettingsBaseUrl => {
                    config
                        .settings
                        .insert("base_url".to_string(), endpoint.clone().into());
                }
                NativeEndpointInput::SettingsApiBase => {
                    config
                        .settings
                        .insert("api_base".to_string(), endpoint.clone().into());
                }
                NativeEndpointInput::TopLevelPrecedence => {
                    config.base_url = Some(endpoint.clone());
                    config
                        .settings
                        .insert("base_url".to_string(), "http://127.0.0.1:1".into());
                    config
                        .settings
                        .insert("api_base".to_string(), "http://127.0.0.1:1".into());
                }
                _ => unreachable!(),
            }
            litellm_rs::core::providers::create_provider(config)
                .await
                .expect("gateway native provider should initialize")
        }
    };
    let is_stability = matches!(provider_type, ProviderType::Stability);
    let _result = provider
        .create_images(
            ImageGenerationRequest {
                prompt: "paint a lighthouse".to_string(),
                model: Some(if is_stability {
                    "stable-image-core".to_string()
                } else {
                    "flux-pro-1.1".to_string()
                }),
                n: Some(1),
                size: None,
                quality: None,
                response_format: Some(if is_stability {
                    "png".to_string()
                } else {
                    "url".to_string()
                }),
                style: None,
                user: None,
            },
            RequestContext::default(),
        )
        .await;
    let request = server.await.expect("mock server should finish");
    let expected_path = if is_stability {
        "/v2beta/stable-image/generate/core"
    } else {
        "/flux-pro-1.1"
    };
    assert!(
        request.starts_with(&format!("POST {expected_path} HTTP/1.1")),
        "{provider_type:?} {input:?} used the wrong endpoint: {request}"
    );
}
