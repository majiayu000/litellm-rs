use super::*;

fn assert_configuration(error: ProviderError) {
    assert!(matches!(error, ProviderError::Configuration { .. }));
    let decision = litellm_rs::core::router::retry_policy::RetryPolicy.decide(
        &litellm_rs::core::router::RouterConfig::default(),
        &error,
        litellm_rs::core::router::retry_policy::RetryContext::unary(1, 2),
    );
    assert!(!decision.should_retry);
}

#[test]
fn native_media_debug_redacts_credentials_and_custom_headers() {
    let mut stability_config = StabilityConfig::with_api_key("stability-debug-secret");
    stability_config.base.headers.insert(
        "x-stability-secret".to_string(),
        "stability-header-secret".to_string(),
    );
    let stability_config_debug = format!("{stability_config:?}");
    let stability = StabilityProvider::new(stability_config).expect("Stability should initialize");
    let stability_debug = format!("{stability:?}");
    let stability_enum_debug = format!("{:?}", Provider::Stability(stability));

    let mut bfl_config = BflConfig::with_api_key("bfl-debug-secret");
    bfl_config
        .base
        .headers
        .insert("x-bfl-secret".to_string(), "bfl-header-secret".to_string());
    let bfl_config_debug = format!("{bfl_config:?}");
    let bfl = BflProvider::new(bfl_config).expect("BFL should initialize");
    let bfl_debug = format!("{bfl:?}");
    let bfl_enum_debug = format!("{:?}", Provider::BlackForestLabs(Box::new(bfl)));

    for (debug, provider, secrets) in [
        (
            stability_config_debug,
            "StabilityConfig",
            ["stability-debug-secret", "stability-header-secret"],
        ),
        (
            stability_debug,
            "StabilityProvider",
            ["stability-debug-secret", "stability-header-secret"],
        ),
        (
            stability_enum_debug,
            "Stability",
            ["stability-debug-secret", "stability-header-secret"],
        ),
        (
            bfl_config_debug,
            "BflConfig",
            ["bfl-debug-secret", "bfl-header-secret"],
        ),
        (
            bfl_debug,
            "BflProvider",
            ["bfl-debug-secret", "bfl-header-secret"],
        ),
        (
            bfl_enum_debug,
            "BlackForestLabs",
            ["bfl-debug-secret", "bfl-header-secret"],
        ),
    ] {
        assert!(
            debug.contains(provider),
            "debug should preserve provider identity: {debug}"
        );
        for secret in secrets {
            assert!(!debug.contains(secret), "debug leaked {secret}: {debug}");
        }
    }

    #[cfg(feature = "runway-media")]
    {
        let mut config = RunwayConfig::with_api_key("runway-debug-secret");
        config.base.headers.insert(
            "x-runway-secret".to_string(),
            "runway-header-secret".to_string(),
        );
        let config_debug = format!("{config:?}");
        let provider = RunwayProvider::new(config).expect("Runway should initialize");
        let provider_debug = format!("{provider:?}");
        for (debug, identity) in [
            (config_debug, "RunwayConfig"),
            (provider_debug, "RunwayProvider"),
        ] {
            assert!(debug.contains(identity));
            assert!(!debug.contains("runway-debug-secret"));
            assert!(!debug.contains("runway-header-secret"));
        }
    }
}

#[test]
fn generation_lifecycle_debug_redacts_transport_configuration() {
    let mut base = BaseConfig {
        api_key: Some("lifecycle-api-key-sentinel".to_string()),
        api_base: Some("https://poll.example.test/v1".to_string()),
        ..BaseConfig::default()
    };
    base.headers.insert(
        "x-lifecycle-secret".to_string(),
        "lifecycle-header-sentinel".to_string(),
    );
    let lifecycle = GenerationLifecycle::new_no_redirect(
        "lifecycle-debug-provider",
        base,
        PollPolicy::from_millis(5, 20, 100),
    )
    .expect("lifecycle should initialize");

    let debug = format!("{lifecycle:?}");
    assert!(debug.contains("GenerationLifecycle"));
    assert!(debug.contains("lifecycle-debug-provider"));
    assert!(debug.contains("PollPolicy"));
    assert!(!debug.contains("lifecycle-api-key-sentinel"));
    assert!(!debug.contains("lifecycle-header-sentinel"));
}

