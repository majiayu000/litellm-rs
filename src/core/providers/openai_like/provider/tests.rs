use super::*;
use crate::core::types::chat::ChatMessage;
use crate::core::types::context::RequestContext;
use crate::core::types::health::HealthStatus;
use crate::core::types::message::{MessageContent, MessageRole};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const TEST_PUBLIC_API_BASE: &str = "https://api.example.com/v1";

fn private_openai_like_config(api_base: impl Into<String>) -> OpenAILikeConfig {
    let mut config = crate::core::providers::openai_like::config::test_openai_like_config(api_base);
    config.base.endpoint_access = crate::core::net::ProviderEndpointAccess::PrivateNetwork;
    config
}

async fn read_full_http_request(socket: &mut TcpStream) -> std::io::Result<()> {
    let mut request_bytes = Vec::new();
    let mut buffer = [0_u8; 1024];

    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(());
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
                return Ok(());
            }
        }
    }
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
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

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
}

#[tokio::test]
async fn openai_compatible_declares_executable_proxy_capabilities() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    let config = OpenAILikeConfig::new(TEST_PUBLIC_API_BASE).with_skip_api_key(true);
    let provider = OpenAILikeProvider::new_openai_compatible(config)
        .await
        .expect("OpenAI-compatible provider should build");

    assert_eq!(
        provider.capabilities(),
        OPENAI_COMPATIBLE_PROXY_CAPABILITIES
    );
}

#[tokio::test]
async fn openai_like_catalog_rejects_invalid_capability_profiles() {
    static EMPTY: &[ProviderCapability] = &[];
    static DUPLICATE: &[ProviderCapability] = &[
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletion,
    ];
    static UNIMPLEMENTED: &[ProviderCapability] = &[ProviderCapability::ImageEdit];

    let config = OpenAILikeConfig::new(TEST_PUBLIC_API_BASE).with_skip_api_key(true);
    for (profile, expected) in [
        (EMPTY, "cannot be empty"),
        (DUPLICATE, "duplicate ChatCompletion"),
        (UNIMPLEMENTED, "not executable for this OpenAI-like profile"),
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
    use crate::core::providers::unified_provider::ProviderError;
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

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
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

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
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

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
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    let config = OpenAILikeConfig::new("https://api.x.ai/v1")
        .with_provider_name("xai")
        .with_skip_api_key(true);
    let provider = OpenAILikeProvider::new(config).await.unwrap();

    let cost = LLMProvider::calculate_cost(&provider, "grok-4.3", 250_000, 1_000)
        .await
        .unwrap();

    assert!((cost - 0.315).abs() < 1e-12);
}

#[tokio::test]
async fn test_non_xai_provider_does_not_use_xai_pricing() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

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
