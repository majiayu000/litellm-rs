use super::*;

#[tokio::test]
async fn bfl_kontext_edit_rejects_noncanonical_exact_size_before_network_access() {
    let mut config = BflConfig::with_api_key("bfl-secret");
    config.base.api_base = Some("http://127.0.0.1:1".to_string());
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = BflProvider::new(config).expect("BFL provider should initialize");
    let error = provider
        .image_edit(
            ImageEditRequest {
                image: b"source-image".to_vec(),
                mask: None,
                prompt: "make the sky green".to_string(),
                model: Some("flux-kontext-pro".to_string()),
                n: Some(1),
                size: Some("1024x768".to_string()),
                response_format: Some("url".to_string()),
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("Kontext must not collapse an exact size to an aspect ratio");

    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    assert!(error.to_string().contains("1024x768"));
}

#[tokio::test]
async fn bfl_cancellation_interrupts_submit_body_read() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let (headers_sent_tx, headers_sent_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("submit should arrive");
        let _request = read_http_request(&mut socket).await;
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("headers should write");
        let _ = headers_sent_tx.send(());
        let mut byte = [0_u8; 1];
        let _ = socket.read(&mut byte).await;
    });

    let mut config = BflConfig::with_api_key("bfl-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = BflProvider::new(config).expect("BFL provider should initialize");
    let cancellation = CancellationToken::new();
    let cancellation_for_request = cancellation.clone();
    let request = tokio::spawn(async move {
        provider
            .generate_native(
                BflImageRequest::new("flux-pro-1.1", "a glass city"),
                &cancellation_for_request,
            )
            .await
    });
    headers_sent_rx
        .await
        .expect("response headers should arrive");
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    cancellation.cancel();
    let error = tokio::time::timeout(std::time::Duration::from_millis(200), request)
        .await
        .expect("cancellation must interrupt the stalled body read")
        .expect("request task should finish")
        .expect_err("cancellation must return an error");
    server.await.expect("mock server should finish");

    assert!(matches!(error, ProviderError::Cancelled { .. }));
}

#[tokio::test]
async fn bfl_poll_transport_error_is_not_safe_to_resubmit() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("submit should arrive");
        let _request = read_http_request(&mut socket).await;
        let body = r#"{"id":"task-1","polling_url":"http://127.0.0.1:1/poll/task-1"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("submit response should write");
    });

    let mut config = BflConfig::with_api_key("bfl-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config.poll_policy = PollPolicy::from_millis(1, 4, 200);
    let provider = BflProvider::new(config).expect("BFL provider should initialize");
    let error = provider
        .generate_native(
            BflImageRequest::new("flux-pro-1.1", "a glass city"),
            &CancellationToken::new(),
        )
        .await
        .expect_err("poll transport failure should surface");
    server.await.expect("mock server should finish");

    assert!(matches!(error, ProviderError::Other { .. }));
    assert!(error.to_string().contains("already accepted"));
}

#[tokio::test]
async fn bfl_success_headers_then_body_failure_is_not_safe_to_resubmit() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("submit should arrive");
        let _request = read_http_request(&mut socket).await;
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\ntruncated",
            )
            .await
            .expect("truncated response should write");
    });

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
        .expect_err("accepted truncated response must fail");
    server.await.expect("mock server should finish");

    assert!(matches!(error, ProviderError::Other { .. }));
    assert!(error.to_string().contains("already accepted"));
}