#[test]
fn native_media_rejects_structurally_unsafe_custom_endpoints() {
    for endpoint in [
        "https://user:pass@example.com/v1",
        "https://example.com/v1?route=other",
        "https://example.com/v1#fragment",
    ] {
        let mut stability = StabilityConfig::with_api_key("stability-secret");
        stability.base.api_base = Some(endpoint.to_string());
        assert_configuration(StabilityProvider::new(stability).unwrap_err());

        let mut bfl = BflConfig::with_api_key("bfl-secret");
        bfl.base.api_base = Some(endpoint.to_string());
        assert_configuration(BflProvider::new(bfl).unwrap_err());

        #[cfg(feature = "runway-media")]
        {
            let mut runway = RunwayConfig::with_api_key("runway-secret");
            runway.base.api_base = Some(endpoint.to_string());
            assert_configuration(RunwayProvider::new(runway).unwrap_err());
        }
    }
}

#[tokio::test]
async fn native_media_factory_applies_custom_endpoint_boundary() {
    for provider_type in ["stability", "black_forest_labs"] {
        let error = litellm_rs::core::providers::create_provider(
            litellm_rs::config::models::provider::ProviderConfig {
                name: provider_type.to_string(),
                provider_type: provider_type.to_string(),
                api_key: "native-secret".to_string(),
                base_url: Some("https://example.com/v1?wrong=path".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect_err("factory must apply the native media endpoint boundary");
        assert_configuration(error);
    }
}

#[test]
fn native_media_rejects_invalid_final_headers_at_construction() {
    let mut stability = StabilityConfig::with_api_key("stability\nsecret");
    assert_configuration(StabilityProvider::new(stability).unwrap_err());
    stability = StabilityConfig::with_api_key("stability-secret");
    stability
        .base
        .headers
        .insert("x-route".to_string(), "bad\nvalue".to_string());
    assert_configuration(StabilityProvider::new(stability).unwrap_err());

    let mut bfl = BflConfig::with_api_key("bfl-secret");
    bfl.base
        .headers
        .insert("bad\nname".to_string(), "value".to_string());
    assert_configuration(BflProvider::new(bfl).unwrap_err());

    #[cfg(feature = "runway-media")]
    {
        let mut runway = RunwayConfig::with_api_key("runway-secret");
        runway
            .base
            .headers
            .insert("x-route".to_string(), "bad\r\nvalue".to_string());
        assert_configuration(RunwayProvider::new(runway).unwrap_err());
    }
}

#[test]
fn native_media_ignores_invalid_configured_values_for_mandatory_headers() {
    let mut stability = StabilityConfig::with_api_key("stability-secret");
    stability
        .base
        .headers
        .insert("Authorization".to_string(), "bad\nvalue".to_string());
    StabilityProvider::new(stability)
        .expect("mandatory Stability auth must replace configured auth");

    let mut bfl = BflConfig::with_api_key("bfl-secret");
    bfl.base
        .headers
        .insert("x-key".to_string(), "bad\nvalue".to_string());
    BflProvider::new(bfl).expect("mandatory BFL auth must replace configured x-key");

    #[cfg(feature = "runway-media")]
    {
        let mut runway = RunwayConfig::with_api_key("runway-secret");
        runway
            .base
            .headers
            .insert("Authorization".to_string(), "bad\nvalue".to_string());
        RunwayProvider::new(runway).expect("mandatory Runway auth must replace configured auth");
    }
}

#[cfg(feature = "runway-media")]
#[tokio::test]
async fn runway_rejects_task_response_ids_that_do_not_match_the_request() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Runway listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for body in [
            r#"{"id":"","status":"RUNNING"}"#,
            r#"{"id":"task-2","status":"RUNNING"}"#,
            r#"{"id":"../task-1","status":"RUNNING"}"#,
            r#"{"id":"task-2","status":"RUNNING"}"#,
        ] {
            let (mut socket, _) = listener.accept().await.expect("query should arrive");
            let _request = read_http_request(&mut socket).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("response should write");
        }
    });
    let mut config = RunwayConfig::with_api_key("runway-secret");
    config.base.api_base = Some(format!("http://{address}/v1"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config.poll_policy = PollPolicy::from_millis(1, 1, 50);
    let provider = RunwayProvider::new(config).expect("Runway should initialize");

    for _ in 0..3 {
        let error = provider
            .get_task("task-1")
            .await
            .expect_err("invalid or mismatched response id must fail");
        assert!(matches!(error, ProviderError::ResponseParsing { .. }));
    }
    let error = provider
        .wait_for_task("task-1", &CancellationToken::new())
        .await
        .expect_err("polling must reject a mismatched response id");
    assert!(matches!(error, ProviderError::ResponseParsing { .. }));
    server.await.expect("mock server should finish");
}
