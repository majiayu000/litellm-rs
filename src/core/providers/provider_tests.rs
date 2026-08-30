use super::*;
use crate::core::net::ProviderEndpointAccess;
use crate::core::types::context::RequestContext;
use crate::core::types::image::ImageEditRequest;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn read_full_request(socket: &mut TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = socket.read(&mut buffer).await.expect("request should read");
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            break;
        }
    }
    String::from_utf8_lossy(&request).into_owned()
}

fn image_edit_request() -> ImageEditRequest {
    ImageEditRequest {
        image: b"source-image".to_vec(),
        mask: Some(b"mask-image".to_vec()),
        prompt: "replace the sky".to_string(),
        model: Some("gpt-image-1".to_string()),
        n: Some(1),
        size: Some("1024x1024".to_string()),
        response_format: Some("b64_json".to_string()),
        user: Some("test-user".to_string()),
    }
}

async fn redirecting_image_edit_server(
    status: u16,
    reason: &'static str,
) -> (String, tokio::task::JoinHandle<Option<String>>) {
    let source = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("redirect source should bind");
    let source_address = source.local_addr().expect("source address should exist");
    let sink = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("redirect sink should bind");
    let sink_address = sink.local_addr().expect("sink address should exist");
    let server = tokio::spawn(async move {
        let (mut source_socket, _) = source.accept().await.expect("edit request should arrive");
        let _request = read_full_request(&mut source_socket).await;
        source_socket
            .write_all(
                format!(
                    "HTTP/1.1 {status} {reason}\r\nLocation: http://{sink_address}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .expect("redirect should write");

        let Ok(Ok((mut sink_socket, _))) =
            tokio::time::timeout(Duration::from_millis(250), sink.accept()).await
        else {
            return None;
        };
        let request = read_full_request(&mut sink_socket).await;
        let body = r#"{"created":1,"data":[]}"#;
        sink_socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .expect("sink response should write");
        Some(request)
    });
    (format!("http://{source_address}/v1"), server)
}

#[test]
fn test_provider_enum_is_send_sync() {
    assert!(matches!(ProviderType::from("openai"), ProviderType::OpenAI));
}

#[tokio::test]
async fn test_provider_capabilities_embeddings_error_names_real_provider() {
    let provider = Provider::Anthropic(
        anthropic::AnthropicProvider::new(anthropic::AnthropicConfig::new_test("test-key"))
            .unwrap(),
    );

    assert!(!provider.supports_capability(&ProviderCapability::Embeddings));

    let err = provider
        .create_embeddings(
            crate::core::types::embedding::EmbeddingRequest {
                model: "claude-3-opus-20240229".to_string(),
                input: crate::core::types::embedding::EmbeddingInput::Text("hello".to_string()),
                user: None,
                encoding_format: None,
                dimensions: None,
                task_type: None,
                truncation: None,
            },
            crate::core::types::context::RequestContext::default(),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            ProviderError::NotSupported {
                provider: "anthropic",
                ..
            }
        ),
        "expected provider-specific NotSupported, got {err}"
    );
}

#[tokio::test]
async fn test_provider_enum_calculate_cost_delegates_mistral_aliases() {
    let Ok(mistral_provider) = mistral::MistralProvider::new(mistral::MistralConfig {
        api_key: "sk-test".to_string(),
        ..mistral::MistralConfig::default()
    })
    .await
    else {
        panic!("Mistral provider should initialize with a test API key");
    };
    let provider = Provider::Mistral(mistral_provider);

    let Ok(alias_cost) = provider
        .calculate_cost("magistral-medium-1-2", 1000, 500)
        .await
    else {
        panic!("Mistral alias cost should calculate");
    };
    let Ok(canonical_cost) = provider
        .calculate_cost("magistral-medium-2509", 1000, 500)
        .await
    else {
        panic!("Mistral canonical cost should calculate");
    };
    let Ok(devstral_alias_cost) = provider.calculate_cost("devstral-2-2512", 1000, 500).await
    else {
        panic!("Devstral alias cost should calculate");
    };

    assert!((alias_cost - canonical_cost).abs() < 1e-12);
    assert!((alias_cost - 0.0045).abs() < 1e-12);
    assert!((devstral_alias_cost - 0.0014).abs() < 1e-12);
}

#[tokio::test]
async fn test_provider_enum_calculate_cost_strips_openai_prefix() {
    let mut config = openai::OpenAIConfig::default();
    config.base.api_key = Some("sk-test123456789012345678901234567890123456".to_string());
    let Ok(openai_provider) = openai::OpenAIProvider::new(config).await else {
        panic!("OpenAI provider should initialize with a test API key");
    };
    let provider = Provider::OpenAI(openai_provider);

    let Ok(cost) = provider
        .calculate_cost("openai/gpt-5.5-pro", 1000, 500)
        .await
    else {
        panic!("prefixed OpenAI cost should calculate");
    };

    assert!((cost - 0.12).abs() < 1e-12);
}

#[tokio::test]
async fn test_provider_capabilities_image_error_names_real_provider() {
    let provider = Provider::Anthropic(
        anthropic::AnthropicProvider::new(anthropic::AnthropicConfig::new_test("test-key"))
            .unwrap(),
    );

    assert!(!provider.supports_capability(&ProviderCapability::ImageGeneration));

    let err = provider
        .create_images(
            crate::core::types::image::ImageGenerationRequest {
                prompt: "a small test image".to_string(),
                model: Some("claude-3-opus-20240229".to_string()),
                n: None,
                size: None,
                quality: None,
                response_format: None,
                style: None,
                user: None,
            },
            crate::core::types::context::RequestContext::default(),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            ProviderError::NotSupported {
                provider: "anthropic",
                ..
            }
        ),
        "expected provider-specific NotSupported, got {err}"
    );
}