#[tokio::test]
async fn bfl_rejects_cross_origin_polling_url_before_forwarding_api_key() {
    let submit_listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL submit listener should bind");
    let submit_address = submit_listener
        .local_addr()
        .expect("submit address should exist");
    let poll_listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock foreign poll listener should bind");
    let poll_address = poll_listener
        .local_addr()
        .expect("poll address should exist");
    let submit_server = tokio::spawn(async move {
        let (mut socket, _) = submit_listener
            .accept()
            .await
            .expect("submit should arrive");
        let _request = read_http_request(&mut socket).await;
        let body =
            format!(r#"{{"id":"task-1","polling_url":"http://{poll_address}/poll/task-1"}}"#);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("submit response should write");
    });
    let poll_server = tokio::spawn(async move {
        tokio::time::timeout(
            std::time::Duration::from_millis(150),
            poll_listener.accept(),
        )
        .await
    });

    let mut config = BflConfig::with_api_key("bfl-secret");
    config.base.api_base = Some(format!("http://{submit_address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config.poll_policy = PollPolicy::from_millis(1, 4, 200);
    let provider = BflProvider::new(config).expect("BFL provider should initialize");
    let error = provider
        .generate_native(
            BflImageRequest::new("flux-pro-1.1", "a glass city"),
            &CancellationToken::new(),
        )
        .await
        .expect_err("cross-origin polling URL must be rejected");
    submit_server.await.expect("submit server should finish");
    let poll_result = poll_server.await.expect("poll server should finish");

    assert!(matches!(error, ProviderError::Other { .. }));
    assert!(error.to_string().contains("configured API origin"));
    assert!(
        poll_result.is_err(),
        "foreign polling origin must not receive a request containing x-key"
    );
}

#[tokio::test]
async fn stability_post_header_body_failures_are_not_retryable() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Stability listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let _request = read_http_request(&mut socket).await;
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 100\r\nConnection: close\r\n\r\ntruncated",
                )
                .await
                .expect("truncated response should write");
        }
    });
    let mut config = StabilityConfig::with_api_key("stability-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = StabilityProvider::new(config).expect("provider should initialize");

    let generation_error = provider
        .image_generation(
            ImageGenerationRequest {
                prompt: "paint a lighthouse".to_string(),
                model: Some("stable-image-core".to_string()),
                n: Some(1),
                size: None,
                quality: None,
                response_format: Some("png".to_string()),
                style: None,
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("truncated generation response should fail");
    let edit_error = provider
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
        .expect_err("truncated edit response should fail");
    server.await.expect("mock server should finish");

    assert!(matches!(generation_error, ProviderError::Other { .. }));
    assert!(matches!(edit_error, ProviderError::Other { .. }));
}

#[tokio::test]
async fn stability_generation_and_edit_map_403_to_content_filtered() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Stability listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let _request = read_http_request(&mut socket).await;
            let body = r#"{"name":"content_moderation","errors":["flagged"]}"#;
            let response = format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("moderation response should write");
        }
    });
    let mut config = StabilityConfig::with_api_key("stability-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = StabilityProvider::new(config).expect("provider should initialize");

    let generation_error = provider
        .image_generation(
            ImageGenerationRequest {
                prompt: "moderated prompt".to_string(),
                model: Some("stable-image-core".to_string()),
                n: Some(1),
                size: None,
                quality: None,
                response_format: Some("png".to_string()),
                style: None,
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("403 generation should be content-filtered");
    let edit_error = provider
        .image_edit(
            ImageEditRequest {
                image: b"source-image".to_vec(),
                mask: None,
                prompt: "moderated prompt".to_string(),
                model: Some("inpaint".to_string()),
                n: Some(1),
                size: None,
                response_format: Some("png".to_string()),
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("403 edit should be content-filtered");
    server.await.expect("mock server should finish");

    assert!(matches!(
        generation_error,
        ProviderError::ContentFiltered { .. }
    ));
    assert!(matches!(edit_error, ProviderError::ContentFiltered { .. }));
}

#[tokio::test]
async fn stability_generic_403_is_not_content_filtered() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Stability listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request should arrive");
        let _request = read_http_request(&mut socket).await;
        let body = r#"{"name":"forbidden","errors":["proxy policy"]}"#;
        let response = format!(
            "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("generic 403 response should write");
    });
    let mut config = StabilityConfig::with_api_key("stability-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = StabilityProvider::new(config).expect("provider should initialize");

    let error = provider
        .image_generation(
            ImageGenerationRequest {
                prompt: "paint a lighthouse".to_string(),
                model: Some("stable-image-core".to_string()),
                n: Some(1),
                size: None,
                quality: None,
                response_format: Some("png".to_string()),
                style: None,
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("generic 403 should fail");
    server.await.expect("mock server should finish");

    assert!(matches!(error, ProviderError::ApiError { status: 403, .. }));
}

#[tokio::test]
async fn stability_empty_success_bodies_are_response_parsing_errors() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Stability listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let _request = read_http_request(&mut socket).await;
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .expect("empty response should write");
        }
    });
    let mut config = StabilityConfig::with_api_key("stability-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = StabilityProvider::new(config).expect("provider should initialize");

    let generation_error = provider
        .image_generation(
            ImageGenerationRequest {
                prompt: "paint a lighthouse".to_string(),
                model: Some("stable-image-core".to_string()),
                n: Some(1),
                size: None,
                quality: None,
                response_format: Some("png".to_string()),
                style: None,
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("empty generation body should fail");
    let edit_error = provider
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
        .expect_err("empty edit body should fail");
    server.await.expect("mock server should finish");

    assert!(matches!(
        generation_error,
        ProviderError::ResponseParsing { .. }
    ));
    assert!(matches!(edit_error, ProviderError::ResponseParsing { .. }));
}

#[tokio::test]
async fn native_image_factory_merges_gateway_custom_headers() {
    for (name, provider_type) in [
        ("stability", "stability"),
        ("black_forest_labs", "black_forest_labs"),
    ] {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("mock native listener should bind");
        let address = listener.local_addr().expect("mock address should exist");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let request = read_http_request(&mut socket).await;
            if request.starts_with("POST /v2beta/") {
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 8\r\nConnection: close\r\n\r\n\x89PNG\r\n\x1a\n",
                    )
                    .await
                    .expect("Stability response should write");
            } else {
                let body =
                    format!(r#"{{"id":"task-1","polling_url":"http://{address}/poll/task-1"}}"#);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("BFL submit response should write");
            }
            request
        });
        let mut gateway_config = litellm_rs::config::models::provider::ProviderConfig {
            name: name.to_string(),
            provider_type: provider_type.to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(format!("http://{address}")),
            endpoint_access: ProviderEndpointAccess::PrivateNetwork,
            ..Default::default()
        };
        gateway_config.settings.insert(
            "custom_headers".to_string(),
            serde_json::json!({"x-native-route": "configured"}),
        );
        let provider = litellm_rs::core::providers::create_provider(gateway_config)
            .await
            .expect("native provider should initialize");
        let result = provider
            .create_images(
                ImageGenerationRequest {
                    prompt: "paint a lighthouse".to_string(),
                    model: Some(if provider_type == "stability" {
                        "stable-image-core".to_string()
                    } else {
                        "flux-pro-1.1".to_string()
                    }),
                    n: Some(1),
                    size: None,
                    quality: None,
                    response_format: Some(if provider_type == "stability" {
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
        assert!(request.contains("x-native-route: configured"));
        if provider_type == "stability" {
            result.expect("Stability request should succeed");
        } else {
            result.expect_err("BFL poll is intentionally unavailable");
        }
    }
}

#[path = "media_native_review_loop_tests.rs"]
mod review_loop_tests;

#[path = "media_native_boundary_tests.rs"]
mod boundary_tests;

#[path = "media_native_final_invariant_tests.rs"]
mod final_invariant_tests;

#[tokio::test]
async fn bfl_does_not_advertise_unpriced_flux_2_models() {
    let provider = BflProvider::new(BflConfig::with_api_key("bfl-secret"))
        .expect("BFL provider should initialize");
    let model_ids = provider
        .models()
        .iter()
        .map(|model| model.id.as_str())
        .collect::<Vec<_>>();

    assert!(!model_ids.contains(&"flux-2-pro"));
    assert!(!model_ids.contains(&"flux-2-flex"));
    assert!(!model_ids.contains(&"flux-2-dev"));
}

#[tokio::test]
async fn media_factory_maps_gateway_base_url_for_native_providers() {
    for (name, provider_type) in [
        ("stability", "stability"),
        ("black_forest_labs", "black_forest_labs"),
    ] {
        let provider = litellm_rs::core::providers::create_provider(
            litellm_rs::config::models::provider::ProviderConfig {
                name: name.to_string(),
                provider_type: provider_type.to_string(),
                api_key: "test-key".to_string(),
                base_url: Some("http://127.0.0.1:9".to_string()),
                endpoint_access: ProviderEndpointAccess::PrivateNetwork,
                ..Default::default()
            },
        )
        .await
        .unwrap_or_else(|error| {
            panic!("{name} factory should preserve the gateway endpoint: {error}")
        });

        assert!(matches!(
            (provider_type, provider),
            ("stability", Provider::Stability(_))
                | ("black_forest_labs", Provider::BlackForestLabs(_))
        ));
    }
}

#[cfg(feature = "runway-media")]
#[tokio::test]
async fn runway_accepts_official_cancelled_status() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Runway listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("query should arrive");
        let _request = read_http_request(&mut socket).await;
        let body = r#"{"id":"task-1","status":"CANCELLED"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("response should write");
    });

    let mut config = RunwayConfig::with_api_key("runway-secret");
    config.base.api_base = Some(format!("http://{address}/v1"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = RunwayProvider::new(config).expect("Runway provider should initialize");
    let task = provider
        .get_task("task-1")
        .await
        .expect("official CANCELLED status should decode");
    server.await.expect("mock server should finish");

    assert_eq!(task.status, RunwayTaskStatus::Canceled);
}
