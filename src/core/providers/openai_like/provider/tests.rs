#![allow(deprecated)]

use super::*;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatMessage;
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::{EmbeddingInput, EmbeddingRequest};
use crate::core::types::health::HealthStatus;
use crate::core::types::message::{MessageContent, MessageRole};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const TEST_PUBLIC_API_BASE: &str = "https://api.example.com/v1";

#[test]
fn gemini_transport_preserves_typed_policy_configuration_errors() {
    let error = gemini_openai_like_transport_error(ProviderError::configuration(
        "openai_like",
        "Provider endpoint rejected by SSRF protection",
    ));

    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(error.to_string().contains("SSRF protection"));
}

#[test]
fn gemini_transport_keeps_redirect_loops_retryable() {
    let error = gemini_openai_like_transport_error(ProviderError::network(
        "openai_like",
        "error following redirect for url (https://example.test/loop)",
    ));

    assert!(matches!(error, ProviderError::Network { .. }));
}

#[test]
fn gemini_transport_redacts_network_error_details() {
    let error = gemini_openai_like_transport_error(ProviderError::network(
        "openai_like",
        "https://example.test?key=secret-key",
    ));

    assert!(matches!(error, ProviderError::Network { .. }));
    assert!(!error.to_string().contains("secret-key"));
}

fn endpoint_policy_pool() -> GlobalPoolManager {
    GlobalPoolManager::new_for_provider(
        "openai_like",
        crate::core::providers::base::BaseConfig {
            api_base: Some("https://api.example.com/v1".to_string()),
            ..Default::default()
        },
    )
    .expect("policy pool should build")
}

#[tokio::test]
async fn gemini_unary_pool_preserves_direct_policy_rejection() {
    let error = endpoint_policy_pool()
        .execute_request_preserving_endpoint_policy(
            "ftp://api.example.com/secret-key",
            HttpMethod::POST,
            Vec::new(),
            None,
        )
        .await
        .expect_err("unsupported scheme must fail");

    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(!error.to_string().contains("secret-key"));
}

#[tokio::test]
async fn gemini_stream_pool_preserves_direct_policy_rejection() {
    let error = endpoint_policy_pool()
        .execute_streaming_request_preserving_endpoint_policy(
            "ftp://api.example.com/secret-key",
            Vec::new(),
            serde_json::json!({}),
            "gemini_proxy",
        )
        .await
        .expect_err("unsupported scheme must fail");

    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(!error.to_string().contains("secret-key"));
}

fn private_openai_like_config(api_base: impl Into<String>) -> OpenAILikeConfig {
    let mut config = crate::core::providers::openai_like::config::test_openai_like_config(api_base);
    config.base.endpoint_access = crate::core::net::ProviderEndpointAccess::PrivateNetwork;
    config
}

#[derive(Clone)]
struct CapturedHttpRequest {
    path: String,
    body: Vec<u8>,
}

impl CapturedHttpRequest {
    fn json_body(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("captured request body should be JSON")
    }
}

async fn read_full_http_request(socket: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut request_bytes = Vec::new();
    let mut buffer = [0_u8; 1024];

    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(request_bytes);
        }

        request_bytes.extend_from_slice(&buffer[..bytes_read]);
        if let Some(header_end) = request_bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&request_bytes[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);

            if request_bytes.len() >= header_end + 4 + content_length {
                return Ok(request_bytes);
            }
        }
    }
}

fn parse_http_path_and_body(request_bytes: &[u8]) -> CapturedHttpRequest {
    let header_end = request_bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or(request_bytes.len());
    let headers = String::from_utf8_lossy(&request_bytes[..header_end]);
    let path = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_string();
    let body = request_bytes
        .get(header_end.saturating_add(4)..)
        .unwrap_or(&[])
        .to_vec();
    CapturedHttpRequest { path, body }
}

async fn openai_like_stream_response_url(
    status: &str,
    body: &str,
    complete: bool,
) -> std::io::Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len() + usize::from(!complete)
    );

    tokio::spawn(async move {
        let (mut socket, _) = match listener.accept().await {
            Ok(connection) => connection,
            Err(err) => panic!("test server failed to accept request: {err}"),
        };
        if let Err(err) = read_full_http_request(&mut socket).await {
            panic!("test server failed to read request: {err}");
        }
        if let Err(err) = socket.write_all(response.as_bytes()).await {
            panic!("test server failed to write response: {err}");
        }
    });

    Ok(format!("http://{addr}"))
}

