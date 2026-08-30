#![cfg(feature = "providers-extended")]

use litellm_rs::core::net::ProviderEndpointAccess;
use litellm_rs::core::providers::base::{BaseConfig, header_static};
use litellm_rs::core::providers::bfl::{BflConfig, BflImageRequest, BflProvider};
use litellm_rs::core::providers::media::{
    GenerationLifecycle, GenerationOutput, GenerationPoll, PollPolicy,
};
use litellm_rs::core::providers::stability::{StabilityConfig, StabilityProvider};
use litellm_rs::core::providers::unified_provider::ProviderError;
use litellm_rs::core::providers::{LLMProvider, Provider, ProviderType};
use litellm_rs::core::types::context::RequestContext;
use litellm_rs::core::types::health::HealthStatus;
use litellm_rs::core::types::image::{ImageEditRequest, ImageGenerationRequest};
use litellm_rs::core::types::model::ProviderCapability;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[cfg(feature = "runway-media")]
use litellm_rs::core::providers::runway::{
    RunwayConfig, RunwayImageToVideoRequest, RunwayProvider, RunwayTaskStatus,
};

#[tokio::test]
async fn stability_generation_uses_native_multipart_contract() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Stability listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_server = Arc::clone(&captured);
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request should arrive");
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = socket.read(&mut buffer).await.expect("request should read");
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + length {
                    break;
                }
            }
        }
        *captured_for_server.lock().expect("capture lock") = bytes;
        let body = b"\x89PNG\r\n\x1a\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
    });

    let mut config = StabilityConfig::with_api_key("  stability-secret  ");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config
        .base
        .headers
        .insert("x-stability-route".to_string(), "generation".to_string());
    config.base.headers.insert(
        "Authorization".to_string(),
        "Bearer attacker-controlled".to_string(),
    );
    let provider = StabilityProvider::new(config).expect("provider should initialize");
    let response = provider
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
        .expect("native Stability generation should succeed");
    server.await.expect("mock server should finish");

    assert_eq!(response.data.len(), 1);
    assert!(response.data[0].b64_json.is_some());
    let captured_request = captured.lock().expect("capture lock");
    let request = String::from_utf8_lossy(&captured_request);
    assert!(request.starts_with("POST /v2beta/stable-image/generate/core HTTP/1.1"));
    assert!(request.contains("name=\"aspect_ratio\""));
    assert!(request.contains("1:1"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer stability-secret")
    );
    assert!(request.to_ascii_lowercase().contains("accept: image/*"));
    assert!(request.contains("x-stability-route: generation"));
    assert!(!request.contains("attacker-controlled"));
    assert!(request.contains("paint a lighthouse"));
    assert!(!request.contains("stability-secret\r\n\r\n"));
}

#[tokio::test]
async fn stability_rejects_openai_style_before_network_access() {
    let mut config = StabilityConfig::with_api_key("stability-secret");
    config.base.api_base = Some("http://127.0.0.1:1".to_string());
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
                style: Some("vivid".to_string()),
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("OpenAI style must not be forwarded as a Stability style preset");

    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
}

