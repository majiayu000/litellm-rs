use super::*;

#[tokio::test]
async fn stability_rejects_non_image_success_bodies_for_generation_and_edit() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Stability listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let _request = read_http_request(&mut socket).await;
            let body = br#"{"error":"proxy returned JSON with a 2xx status"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("response headers should write");
            socket
                .write_all(body)
                .await
                .expect("response body should write");
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
        .expect_err("JSON generation body must not be returned as an image");
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
        .expect_err("JSON edit body must not be returned as an image");
    server.await.expect("mock server should finish");

    assert!(matches!(
        generation_error,
        ProviderError::ResponseParsing { .. }
    ));
    assert!(matches!(edit_error, ProviderError::ResponseParsing { .. }));
}

#[tokio::test]
async fn stability_requires_requested_raster_signature_for_success_bodies() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Stability listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for (content_type, body) in [
            ("image/png", b"<html>not an image</html>".as_slice()),
            ("image/svg+xml", b"<svg><script/></svg>".as_slice()),
            ("image/jpeg", b"\x89PNG\r\n\x1a\n".as_slice()),
        ] {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let _request = read_http_request(&mut socket).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("response headers should write");
            socket
                .write_all(body)
                .await
                .expect("response body should write");
        }
    });
    let mut config = StabilityConfig::with_api_key("stability-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = StabilityProvider::new(config).expect("provider should initialize");

    let html_error = provider
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
        .expect_err("HTML mislabeled image/png must be rejected");
    let svg_error = provider
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
        .expect_err("SVG success body must be rejected");
    let mismatch_error = provider
        .image_generation(
            ImageGenerationRequest {
                prompt: "paint a lighthouse".to_string(),
                model: Some("stable-image-core".to_string()),
                n: Some(1),
                size: None,
                quality: None,
                response_format: Some("jpeg".to_string()),
                style: None,
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("PNG signature must not satisfy a JPEG request");
    server.await.expect("mock server should finish");

    for error in [html_error, svg_error, mismatch_error] {
        assert!(matches!(error, ProviderError::ResponseParsing { .. }));
    }
}

#[test]
fn credentialed_media_clients_are_all_configured_without_redirects() {
    let stability = include_str!("../../src/core/providers/stability/mod.rs");
    let bfl = include_str!("../../src/core/providers/black_forest_labs/mod.rs");
    let runway = include_str!("../../src/core/providers/media.rs");

    assert!(stability.contains("BaseHttpClient::new_for_provider_no_redirect"));
    assert!(bfl.contains("client: BaseHttpClient::new_for_provider_no_redirect"));
    assert!(bfl.contains("GenerationLifecycle::new_no_redirect"));
    assert!(runway.contains("client: BaseHttpClient::new_for_provider_no_redirect"));
    assert!(runway.contains("lifecycle: GenerationLifecycle::new_no_redirect"));
}

#[tokio::test]
async fn stability_generation_and_edit_redirects_do_not_forward_custom_headers() {
    for edit in [false, true] {
        let source_listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("source listener should bind");
        let source_address = source_listener.local_addr().expect("source address");
        let target_listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("target listener should bind");
        let target_address = target_listener.local_addr().expect("target address");
        let source = tokio::spawn(async move {
            let (mut socket, _) = source_listener
                .accept()
                .await
                .expect("request should arrive");
            let request = read_http_request(&mut socket).await;
            assert!(request.contains("x-stability-route: credentialed"));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer")
            );
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/foreign\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("redirect should write");
        });
        let target = tokio::spawn(async move {
            tokio::time::timeout(std::time::Duration::from_secs(1), target_listener.accept()).await
        });

        let mut config = StabilityConfig::with_api_key("stability-secret");
        config.base.api_base = Some(format!("http://{source_address}"));
        config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        config
            .base
            .headers
            .insert("x-stability-route".to_string(), "credentialed".to_string());
        let provider = StabilityProvider::new(config).expect("provider should initialize");
        if edit {
            provider
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
                .expect_err("edit redirect must not be followed");
        } else {
            provider
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
                .expect_err("generation redirect must not be followed");
        }
        source.await.expect("source server should finish");
        assert!(
            target.await.expect("target server should finish").is_err(),
            "redirect target received Stability custom headers"
        );
    }
}

#[cfg(feature = "runway-media")]
#[tokio::test]
async fn every_runway_request_path_rejects_cross_origin_redirects() {
    for operation in ["submit", "query", "cancel", "poll"] {
        let source_listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("source listener should bind");
        let source_address = source_listener.local_addr().expect("source address");
        let target_listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("target listener should bind");
        let target_address = target_listener.local_addr().expect("target address");
        let source = tokio::spawn(async move {
            let (mut socket, _) = source_listener
                .accept()
                .await
                .expect("request should arrive");
            let request = read_http_request(&mut socket).await;
            assert!(request.contains("x-runway-route: credentialed"));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer")
            );
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/foreign\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("redirect should write");
        });
        let target = tokio::spawn(async move {
            tokio::time::timeout(std::time::Duration::from_secs(1), target_listener.accept()).await
        });

        let mut config = RunwayConfig::with_api_key("runway-secret");
        config.base.api_base = Some(format!("http://{source_address}/v1"));
        config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        config
            .base
            .headers
            .insert("x-runway-route".to_string(), "credentialed".to_string());
        config.poll_policy = PollPolicy::from_millis(1, 2, 100);
        let provider = RunwayProvider::new(config).expect("Runway provider should initialize");
        match operation {
            "submit" => {
                provider
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
                    .expect_err("submit redirect must not be followed");
            }
            "query" => {
                provider
                    .get_task("task-1")
                    .await
                    .expect_err("query redirect must not be followed");
            }
            "cancel" => {
                provider
                    .cancel_task("task-1")
                    .await
                    .expect_err("cancel redirect must not be followed");
            }
            "poll" => {
                provider
                    .wait_for_task("task-1", &CancellationToken::new())
                    .await
                    .expect_err("poll redirect must not be followed");
            }
            _ => unreachable!(),
        }
        source.await.expect("source server should finish");
        assert!(
            target.await.expect("target server should finish").is_err(),
            "redirect target received Runway custom headers for {operation}"
        );
    }
}