async fn openai_like_json_response_url(
    status: &str,
    body: &str,
) -> std::io::Result<(String, Arc<Mutex<Option<CapturedHttpRequest>>>)> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let captured = Arc::new(Mutex::new(None));
    let captured_for_server = Arc::clone(&captured);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );

    tokio::spawn(async move {
        let (mut socket, _) = match listener.accept().await {
            Ok(connection) => connection,
            Err(err) => panic!("test server failed to accept request: {err}"),
        };
        let request_bytes = match read_full_http_request(&mut socket).await {
            Ok(bytes) => bytes,
            Err(err) => panic!("test server failed to read request: {err}"),
        };
        *captured_for_server.lock().expect("captured request mutex") =
            Some(parse_http_path_and_body(&request_bytes));
        if let Err(err) = socket.write_all(response.as_bytes()).await {
            panic!("test server failed to write response: {err}");
        }
    });

    Ok((format!("http://{addr}"), captured))
}

fn embedding_request(model: &str, input: EmbeddingInput) -> EmbeddingRequest {
    EmbeddingRequest {
        model: model.to_string(),
        input,
        user: None,
        encoding_format: None,
        dimensions: None,
        task_type: None,
        truncation: None,
    }
}

const EMBEDDING_SUCCESS_BODY: &str = r#"{
    "object": "list",
    "data": [
        {
            "object": "embedding",
            "index": 0,
            "embedding": [0.1, 0.2]
        }
    ],
    "model": "text-embedding-3-small",
    "usage": {
        "prompt_tokens": 1,
        "completion_tokens": 0,
        "total_tokens": 1
    }
}"#;

const EMBEDDING_MULTI_INPUT_BODY: &str = r#"{
    "object": "list",
    "data": [
        {
            "object": "embedding",
            "index": 0,
            "embedding": [0.1]
        },
        {
            "object": "embedding",
            "index": 1,
            "embedding": [0.2, 0.3]
        }
    ],
    "model": "text-embedding-3-small",
    "usage": {
        "prompt_tokens": 2,
        "completion_tokens": 0,
        "total_tokens": 2
    }
}"#;

async fn openai_compatible_embeddings_provider(api_base: impl Into<String>) -> OpenAILikeProvider {
    OpenAILikeProvider::new_openai_compatible(private_openai_like_config(api_base))
        .await
        .expect("openai-compatible provider should build")
}

fn openai_like_chat_stream_request() -> ChatRequest {
    ChatRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[tokio::test]
async fn test_provider_creation_with_api_base() {
    let provider = OpenAILikeProvider::with_api_base(TEST_PUBLIC_API_BASE).await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(provider.name(), "openai_like");
}

#[tokio::test]
async fn provider_rejects_private_access_to_official_openai_endpoint() {
    OpenAILikeProvider::with_api_base("https://api.openai.com/v1")
        .await
        .expect("official OpenAI endpoints must remain valid with public-only access");

    let config = private_openai_like_config("https://api.openai.com/v1");

    let error = OpenAILikeProvider::new(config)
        .await
        .expect_err("official OpenAI endpoints must remain public-only");
    assert!(
        error
            .to_string()
            .contains("private_network access cannot target the official OpenAI endpoint")
    );
}

#[tokio::test]
async fn generic_openai_like_declares_only_catalog_executable_capabilities() {
    let provider = OpenAILikeProvider::with_api_base(TEST_PUBLIC_API_BASE)
        .await
        .expect("OpenAI-like provider should build");

    assert_eq!(
        provider.capabilities(),
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ToolCalling,
            ProviderCapability::FunctionCalling,
        ]
    );
    assert!(
        !provider
            .capabilities()
            .contains(&ProviderCapability::Embeddings)
    );
    assert!(!OPENAI_LIKE_CATALOG_CAPABILITIES.contains(&ProviderCapability::Embeddings));
}

