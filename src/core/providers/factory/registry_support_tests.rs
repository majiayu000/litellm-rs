use crate::core::providers::provider_type::{ProviderType, all_non_custom_provider_types};
use crate::core::providers::{Provider, ProviderError};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::core::types::tools::{FunctionDefinition, Tool, ToolType};
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const MISSING_TEXT_PROVIDERS: [(&str, &str); 3] = [
    ("ai21", "jamba-large"),
    ("huggingface", "openai/gpt-oss-120b:fastest"),
    ("baseten", "deepseek-ai/DeepSeek-V4-Pro"),
];

async fn serve_once(
    status: &str,
    content_type: &str,
    body: &str,
) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("test listener should bind");
    let address = listener
        .local_addr()
        .expect("test listener should have an address");
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("provider should connect");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 2048];
        loop {
            let read = socket
                .read(&mut buffer)
                .await
                .expect("request should be readable");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
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
        socket
            .write_all(response.as_bytes())
            .await
            .expect("response should be writable");
        String::from_utf8(request).expect("request should be UTF-8")
    });
    (format!("http://{address}"), server)
}

async fn catalog_provider(selector: &str, base_url: String) -> Provider {
    crate::core::providers::create_provider(crate::config::models::provider::ProviderConfig {
        name: selector.to_string(),
        provider_type: selector.to_string(),
        api_key: "provider-contract-key".to_string(),
        base_url: Some(base_url),
        endpoint_access: crate::core::net::ProviderEndpointAccess::PrivateNetwork,
        ..Default::default()
    })
    .await
    .unwrap_or_else(|error| panic!("{selector} should create an OpenAI-like provider: {error}"))
}

fn tool_request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        tools: Some(vec![Tool {
            tool_type: ToolType::Function,
            function: FunctionDefinition {
                name: "lookup".to_string(),
                description: None,
                parameters: Some(serde_json::json!({"type": "object"})),
            },
        }]),
        ..Default::default()
    }
}

fn assert_request_contract(request: &str, model: &str, stream: bool, has_tools: bool) {
    let (headers, body) = request
        .split_once("\r\n\r\n")
        .expect("captured request should contain headers and body");
    assert!(headers.starts_with("POST /chat/completions "), "{headers}");
    assert!(
        headers
            .to_ascii_lowercase()
            .contains("authorization: bearer provider-contract-key"),
        "{headers}"
    );
    let body: serde_json::Value = serde_json::from_str(body).expect("request body should be JSON");
    assert_eq!(body["model"], model);
    assert_eq!(body["stream"].as_bool().unwrap_or(false), stream);
    assert_eq!(body.get("tools").is_some(), has_tools);
    if has_tools {
        assert_eq!(body["tools"][0]["function"]["name"], "lookup");
    }
}