#[tokio::test]
async fn test_provider_supports_capability_for_optional_provider() {
    let mut config = openai::OpenAIConfig::default();
    config.base.api_key = Some("sk-test123456789012345678901234567890123456".to_string());
    let Ok(openai_provider) = openai::OpenAIProvider::new(config).await else {
        panic!("OpenAI provider should initialize with a test API key");
    };
    let provider = Provider::OpenAI(openai_provider);

    assert!(provider.supports_capability(&ProviderCapability::ChatCompletion));
    assert!(provider.supports_capability(&ProviderCapability::ChatCompletionStream));
    assert!(provider.supports_capability(&ProviderCapability::Embeddings));
    assert!(provider.supports_capability(&ProviderCapability::TextToSpeech));
}

#[tokio::test]
async fn advertised_openai_image_edit_variants_dispatch_upstream() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("image edit listener should bind");
    let address = listener
        .local_addr()
        .expect("listener address should exist");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_server = Arc::clone(&captured);
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.expect("edit request should arrive");
            let request = read_full_request(&mut socket).await;
            captured_for_server
                .lock()
                .expect("capture lock")
                .push(request);
            let body = r#"{"created":1,"data":[{"b64_json":"edited"}]}"#;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .expect("response should write");
        }
    });

    let mut openai_config = openai::OpenAIConfig::default();
    openai_config.base.api_key = Some("sk-test".to_string());
    openai_config.base.api_base = Some(format!("http://{address}/v1"));
    openai_config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let openai = Provider::OpenAI(
        openai::OpenAIProvider::new(openai_config)
            .await
            .expect("OpenAI provider should initialize"),
    );

    let mut compatible_config = openai_like::OpenAILikeConfig::with_api_key(
        format!("http://{address}/v1"),
        "compatible-secret",
    )
    .with_model_prefix("custom/");
    compatible_config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let compatible = Provider::OpenAILike(
        openai_like::OpenAILikeProvider::new_openai_compatible(compatible_config)
            .await
            .expect("OpenAI-compatible provider should initialize"),
    );

    for provider in [&openai, &compatible] {
        assert!(provider.supports_capability(&ProviderCapability::ImageEdit));
        let mut request = image_edit_request();
        request.model = Some("custom/gpt-image-1".to_string());
        let response = provider
            .edit_image(request, RequestContext::default())
            .await
            .expect("advertised image edit should dispatch upstream");
        assert_eq!(response.data[0].b64_json.as_deref(), Some("edited"));
    }
    server.await.expect("image edit server should finish");

    let captured = captured.lock().expect("capture lock");
    assert_eq!(captured.len(), 2);
    for request in captured.iter() {
        assert!(request.starts_with("POST /v1/images/edits HTTP/1.1"));
        assert!(request.contains("multipart/form-data; boundary="));
        assert!(request.contains("name=\"image\""));
        assert!(request.contains("source-image"));
        assert!(request.contains("name=\"mask\""));
        assert!(request.contains("mask-image"));
        assert!(request.contains("name=\"prompt\""));
        assert!(request.contains("replace the sky"));
        assert!(request.contains("name=\"model\""));
    }
    assert!(captured[0].contains("custom/gpt-image-1"));
    assert!(captured[1].contains("gpt-image-1"));
    assert!(!captured[1].contains("custom/gpt-image-1"));
}

#[tokio::test]
async fn credentialed_image_edits_do_not_follow_cross_origin_redirects() {
    let (openai_base, openai_server) =
        redirecting_image_edit_server(307, "Temporary Redirect").await;
    let mut openai_config = openai::OpenAIConfig::default();
    openai_config.base.api_key = Some("sk-test".to_string());
    openai_config.base.api_base = Some(openai_base);
    openai_config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    openai_config.base.headers.insert(
        "x-proxy-secret".to_string(),
        "openai-placeholder".to_string(),
    );
    let openai = Provider::OpenAI(
        openai::OpenAIProvider::new(openai_config)
            .await
            .expect("OpenAI provider should initialize"),
    );
    let openai_result = openai
        .edit_image(image_edit_request(), RequestContext::default())
        .await;
    let openai_sink_request = openai_server.await.expect("redirect server should finish");

    let (compatible_base, compatible_server) =
        redirecting_image_edit_server(308, "Permanent Redirect").await;
    let mut compatible_config =
        openai_like::OpenAILikeConfig::with_api_key(compatible_base, "compatible-placeholder");
    compatible_config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    compatible_config.custom_headers.insert(
        "x-proxy-secret".to_string(),
        "compatible-placeholder".to_string(),
    );
    let compatible = Provider::OpenAILike(
        openai_like::OpenAILikeProvider::new_openai_compatible(compatible_config)
            .await
            .expect("OpenAI-compatible provider should initialize"),
    );
    let compatible_result = compatible
        .edit_image(image_edit_request(), RequestContext::default())
        .await;
    let compatible_sink_request = compatible_server
        .await
        .expect("redirect server should finish");

    assert!(
        matches!(
            openai_result,
            Err(ProviderError::ApiError { status: 307, .. })
        ),
        "{openai_result:?}"
    );
    assert!(openai_sink_request.is_none(), "OpenAI secret was replayed");
    assert!(
        matches!(
            compatible_result,
            Err(ProviderError::ApiError { status: 308, .. })
        ),
        "{compatible_result:?}"
    );
    assert!(
        compatible_sink_request.is_none(),
        "OpenAI-compatible secret was replayed"
    );
}