#[tokio::test]
async fn openai_compatible_declares_executable_proxy_capabilities() {
    let config = OpenAILikeConfig::new(TEST_PUBLIC_API_BASE).with_skip_api_key(true);
    let provider = OpenAILikeProvider::new_openai_compatible(config)
        .await
        .expect("OpenAI-compatible provider should build");

    assert_eq!(
        provider.capabilities(),
        OPENAI_COMPATIBLE_PROXY_CAPABILITIES
    );
    assert!(OPENAI_COMPATIBLE_PROXY_CAPABILITIES.contains(&ProviderCapability::Embeddings));
    assert!(!OPENAI_LIKE_CATALOG_CAPABILITIES.contains(&ProviderCapability::Embeddings));
}

#[tokio::test]
async fn openai_like_catalog_rejects_invalid_capability_profiles() {
    static EMPTY: &[ProviderCapability] = &[];
    static DUPLICATE: &[ProviderCapability] = &[
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletion,
    ];
    static UNIMPLEMENTED: &[ProviderCapability] = &[ProviderCapability::ImageEdit];
    static UNIMPLEMENTED_EMBEDDINGS: &[ProviderCapability] = &[ProviderCapability::Embeddings];

    let config = OpenAILikeConfig::new(TEST_PUBLIC_API_BASE).with_skip_api_key(true);
    for (profile, expected) in [
        (EMPTY, "cannot be empty"),
        (DUPLICATE, "duplicate ChatCompletion"),
        (UNIMPLEMENTED, "not executable for this OpenAI-like profile"),
        (
            UNIMPLEMENTED_EMBEDDINGS,
            "not executable for this OpenAI-like profile",
        ),
    ] {
        let error = OpenAILikeProvider::new_for_catalog(config.clone(), profile)
            .await
            .expect_err("invalid capability profile must fail closed");
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in {error}"
        );
    }
}