#[tokio::test]
async fn stability_edit_uses_native_inpaint_multipart_contract() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Stability edit listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let captured = Arc::new(Mutex::new(String::new()));
    let captured_for_server = Arc::clone(&captured);
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("edit should arrive");
        *captured_for_server.lock().expect("capture lock") = read_http_request(&mut socket).await;
        let body = b"\x89PNG\r\n\x1a\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("headers should write");
        socket.write_all(body).await.expect("body should write");
    });

    let mut config = StabilityConfig::with_api_key("stability-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config
        .base
        .headers
        .insert("x-stability-route".to_string(), "edit".to_string());
    config.base.headers.insert(
        "authorization".to_string(),
        "Bearer attacker-controlled".to_string(),
    );
    let provider = StabilityProvider::new(config).expect("provider should initialize");
    provider
        .image_edit(
            ImageEditRequest {
                image: b"source-image".to_vec(),
                mask: Some(b"mask-image".to_vec()),
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
        .expect("native Stability edit should succeed");
    server.await.expect("mock server should finish");

    let request = captured.lock().expect("capture lock");
    assert!(request.starts_with("POST /v2beta/stable-image/edit/inpaint HTTP/1.1"));
    assert!(request.contains("replace the background"));
    assert!(request.contains("source-image"));
    assert!(request.contains("mask-image"));
    assert!(request.contains("x-stability-route: edit"));
    assert!(request.contains("authorization: Bearer stability-secret"));
    assert!(!request.contains("attacker-controlled"));
}

#[tokio::test]
async fn stability_edit_rejects_unmapped_size_before_network_access() {
    let mut config = StabilityConfig::with_api_key("stability-secret");
    config.base.api_base = Some("http://127.0.0.1:1".to_string());
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = StabilityProvider::new(config).expect("provider should initialize");

    let error = provider
        .image_edit(
            ImageEditRequest {
                image: b"source-image".to_vec(),
                mask: None,
                prompt: "replace the background".to_string(),
                model: Some("inpaint".to_string()),
                n: Some(1),
                size: Some("1024x1024".to_string()),
                response_format: Some("png".to_string()),
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("Stability edit size must not be silently ignored");

    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    assert!(error.to_string().contains("size"));
}

#[tokio::test]
async fn generation_lifecycle_polls_until_success() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock polling listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for (index, body) in [
            r#"{"status":"RUNNING"}"#,
            r#"{"status":"SUCCEEDED","output":["https://cdn.example/result.mp4"]}"#,
        ]
        .into_iter()
        .enumerate()
        {
            let (mut socket, _) = listener.accept().await.expect("poll should arrive");
            let mut buffer = [0_u8; 2048];
            let read = socket.read(&mut buffer).await.expect("poll should read");
            let request = String::from_utf8_lossy(&buffer[..read]);
            assert!(request.starts_with("GET /task-1 HTTP/1.1"));
            assert!(request.contains("x-test-key: secret"));
            assert_eq!(index, usize::from(body.contains("SUCCEEDED")));
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("poll response should write");
        }
    });

    let lifecycle = test_lifecycle(address.to_string());
    let output = lifecycle
        .wait_for_json(
            format!("http://{address}/task-1"),
            vec![header_static("x-test-key", "secret")],
            &CancellationToken::new(),
            |payload| match payload["status"].as_str() {
                Some("RUNNING") => Ok(GenerationPoll::Pending),
                Some("SUCCEEDED") => Ok(GenerationPoll::Succeeded(GenerationOutput {
                    urls: payload["output"]
                        .as_array()
                        .expect("output should be an array")
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_string))
                        .collect(),
                    credits_used: None,
                })),
                status => Err(ProviderError::response_parsing(
                    "test_media",
                    format!("unexpected status: {status:?}"),
                )),
            },
        )
        .await
        .expect("polling should finish");
    server.await.expect("mock polling server should finish");

    assert_eq!(output.urls, ["https://cdn.example/result.mp4"]);
}

#[tokio::test]
async fn generation_lifecycle_honors_cancellation_before_polling() {
    let lifecycle = test_lifecycle("127.0.0.1:1".to_string());
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let error = lifecycle
        .wait_for_json(
            "http://127.0.0.1:1/task-1".to_string(),
            Vec::new(),
            &cancellation,
            |_| Ok(GenerationPoll::Pending),
        )
        .await
        .expect_err("cancellation should stop before network access");

    assert!(matches!(error, ProviderError::Cancelled { .. }));
}

#[tokio::test]
async fn generation_lifecycle_preserves_failure_direction() {
    for (poll, expected_client_error) in [
        (GenerationPoll::Failed("upstream failed".to_string()), false),
        (
            GenerationPoll::Rejected("request rejected".to_string()),
            true,
        ),
    ] {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("mock polling listener should bind");
        let address = listener.local_addr().expect("mock address should exist");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("poll should arrive");
            let _request = read_http_request(&mut socket).await;
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .await
                .expect("response should write");
        });
        let lifecycle = test_lifecycle(address.to_string());
        let error = lifecycle
            .wait_for_json(
                format!("http://{address}/task"),
                Vec::new(),
                &CancellationToken::new(),
                move |_| Ok(poll.clone()),
            )
            .await
            .expect_err("terminal failure should be returned");
        server.await.expect("mock server should finish");

        if expected_client_error {
            assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        } else {
            assert!(matches!(error, ProviderError::ApiError { status: 502, .. }));
        }
    }
}

#[tokio::test]
async fn generation_lifecycle_rejects_metadata_polling_url() {
    let lifecycle = GenerationLifecycle::new(
        "test_media",
        BaseConfig::default(),
        PollPolicy::from_millis(1, 4, 200),
    )
    .expect("lifecycle should initialize");

    let error = lifecycle
        .wait_for_json(
            "http://169.254.169.254/latest/meta-data".to_string(),
            Vec::new(),
            &CancellationToken::new(),
            |_| Ok(GenerationPoll::Pending),
        )
        .await
        .expect_err("metadata URL must be rejected before polling");

    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(error.to_string().contains("SSRF protection"));
}

