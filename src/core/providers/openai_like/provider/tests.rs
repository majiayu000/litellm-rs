use super::*;

#[tokio::test]
async fn test_provider_creation_with_api_base() {
    let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1").await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(provider.name(), "openai_like");
}

#[tokio::test]
async fn test_provider_creation_with_api_key() {
    let provider = OpenAILikeProvider::with_api_key("http://localhost:8000/v1", "sk-test123").await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_supports_any_model() {
    let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1")
        .await
        .unwrap_or_else(|err| panic!("OpenAI-like test provider should initialize: {err}"));

    assert!(provider.supports_model("gpt-4"));
    assert!(provider.supports_model("llama-2-70b"));
    assert!(provider.supports_model("any-custom-model"));
    assert!(provider.supports_model("custom/my-model"));
}

#[tokio::test]
async fn test_model_info_for_any_model() {
    let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1")
        .await
        .unwrap_or_else(|err| panic!("OpenAI-like test provider should initialize: {err}"));

    let info = provider.get_model_info("my-custom-model");
    assert_eq!(info.id, "my-custom-model");
    assert_eq!(info.provider, "openai_like");
    assert!(info.supports_streaming);
}

#[tokio::test]
async fn test_request_transformation() {
    let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1")
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

    let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1")
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
    let config = OpenAILikeConfig::new("http://localhost:8000/v1")
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
    let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1")
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
    let provider = OpenAILikeProvider::with_api_base("http://localhost:8000/v1")
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