#[tokio::test]
async fn test_openai_like_streaming_maps_non_success_status_before_sse()
-> Result<(), Box<dyn std::error::Error>> {
    let body = r#"{"error":{"type":"rate_limit_error","message":"slow down","retry_after":5}}"#;
    let api_base = openai_like_stream_response_url("429 Too Many Requests", body, true).await?;
    let config = private_openai_like_config(api_base);
    let provider = OpenAILikeProvider::new(config).await?;

    let err = match LLMProvider::chat_completion_stream(
        &provider,
        openai_like_chat_stream_request(),
        RequestContext::default(),
    )
    .await
    {
        Ok(_) => panic!("streaming response should map upstream status to provider error"),
        Err(err) => err,
    };

    match err {
        ProviderError::RateLimit {
            provider,
            retry_after,
            ..
        } => {
            assert_eq!(provider, PROVIDER_NAME);
            assert_eq!(retry_after, Some(5));
        }
        other => panic!("expected OpenAI-like rate limit error, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn openai_like_policy_pool_rejects_cross_authority_without_connect()
-> Result<(), Box<dyn std::error::Error>> {
    let api_base = openai_like_stream_response_url("200 OK", "{}", true).await?;
    let health_provider = OpenAILikeProvider::new(private_openai_like_config(api_base)).await?;
    assert_eq!(
        LLMProvider::health_check(&health_provider).await,
        HealthStatus::Healthy
    );
    let target = TcpListener::bind(("127.0.0.1", 0)).await?;
    let config = private_openai_like_config("http://127.0.0.1:1");
    let provider = OpenAILikeProvider::new(config).await?;
    let target_url = format!("http://{}/models", target.local_addr()?);

    let error = provider
        .pool_manager
        .execute_request(&target_url, HttpMethod::GET, Vec::new(), None)
        .await
        .expect_err("cross-authority request must fail closed");
    assert!(error.to_string().contains("authority"));
    let accepted =
        tokio::time::timeout(std::time::Duration::from_millis(100), target.accept()).await;
    assert!(accepted.is_err());
    let config = private_openai_like_config("http://169.254.169.254/v1");
    let error = OpenAILikeProvider::new(config)
        .await
        .expect_err("metadata endpoints must remain forbidden");
    assert!(error.to_string().contains("private or reserved"));
    Ok(())
}

#[tokio::test]
async fn ordinary_error_body_failure_preserves_http_status()
-> Result<(), Box<dyn std::error::Error>> {
    let api_base = openai_like_stream_response_url("401 Unauthorized", "{}", false).await?;
    let config = private_openai_like_config(api_base);
    let provider = OpenAILikeProvider::new(config).await?;

    let error = provider
        .execute_chat_completion(openai_like_chat_stream_request())
        .await
        .expect_err("truncated 401 body must remain an authentication error");
    assert!(matches!(error, OpenAILikeError::Authentication { .. }));
    Ok(())
}

#[tokio::test]
async fn test_provider_creation_with_api_key() {
    let provider = OpenAILikeProvider::with_api_key(TEST_PUBLIC_API_BASE, "sk-test123").await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_supports_any_model() {
    let provider = OpenAILikeProvider::with_api_base(TEST_PUBLIC_API_BASE)
        .await
        .unwrap_or_else(|err| panic!("OpenAI-like test provider should initialize: {err}"));

    assert!(provider.supports_model("gpt-4"));
    assert!(provider.supports_model("llama-2-70b"));
    assert!(provider.supports_model("any-custom-model"));
    assert!(provider.supports_model("custom/my-model"));
}

#[tokio::test]
async fn test_model_info_for_any_model() {
    let provider = OpenAILikeProvider::with_api_base(TEST_PUBLIC_API_BASE)
        .await
        .unwrap_or_else(|err| panic!("OpenAI-like test provider should initialize: {err}"));

    let info = provider.get_model_info("my-custom-model");
    assert_eq!(info.id, "my-custom-model");
    assert_eq!(info.provider, "openai_like");
    assert!(info.supports_streaming);
}

#[tokio::test]
async fn test_request_transformation() {
    let provider = OpenAILikeProvider::with_api_base(TEST_PUBLIC_API_BASE)
        .await
        .unwrap();

    let request = ChatRequest {
        model: "test-model".to_string(),
        messages: vec![],
        temperature: Some(0.7),
        max_tokens: Some(100),
        reasoning_effort: Some("high".to_string()),
        ..Default::default()
    };

    let transformed = provider.transform_chat_request(request);
    assert!(transformed.is_ok());

    let json = transformed.unwrap();
    assert_eq!(json["model"], "test-model");
    assert!((json["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
    assert_eq!(json["max_tokens"], 100);
    assert_eq!(json["reasoning_effort"], "high");
}

#[tokio::test]
async fn test_supported_params_advertise_forwarded_chat_fields() {
    let provider = OpenAILikeProvider::with_api_base(TEST_PUBLIC_API_BASE)
        .await
        .unwrap_or_else(|err| panic!("OpenAI-like test provider should initialize: {err}"));
    let params = LLMProvider::get_supported_openai_params(&provider, "test-model");

    for forwarded_param in ["store", "metadata", "service_tier"] {
        assert!(
            params.contains(&forwarded_param),
            "supported params should advertise forwarded field {forwarded_param}"
        );
    }
}

#[tokio::test]
async fn test_xai_grok_43_reasoning_effort_is_top_level() {
    let config = OpenAILikeConfig::new("https://api.x.ai/v1")
        .with_provider_name("xai")
        .with_skip_api_key(true);
    let provider = OpenAILikeProvider::new(config).await.unwrap();

    let request = ChatRequest {
        model: "grok-4.3".to_string(),
        messages: vec![],
        reasoning_effort: Some("high".to_string()),
        ..Default::default()
    };

    let json = provider.transform_chat_request(request).unwrap();
    assert_eq!(json["reasoning_effort"], "high");
    assert!(json.get("reasoning").is_none());
}

#[tokio::test]
async fn test_xai_multi_agent_reasoning_effort_is_nested() {
    let config = OpenAILikeConfig::new("https://api.x.ai/v1")
        .with_provider_name("xai")
        .with_skip_api_key(true);
    let provider = OpenAILikeProvider::new(config).await.unwrap();

    let request = ChatRequest {
        model: "grok-4.20-multi-agent-0309".to_string(),
        messages: vec![],
        reasoning_effort: Some("xhigh".to_string()),
        ..Default::default()
    };

    let json = provider.transform_chat_request(request).unwrap();
    assert_eq!(json["reasoning"]["effort"], "xhigh");
    assert!(json.get("reasoning_effort").is_none());
}

#[tokio::test]
async fn test_xai_grok_420_rejects_reasoning_effort() {
    let config = OpenAILikeConfig::new("https://api.x.ai/v1")
        .with_provider_name("xai")
        .with_skip_api_key(true);
    let provider = OpenAILikeProvider::new(config).await.unwrap();

    let request = ChatRequest {
        model: "grok-4.20".to_string(),
        messages: vec![],
        reasoning_effort: Some("high".to_string()),
        ..Default::default()
    };

    let err = provider.transform_chat_request(request).unwrap_err();
    assert!(
        err.to_string()
            .contains("does not support reasoning_effort")
    );
}

#[tokio::test]
async fn test_xai_high_context_uses_registered_pricing() {
    let config = OpenAILikeConfig::new("https://api.x.ai/v1")
        .with_provider_name("xai")
        .with_skip_api_key(true);
    let provider = OpenAILikeProvider::new(config).await.unwrap();

    let cost = LLMProvider::calculate_cost(&provider, "grok-4.3", 250_000, 1_000)
        .await
        .unwrap();

    assert!((cost - 0.315).abs() < 1e-12);
    let current_alias = LLMProvider::calculate_cost(&provider, "grok-build-latest", 1_000, 1_000)
        .await
        .unwrap();
    assert!((current_alias - 0.008).abs() < 1e-12);
}

#[tokio::test]
async fn test_non_xai_provider_does_not_use_xai_pricing() {
    let config = OpenAILikeConfig::new("https://api.groq.com/openai/v1")
        .with_provider_name("groq")
        .with_skip_api_key(true);
    let Ok(provider) = OpenAILikeProvider::new(config).await else {
        panic!("OpenAI-like test provider should initialize");
    };

    let cost = LLMProvider::calculate_cost(&provider, "grok-4.3", 250_000, 1_000).await;

    assert!(matches!(cost, Ok(value) if value == 0.0));
}

#[tokio::test]
async fn test_xai_reasoning_effort_rejects_incompatible_params() {
    let config = OpenAILikeConfig::new("https://api.x.ai/v1")
        .with_provider_name("xai")
        .with_skip_api_key(true);
    let Ok(provider) = OpenAILikeProvider::new(config).await else {
        panic!("xAI test provider should initialize");
    };

    let request = ChatRequest {
        model: "grok-4.3".to_string(),
        messages: vec![],
        reasoning_effort: Some("high".to_string()),
        stop: Some(vec!["done".to_string()]),
        presence_penalty: Some(0.1),
        ..Default::default()
    };

    let Err(err) = provider.transform_chat_request(request) else {
        panic!("xAI reasoning with incompatible params should fail");
    };

    let message = err.to_string();
    assert!(message.contains("reasoning_effort is incompatible"));
    assert!(message.contains("stop"));
    assert!(message.contains("presence_penalty"));
}

#[tokio::test]
async fn test_model_prefix_stripping() {
    let config = OpenAILikeConfig::new(TEST_PUBLIC_API_BASE)
        .with_model_prefix("custom/")
        .with_skip_api_key(true);

    let provider = OpenAILikeProvider::new(config).await.unwrap();

    let request = ChatRequest {
        model: "custom/gpt-4".to_string(),
        messages: vec![],
        ..Default::default()
    };

    let transformed = provider.transform_chat_request(request).unwrap();
    assert_eq!(transformed["model"], "gpt-4");
}

#[test]
fn test_error_mapping() {
    let provider_name = PROVIDER_NAME;

    let err = OpenAILikeError::openai_like_authentication("Invalid API key");
    assert_eq!(err.provider(), provider_name);

    let err = OpenAILikeError::openai_like_rate_limit(Some(60));
    assert!(err.is_retryable());
    assert_eq!(err.retry_delay(), Some(60));
}

#[tokio::test]
async fn test_non_json_upstream_error_body_is_not_forwarded() {
    let provider = OpenAILikeProvider::with_api_base(TEST_PUBLIC_API_BASE)
        .await
        .unwrap();
    let err = provider.map_error_response(
        418,
        "trace_id=abc secret=sk-ant-api03-abcdefghijklmnopqrstuvwxyz",
    );

    match err {
        ProviderError::ApiError {
            status, message, ..
        } => {
            assert_eq!(status, 418);
            assert_eq!(
                message,
                "Upstream OpenAI-compatible provider returned HTTP 418"
            );
            assert!(!message.contains("sk-ant"));
        }
        other => panic!("expected api error, got {other:?}"),
    }
}

#[test]
fn test_error_mapper_non_json_body_is_not_forwarded() {
    let mapper = OpenAILikeErrorMapper;
    let err: ProviderError = mapper.map_http_error(
        502,
        "upstream panic included Authorization: Bearer eyJsecret.token.value",
    );

    match err {
        ProviderError::Network { message, .. } => {
            assert_eq!(
                message,
                "Upstream OpenAI-compatible provider returned HTTP 502"
            );
            assert!(!message.contains("Bearer"));
        }
        other => panic!("expected network error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_name_returns_default_for_default_config() {
    let provider = OpenAILikeProvider::with_api_base(TEST_PUBLIC_API_BASE)
        .await
        .unwrap();
    assert_eq!(provider.name(), "openai_like");
}

#[tokio::test]
async fn test_name_returns_actual_provider_name() {
    let config = OpenAILikeConfig::new("https://api.groq.com/openai/v1")
        .with_provider_name("groq")
        .with_skip_api_key(true);
    let provider = OpenAILikeProvider::new(config).await.unwrap();
    assert_eq!(provider.name(), "groq");
}

#[tokio::test]
async fn test_name_returns_deepseek_name() {
    let config = OpenAILikeConfig::new("https://api.deepseek.com/v1")
        .with_provider_name("deepseek")
        .with_skip_api_key(true);
    let provider = OpenAILikeProvider::new(config).await.unwrap();
    assert_eq!(provider.name(), "deepseek");
}

/// OR_SITE_URL → HTTP-Referer and OR_APP_NAME → X-Title are injected via
/// env vars at request time in get_request_headers(). Integration tests cover
/// the live path; here we verify the branch is only active for "openrouter".
#[tokio::test]
async fn test_non_openrouter_provider_no_or_headers() {
    // When provider_name is NOT "openrouter", OR_* env vars must be ignored
    // even if they happen to be set in the environment.
    let config = OpenAILikeConfig::new("https://api.openai.com/v1")
        .with_provider_name("openai")
        .with_skip_api_key(true);
    let Ok(provider) = OpenAILikeProvider::new(config).await else {
        panic!("provider creation must succeed");
    };
    let headers = provider.get_request_headers();

    let has_referer = headers.iter().any(|h| h.0 == "HTTP-Referer");
    let has_title = headers.iter().any(|h| h.0 == "X-Title");

    assert!(
        !has_referer,
        "HTTP-Referer must not be set for non-openrouter providers"
    );
    assert!(
        !has_title,
        "X-Title must not be set for non-openrouter providers"
    );
}

#[tokio::test]
async fn test_meta_llama_uses_native_organization_header() {
    let mut config = OpenAILikeConfig::new("https://api.llama.com/compat/v1")
        .with_provider_name("meta_llama")
        .with_skip_api_key(true);
    config.base.organization = Some("org-123".to_string());
    config
        .base
        .headers
        .insert("x-organization-id".to_string(), "org-base".to_string());
    config
        .custom_headers
        .insert("X-ORGANIZATION-ID".to_string(), "org-custom".to_string());
    let provider = OpenAILikeProvider::new(config).await.unwrap();

    let headers = provider.get_request_headers();
    let organization_headers = headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("X-Organization-ID"))
        .collect::<Vec<_>>();
    assert_eq!(organization_headers.len(), 1);
    assert_eq!(organization_headers[0].1, "org-custom");
    assert!(
        headers
            .iter()
            .all(|(name, _)| name != "OpenAI-Organization")
    );
}

#[tokio::test]
async fn test_meta_llama_deduplicates_configured_organization_headers() {
    let mut config = OpenAILikeConfig::new("https://api.llama.com/compat/v1")
        .with_provider_name("meta_llama")
        .with_skip_api_key(true);
    config
        .base
        .headers
        .insert("x-organization-id".to_string(), "org-base".to_string());
    config
        .custom_headers
        .insert("X-ORGANIZATION-ID".to_string(), "org-custom".to_string());
    let provider = OpenAILikeProvider::new(config).await.unwrap();

    let organization_headers = provider
        .get_request_headers()
        .into_iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("X-Organization-ID"))
        .collect::<Vec<_>>();
    assert_eq!(organization_headers.len(), 1);
    assert_eq!(organization_headers[0].1, "org-custom");
}

#[tokio::test]
async fn test_openrouter_thinking_wired_in_transform() {
    use crate::core::types::thinking::{ThinkingConfig, ThinkingEffort};

    let config = OpenAILikeConfig::new("https://openrouter.ai/api/v1")
        .with_provider_name("openrouter")
        .with_skip_api_key(true);
    let Ok(provider) = OpenAILikeProvider::new(config).await else {
        panic!("provider creation must succeed");
    };

    let request = ChatRequest {
        model: "unknown-model".to_string(),
        messages: vec![],
        thinking: Some(
            ThinkingConfig::new()
                .enabled()
                .with_effort(ThinkingEffort::High)
                .with_budget(5000),
        ),
        ..Default::default()
    };

    let Ok(json) = provider.transform_chat_request(request) else {
        panic!("transform_chat_request must succeed");
    };

    assert_eq!(
        json.get("reasoning")
            .and_then(|r| r.get("effort"))
            .and_then(|v| v.as_str()),
        Some("high"),
        "reasoning.effort must be forwarded"
    );
}

#[tokio::test]
async fn current_xai_reasoning_normalizes_after_extra_merge() {
    let config = OpenAILikeConfig::new("https://api.x.ai/v1")
        .with_provider_name("xai")
        .with_skip_api_key(true);
    let provider = OpenAILikeProvider::new(config).await.unwrap();
    for (model, effort, expected) in [
        ("grok-4.5", "xhigh", "high"),
        ("grok-4.6", "xhigh", "xhigh"),
    ] {
        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![],
            reasoning_effort: Some(effort.to_string()),
            extra_params: std::collections::HashMap::from([(
                "reasoning_effort".to_string(),
                serde_json::json!(effort),
            )]),
            ..Default::default()
        };
        let json = provider.transform_chat_request(request).unwrap();
        assert_eq!(json["reasoning_effort"], expected);
    }

    for extra in [serde_json::json!("low"), serde_json::json!(7)] {
        let request = ChatRequest {
            model: "grok-4.6".to_string(),
            messages: vec![],
            reasoning_effort: Some("high".to_string()),
            extra_params: std::collections::HashMap::from([(
                "reasoning_effort".to_string(),
                extra,
            )]),
            ..Default::default()
        };
        assert!(provider.transform_chat_request(request).is_err());
    }

    let request = ChatRequest {
        model: "grok-4.6".to_string(),
        messages: vec![],
        extra_params: std::collections::HashMap::from([
            ("reasoning_effort".to_string(), serde_json::json!("high")),
            ("stop".to_string(), serde_json::json!(["done"])),
        ]),
        ..Default::default()
    };
    assert!(provider.transform_chat_request(request).is_err());
}

#[tokio::test]
async fn openai_compatible_embeddings_forwards_success_response()
-> Result<(), Box<dyn std::error::Error>> {
    let (api_base, captured) =
        openai_like_json_response_url("200 OK", EMBEDDING_SUCCESS_BODY).await?;
    let provider = openai_compatible_embeddings_provider(api_base).await;

    let response = LLMProvider::embeddings(
        &provider,
        embedding_request(
            "text-embedding-3-small",
            EmbeddingInput::Text("hello".to_string()),
        ),
        RequestContext::default(),
    )
    .await?;

    assert_eq!(response.object, "list");
    assert_eq!(response.model, "text-embedding-3-small");
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.data[0].index, 0);
    assert_eq!(response.data[0].embedding, vec![0.1, 0.2]);

    let captured = captured
        .lock()
        .expect("captured request mutex")
        .clone()
        .expect("embeddings request should reach upstream");
    assert_eq!(captured.path, "/embeddings");
    let body = captured.json_body();
    assert_eq!(body["model"], "text-embedding-3-small");
    assert_eq!(body["input"], "hello");
    Ok(())
}

#[tokio::test]
async fn openai_compatible_embeddings_preserve_multi_input_indexes()
-> Result<(), Box<dyn std::error::Error>> {
    let (api_base, captured) =
        openai_like_json_response_url("200 OK", EMBEDDING_MULTI_INPUT_BODY).await?;
    let provider = openai_compatible_embeddings_provider(api_base).await;

    let response = LLMProvider::embeddings(
        &provider,
        embedding_request(
            "text-embedding-3-small",
            EmbeddingInput::Array(vec!["hello".to_string(), "world".to_string()]),
        ),
        RequestContext::default(),
    )
    .await?;

    assert_eq!(response.data.len(), 2);
    assert_eq!(response.data[0].index, 0);
    assert_eq!(response.data[0].embedding, vec![0.1]);
    assert_eq!(response.data[1].index, 1);
    assert_eq!(response.data[1].embedding, vec![0.2, 0.3]);

    let captured = captured
        .lock()
        .expect("captured request mutex")
        .clone()
        .expect("embeddings request should reach upstream");
    let body = captured.json_body();
    assert_eq!(body["input"], serde_json::json!(["hello", "world"]));
    Ok(())
}

#[tokio::test]
async fn openai_compatible_embeddings_rewrites_prefixed_model()
-> Result<(), Box<dyn std::error::Error>> {
    let (api_base, captured) =
        openai_like_json_response_url("200 OK", EMBEDDING_SUCCESS_BODY).await?;
    let mut config = private_openai_like_config(api_base);
    config.model_prefix = Some("custom/".to_string());
    let provider = OpenAILikeProvider::new_openai_compatible(config).await?;

    LLMProvider::embeddings(
        &provider,
        embedding_request(
            "custom/text-embedding-3-small",
            EmbeddingInput::Text("hello".to_string()),
        ),
        RequestContext::default(),
    )
    .await?;

    let captured = captured
        .lock()
        .expect("captured request mutex")
        .clone()
        .expect("embeddings request should reach upstream");
    let body = captured.json_body();
    assert_eq!(body["model"], "text-embedding-3-small");
    assert_ne!(body["model"], "custom/text-embedding-3-small");
    Ok(())
}

#[tokio::test]
async fn openai_compatible_embeddings_maps_401_before_deserialize()
-> Result<(), Box<dyn std::error::Error>> {
    let body = r#"{"error":{"type":"authentication_error","message":"invalid api key"}}"#;
    let (api_base, _) = openai_like_json_response_url("401 Unauthorized", body).await?;
    let provider = openai_compatible_embeddings_provider(api_base).await;

    let err = LLMProvider::embeddings(
        &provider,
        embedding_request(
            "text-embedding-3-small",
            EmbeddingInput::Text("hello".to_string()),
        ),
        RequestContext::default(),
    )
    .await
    .expect_err("401 must map to authentication before deserialize");

    match err {
        ProviderError::Authentication { provider, .. } => {
            assert_eq!(provider, PROVIDER_NAME);
        }
        other => panic!("expected authentication error, got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn openai_compatible_embeddings_maps_429_before_deserialize()
-> Result<(), Box<dyn std::error::Error>> {
    let body = r#"{"error":{"type":"rate_limit_error","message":"slow down","retry_after":7}}"#;
    let (api_base, _) = openai_like_json_response_url("429 Too Many Requests", body).await?;
    let provider = openai_compatible_embeddings_provider(api_base).await;

    let err = LLMProvider::embeddings(
        &provider,
        embedding_request(
            "text-embedding-3-small",
            EmbeddingInput::Text("hello".to_string()),
        ),
        RequestContext::default(),
    )
    .await
    .expect_err("429 must map to rate limit before deserialize");

    match err {
        ProviderError::RateLimit {
            provider,
            retry_after,
            ..
        } => {
            assert_eq!(provider, PROVIDER_NAME);
            assert_eq!(retry_after, Some(7));
        }
        other => panic!("expected rate limit error, got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn openai_compatible_embeddings_malformed_200_is_parse_error()
-> Result<(), Box<dyn std::error::Error>> {
    let (api_base, _) = openai_like_json_response_url("200 OK", r#"{"object":"list"}"#).await?;
    let provider = openai_compatible_embeddings_provider(api_base).await;

    let err = LLMProvider::embeddings(
        &provider,
        embedding_request(
            "text-embedding-3-small",
            EmbeddingInput::Text("hello".to_string()),
        ),
        RequestContext::default(),
    )
    .await
    .expect_err("malformed 200 must not become an empty success");

    match err {
        ProviderError::ResponseParsing { provider, .. } => {
            assert_eq!(provider, PROVIDER_NAME);
        }
        other => panic!("expected response parsing error, got {other:?}"),
    }
    Ok(())
}