fn test_lifecycle(api_base: String) -> GenerationLifecycle {
    let config = BaseConfig {
        api_base: Some(format!("http://{api_base}")),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        ..BaseConfig::default()
    };
    GenerationLifecycle::new("test_media", config, PollPolicy::from_millis(1, 4, 200))
        .expect("test lifecycle should initialize")
}

#[tokio::test]
async fn bfl_generation_and_edit_use_submit_poll_contract() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured_for_server = Arc::clone(&captured);
    let server = tokio::spawn(async move {
        for index in 0..4 {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let request = read_http_request(&mut socket).await;
            captured_for_server
                .lock()
                .expect("capture lock")
                .push(request);
            let body = if index % 2 == 0 {
                format!(r#"{{"id":"task-{index}","polling_url":"http://{address}/poll/{index}"}}"#)
            } else {
                format!(
                    r#"{{"status":"Ready","result":{{"sample":"https://cdn.example/{index}.png"}},"cost":4.0}}"#
                )
            };
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

    let mut config = BflConfig::with_api_key("  bfl-secret  ");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config
        .base
        .headers
        .insert("x-bfl-route".to_string(), "media".to_string());
    config
        .base
        .headers
        .insert("X-Key".to_string(), "attacker-controlled".to_string());
    config.poll_policy = PollPolicy::from_millis(1, 4, 200);
    let provider = BflProvider::new(config).expect("BFL provider should initialize");
    let generation = provider
        .image_generation(
            ImageGenerationRequest {
                prompt: "a glass city".to_string(),
                model: Some("flux-pro-1.1".to_string()),
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
        .expect("BFL generation should finish");
    let edit = provider
        .image_edit(
            ImageEditRequest {
                image: b"source-image".to_vec(),
                mask: None,
                prompt: "make the sky green".to_string(),
                model: Some("flux-kontext-pro".to_string()),
                n: Some(1),
                size: Some("1024x1024".to_string()),
                response_format: Some("url".to_string()),
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect("BFL edit should finish");
    server.await.expect("mock BFL server should finish");

    assert_eq!(
        generation.data[0].url.as_deref(),
        Some("https://cdn.example/1.png")
    );
    assert_eq!(
        edit.data[0].url.as_deref(),
        Some("https://cdn.example/3.png")
    );
    let requests = captured.lock().expect("capture lock");
    assert!(requests[0].starts_with("POST /flux-pro-1.1 HTTP/1.1"));
    assert!(requests[0].contains("\"prompt\":\"a glass city\""));
    assert!(requests[0].contains("\"width\":1024"));
    assert!(requests[0].contains("\"height\":768"));
    assert!(requests[1].starts_with("GET /poll/0 HTTP/1.1"));
    assert!(requests[2].starts_with("POST /flux-kontext-pro HTTP/1.1"));
    assert!(requests[2].contains("\"input_image\":"));
    assert!(requests[2].contains("\"aspect_ratio\":\"1:1\""));
    assert!(
        requests
            .iter()
            .all(|request| request.contains("x-key: bfl-secret"))
    );
    assert!(
        requests
            .iter()
            .all(|request| request.contains("x-bfl-route: media"))
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.contains("attacker-controlled"))
    );
}

#[tokio::test]
async fn stability_and_bfl_are_routeable_with_truthful_capabilities_and_pricing() {
    let stability = Provider::from_config_async(
        ProviderType::Stability,
        serde_json::json!({ "api_key": "stability-secret" }),
    )
    .await
    .expect("Stability factory should be wired");
    let bfl = Provider::from_config_async(
        ProviderType::BlackForestLabs,
        serde_json::json!({ "api_key": "bfl-secret" }),
    )
    .await
    .expect("BFL factory should be wired");

    assert!(matches!(stability, Provider::Stability(_)));
    assert!(matches!(bfl, Provider::BlackForestLabs(_)));
    assert!(
        stability.supports_capability(
            &litellm_rs::core::types::model::ProviderCapability::ImageGeneration
        )
    );
    assert!(
        stability
            .supports_capability(&litellm_rs::core::types::model::ProviderCapability::ImageEdit)
    );
    assert!(
        bfl.supports_capability(
            &litellm_rs::core::types::model::ProviderCapability::ImageGeneration
        )
    );
    assert!(
        bfl.supports_capability(&litellm_rs::core::types::model::ProviderCapability::ImageEdit)
    );
    assert!(
        stability.supports_capability_for_model(
            "stable-image-core",
            &ProviderCapability::ImageGeneration
        )
    );
    assert!(
        !stability
            .supports_capability_for_model("stable-image-core", &ProviderCapability::ImageEdit)
    );
    assert!(stability.supports_capability_for_model("inpaint", &ProviderCapability::ImageEdit));
    assert!(
        !stability.supports_capability_for_model("inpaint", &ProviderCapability::ImageGeneration)
    );
    assert!(
        !stability
            .supports_capability_for_model("unknown-model", &ProviderCapability::ImageGeneration)
    );
    assert!(
        bfl.supports_capability_for_model("flux-kontext-pro", &ProviderCapability::ImageGeneration)
    );
    assert!(bfl.supports_capability_for_model("flux-kontext-pro", &ProviderCapability::ImageEdit));
    assert!(
        bfl.supports_capability_for_model("flux-pro-1.1", &ProviderCapability::ImageGeneration)
    );
    assert!(!bfl.supports_capability_for_model("flux-pro-1.1", &ProviderCapability::ImageEdit));
    assert!(
        !bfl.supports_capability_for_model("unknown-model", &ProviderCapability::ImageGeneration)
    );
    assert_eq!(stability.health_check().await, HealthStatus::Unknown);
    assert_eq!(bfl.health_check().await, HealthStatus::Unknown);
    assert!(matches!(
        stability.calculate_cost("stable-image-core", 10, 20).await,
        Err(ProviderError::NotSupported { .. })
    ));
    assert!(matches!(
        bfl.calculate_cost("flux-pro-1.1", 10, 20).await,
        Err(ProviderError::NotSupported { .. })
    ));
}

#[tokio::test]
async fn bfl_edit_rejects_non_kontext_model_before_network_access() {
    let mut config = BflConfig::with_api_key("bfl-secret");
    config.base.api_base = Some("http://127.0.0.1:1".to_string());
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = BflProvider::new(config).expect("BFL provider should initialize");

    let error = provider
        .image_edit(
            ImageEditRequest {
                image: b"source-image".to_vec(),
                mask: None,
                prompt: "edit this image".to_string(),
                model: Some("flux-pro-1.1".to_string()),
                n: Some(1),
                size: None,
                response_format: Some("url".to_string()),
                user: None,
            },
            RequestContext::default(),
        )
        .await
        .expect_err("non-Kontext models must fail before any network request");

    assert!(matches!(error, ProviderError::NotSupported { .. }));
}

#[tokio::test]
async fn bfl_rejects_invalid_webhook_as_client_input() {
    let provider = BflProvider::new(BflConfig::with_api_key("bfl-secret"))
        .expect("BFL provider should initialize");
    let mut request = BflImageRequest::new("flux-pro-1.1", "a glass city");
    request.parameters.insert(
        "webhook_url".to_string(),
        serde_json::json!("http://169.254.169.254/latest/meta-data"),
    );

    let error = provider
        .generate_native(request, &CancellationToken::new())
        .await
        .expect_err("metadata webhook must be rejected before submission");

    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
}

#[tokio::test]
async fn bfl_cancellation_interrupts_submission() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let (request_seen_tx, request_seen_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("submit should arrive");
        let _request = read_http_request(&mut socket).await;
        let _ = request_seen_tx.send(());
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
    request_seen_rx
        .await
        .expect("submission should reach server");
    cancellation.cancel();
    let error = request
        .await
        .expect("request task should finish")
        .expect_err("cancellation must interrupt an in-flight submit");
    server.await.expect("mock server should finish");

    assert!(matches!(error, ProviderError::Cancelled { .. }));
}

async fn read_http_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = socket.read(&mut buffer).await.expect("request should read");
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|value| value.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if bytes.len() >= header_end + 4 + length {
                break;
            }
        }
    }
    String::from_utf8(bytes).expect("request should be UTF-8")
}

#[cfg(feature = "runway-media")]
#[tokio::test]
async fn runway_submit_query_cancel_and_wait_use_native_task_contract() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Runway listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_for_server = Arc::clone(&requests);
    let server = tokio::spawn(async move {
        let responses = [
            r#"{"id":"task-1"}"#,
            r#"{"id":"task-1","status":"SUCCEEDED","output":["https://cdn.example/video.mp4"]}"#,
            "",
            r#"{"id":"task-1","status":"THROTTLED"}"#,
            r#"{"id":"task-1","status":"SUCCEEDED","output":["https://cdn.example/video.mp4"]}"#,
        ];
        for (index, body) in responses.into_iter().enumerate() {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let request = read_http_request(&mut socket).await;
            requests_for_server
                .lock()
                .expect("capture lock")
                .push(request);
            let status = if index == 2 {
                "204 No Content"
            } else {
                "200 OK"
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("response should write");
        }
    });

    let mut config = RunwayConfig::with_api_key("  runway-secret  ");
    config.base.api_base = Some(format!("http://{address}/v1"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config
        .base
        .headers
        .insert("x-runway-route".to_string(), "configured".to_string());
    config.base.headers.insert(
        "authorization".to_string(),
        "Bearer attacker-controlled".to_string(),
    );
    config.base.headers.insert(
        "X-Runway-Version".to_string(),
        "attacker-controlled".to_string(),
    );
    config.poll_policy = PollPolicy::from_millis(1, 4, 200);
    let provider = RunwayProvider::new(config).expect("Runway provider should initialize");
    let task = provider
        .submit_image_to_video(RunwayImageToVideoRequest {
            model: "gen4_turbo".to_string(),
            prompt_image: serde_json::json!("https://images.example/source.png"),
            prompt_text: Some("slow camera push".to_string()),
            ratio: Some("1280:720".to_string()),
            duration: Some(5),
            seed: None,
            extra: serde_json::Map::new(),
        })
        .await
        .expect("submit should succeed");
    let queried = provider
        .get_task(&task.id)
        .await
        .expect("query should succeed");
    assert_eq!(queried.status, RunwayTaskStatus::Succeeded);
    assert_eq!(queried.output, ["https://cdn.example/video.mp4"]);
    provider
        .cancel_task(&task.id)
        .await
        .expect("cancel should succeed");
    let output = provider
        .wait_for_task(&task.id, &CancellationToken::new())
        .await
        .expect("shared lifecycle should finish");
    server.await.expect("mock Runway server should finish");

    assert_eq!(output.urls, ["https://cdn.example/video.mp4"]);
    let requests = requests.lock().expect("capture lock");
    assert!(requests[0].starts_with("POST /v1/image_to_video HTTP/1.1"));
    assert!(requests[1].starts_with("GET /v1/tasks/task-1 HTTP/1.1"));
    assert!(requests[2].starts_with("DELETE /v1/tasks/task-1 HTTP/1.1"));
    assert!(requests[0].contains("authorization: Bearer runway-secret"));
    assert!(requests[0].contains("x-runway-version: 2024-11-06"));
    assert!(requests[0].contains("\"promptImage\":\"https://images.example/source.png\""));
    for request in requests.iter() {
        assert!(request.contains("x-runway-route: configured"));
        assert!(request.contains("authorization: Bearer runway-secret"));
        assert!(request.contains("x-runway-version: 2024-11-06"));
        assert!(!request.contains("attacker-controlled"));
    }
}

#[cfg(feature = "runway-media")]
#[tokio::test]
async fn runway_rejects_malformed_succeeded_output() {
    for body in [
        r#"{"id":"task-1","status":"SUCCEEDED","output":[]}"#,
        r#"{"id":"task-1","status":"SUCCEEDED","output":["https://cdn.example/video.mp4",7]}"#,
        r#"{"id":"task-1","status":"SUCCEEDED","output":["not-a-url"]}"#,
    ] {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("mock Runway listener should bind");
        let address = listener.local_addr().expect("mock address should exist");
        let body = body.to_string();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let _request = read_http_request(&mut socket).await;
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
        let error = provider
            .get_task("task-1")
            .await
            .expect_err("malformed successful output must remain an error");
        server.await.expect("mock Runway server should finish");

        assert!(matches!(error, ProviderError::ResponseParsing { .. }));
    }
}

#[cfg(feature = "runway-media")]
#[tokio::test]
async fn runway_rejects_invalid_submit_task_id() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Runway listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("submit should arrive");
        let _request = read_http_request(&mut socket).await;
        let body = r#"{"id":"../tasks/other"}"#;
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

    let error = provider
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
        .expect_err("invalid submit task ID must be rejected immediately");
    server.await.expect("mock server should finish");

    assert!(matches!(error, ProviderError::ResponseParsing { .. }));
}

#[path = "tests/media_native_followup_tests.rs"]
mod followup_tests;