#[tokio::test]
async fn missing_text_provider_chat_stream_and_error_contracts() {
    let unary_body = r#"{"id":"chatcmpl-contract","object":"chat.completion","created":1,"model":"upstream","choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call-1","type":"function","function":{"name":"lookup","arguments":"{}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}"#;
    let stream_body = concat!(
        "data: {\"id\":\"chunk-1\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"upstream\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"chunk-2\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"upstream\",\"choices\":[],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}\n\n",
        "data: [DONE]\n\n"
    );
    let error_body =
        r#"{"error":{"type":"rate_limit_error","message":"slow down","retry_after":7}}"#;

    for (selector, model) in MISSING_TEXT_PROVIDERS {
        let (base_url, server) = serve_once("200 OK", "application/json", unary_body).await;
        let response = catalog_provider(selector, base_url)
            .await
            .chat_completion(tool_request(model), RequestContext::default())
            .await
            .unwrap_or_else(|error| panic!("{selector} unary chat should succeed: {error}"));
        assert!(
            response.has_tool_calls(),
            "{selector} should preserve tool calls"
        );
        assert_eq!(
            response
                .usage
                .expect("usage should be preserved")
                .total_tokens,
            5
        );
        assert_request_contract(
            &server.await.expect("server should finish"),
            model,
            false,
            true,
        );

        let (base_url, server) = serve_once("200 OK", "text/event-stream", stream_body).await;
        let provider = catalog_provider(selector, base_url).await;
        let mut stream = provider
            .chat_completion_stream(ChatRequest::new(model), RequestContext::default())
            .await
            .unwrap_or_else(|error| panic!("{selector} stream should start: {error}"));
        let mut chunks = Vec::new();
        while let Some(chunk) = stream.next().await {
            chunks.push(chunk.unwrap_or_else(|error| panic!("{selector} stream chunk: {error}")));
        }
        assert_eq!(chunks[0].choices[0].delta.content.as_deref(), Some("ok"));
        assert_eq!(
            chunks[1]
                .usage
                .as_ref()
                .expect("stream usage should be preserved")
                .total_tokens,
            5
        );
        assert_request_contract(
            &server.await.expect("server should finish"),
            model,
            true,
            false,
        );

        let (base_url, server) =
            serve_once("429 Too Many Requests", "application/json", error_body).await;
        let error = catalog_provider(selector, base_url)
            .await
            .chat_completion(tool_request(model), RequestContext::default())
            .await
            .expect_err("rate-limit envelope should fail clearly");
        assert!(matches!(
            error,
            ProviderError::RateLimit {
                retry_after: Some(7),
                ..
            }
        ));
        assert_request_contract(
            &server.await.expect("server should finish"),
            model,
            false,
            true,
        );
    }
}

#[tokio::test]
async fn catalog_text_provider_wire_ids_remain_lossless() {
    let cases = [
        ("meta_llama", "Llama-4-Maverick-17B-128E-Instruct-FP8"),
        ("perplexity", "sonar"),
        ("together", "meta-llama/Llama-3.3-70B-Instruct-Turbo"),
        (
            "fireworks",
            "accounts/fireworks/models/llama-v3p3-70b-instruct",
        ),
        ("groq", "llama-3.3-70b-versatile"),
        ("cerebras", "llama3.1-8b"),
        ("sambanova", "Meta-Llama-3.1-8B-Instruct"),
        ("deepinfra", "meta-llama/Meta-Llama-3.1-70B-Instruct"),
    ];

    for (selector, model) in cases {
        let provider = crate::core::providers::create_provider(
            crate::config::models::provider::ProviderConfig {
                name: selector.to_string(),
                provider_type: selector.to_string(),
                api_key: "provider-contract-key".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap_or_else(|error| panic!("{selector} should remain creatable: {error}"));
        let Provider::OpenAILike(provider) = provider else {
            panic!("{selector} should remain catalog-backed");
        };
        let transformed = LLMProvider::transform_request(
            &provider,
            ChatRequest::new(model),
            RequestContext::default(),
        )
        .await
        .unwrap_or_else(|error| panic!("{selector} transform should succeed: {error}"));
        assert_eq!(
            transformed["model"], model,
            "{selector} changed its wire ID"
        );
    }
}

#[tokio::test]
async fn missing_text_provider_wrong_selectors_fail_closed() {
    for selector in ["ai-21", "huggingface_inference", "base-ten", "unknown"] {
        let error = crate::core::providers::create_provider(
            crate::config::models::provider::ProviderConfig {
                name: selector.to_string(),
                provider_type: selector.to_string(),
                api_key: "provider-contract-key".to_string(),
                ..Default::default()
            },
        )
        .await
        .expect_err("unknown or wrong provider selector must fail closed");
        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        assert!(error.to_string().contains(selector));
    }
}

#[tokio::test]
async fn supported_variants_do_not_fallthrough_to_not_implemented() {
    for provider_type in Provider::factory_supported_provider_types() {
        let result =
            Provider::from_config_async(provider_type.clone(), serde_json::json!({})).await;
        // Success is fine (e.g. local catalog providers with skip_api_key);
        // a real config error is also fine. Only NotImplemented is wrong.
        if let Err(error) = result {
            assert!(
                !matches!(error, ProviderError::NotImplemented { .. }),
                "{provider_type:?} unexpectedly fell through to NotImplemented: {error}"
            );
        }
    }
}

#[tokio::test]
async fn unsupported_variants_return_not_implemented() {
    let supported = Provider::factory_supported_provider_types();

    for provider_type in all_non_custom_provider_types() {
        if supported.contains(&provider_type) {
            continue;
        }

        let error = Provider::from_config_async(provider_type.clone(), serde_json::json!({}))
            .await
            .expect_err("Expected unsupported provider to fail");
        assert!(
            matches!(error, ProviderError::NotImplemented { .. }),
            "Expected NotImplemented for {provider_type:?}, got {error}"
        );
        assert_eq!(
            error.provider(),
            provider_type.to_string(),
            "NotImplemented provider name should identify the requested provider"
        );
    }
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn ollama_factory_creates_policy_wired_native_provider() {
    let provider = Provider::from_config_async(
        ProviderType::Ollama,
        serde_json::json!({
            "base_url": "http://127.0.0.1:11434",
            "endpoint_access": "private_network",
            "models": ["llama3:8b"]
        }),
    )
    .await
    .unwrap_or_else(|error| panic!("ollama should create a native provider: {error}"));

    assert!(matches!(provider, Provider::Ollama(_)));
    assert_eq!(provider.name(), "ollama");
    assert_eq!(provider.provider_type(), ProviderType::Ollama);
    let capabilities = provider.capabilities();
    assert!(capabilities.contains(&crate::core::types::model::ProviderCapability::ChatCompletion));
    assert!(
        capabilities.contains(&crate::core::types::model::ProviderCapability::ChatCompletionStream)
    );
    assert!(capabilities.contains(&crate::core::types::model::ProviderCapability::Embeddings));
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn ollama_factory_discovers_models_and_normalizes_trailing_slash() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("test listener binds");
    let address = listener.local_addr().expect("test listener has address");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("factory requests tags");
        let mut request = [0_u8; 4096];
        let read = socket
            .read(&mut request)
            .await
            .expect("request is readable");
        let body = r#"{"models":[{"name":"llama3:8b"}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("tags response is writable");
        String::from_utf8_lossy(&request[..read]).into_owned()
    });

    let provider = Provider::from_config_async(
        ProviderType::Ollama,
        serde_json::json!({
            "api_base": format!("http://{address}/"),
            "endpoint_access": "private_network"
        }),
    )
    .await
    .expect("Ollama factory should discover models");

    assert_eq!(provider.list_models()[0].id, "llama3:8b");
    let request = server.await.expect("test server completes");
    assert!(request.starts_with("GET /api/tags "), "{request}");
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn ollama_factory_rejects_explicit_public_loopback() {
    let error = Provider::from_config_async(
        ProviderType::Ollama,
        serde_json::json!({
            "api_base": "http://127.0.0.1:11434",
            "endpoint_access": "public_only"
        }),
    )
    .await
    .expect_err("public-only Ollama must reject an explicit loopback endpoint");

    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(error.to_string().contains("private or reserved"));
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn ollama_factory_rejects_conflicting_endpoint_aliases() {
    let error = Provider::from_config_async(
        ProviderType::Ollama,
        serde_json::json!({
            "base_url": "https://example.com",
            "api_base": "http://127.0.0.1:11434",
            "endpoint_access": "public_only"
        }),
    )
    .await
    .expect_err("conflicting Ollama endpoint aliases must fail closed");

    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(error.to_string().contains("different endpoints"));
}

#[cfg(not(feature = "providers-extended"))]
#[tokio::test]
async fn ollama_factory_requires_providers_extended() {
    let error = Provider::from_config_async(ProviderType::Ollama, serde_json::json!({}))
        .await
        .expect_err("ollama should require providers-extended");

    assert!(matches!(error, ProviderError::NotImplemented { .. }));
    assert_eq!(error.provider(), "ollama");
    assert!(error.to_string().contains("providers-extended"));
}
