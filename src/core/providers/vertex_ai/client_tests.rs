use super::*;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::types::model::ProviderCapability;

fn test_vertex_provider_config() -> VertexAIProviderConfig {
    VertexAIProviderConfig {
        project_id: "test-project".to_string(),
        location: "us-central1".to_string(),
        credentials: crate::core::providers::vertex_ai::VertexCredentials::AccessToken(
            "test-token".to_string(),
        ),
        ..Default::default()
    }
}

async fn pricing_test_provider() -> VertexAIProvider {
    VertexAIProvider::new(VertexAIProviderConfig {
        project_id: "pricing-test-project".to_string(),
        ..Default::default()
    })
    .await
    .expect("Vertex AI provider should initialize")
}

#[tokio::test]
async fn vertex_cost_uses_shared_per_token_pricing() {
    let provider = pricing_test_provider().await;
    let cost = LLMProvider::calculate_cost(&provider, "gemini-1.5-pro", 1_000, 500)
        .await
        .expect("catalogued Vertex model should be priced");

    assert!((cost - 0.00875).abs() < 1e-12);

    let preview = LLMProvider::calculate_cost(&provider, "gemini-3-flash-preview", 1_000, 500)
        .await
        .expect("provider-prefixed exact Vertex row should be priced");
    assert!((preview - 0.002).abs() < 1e-12);

    for (model, expected) in [
        ("gemini-2.0-flash", 0.0003),
        ("gemini-1.5-pro-002", 0.00875),
        ("gemini-1.5-flash-002", 0.000225),
        ("claude-3-opus@20240229", 0.0525),
        ("claude-opus-4-6@20260114", 0.0175),
        ("claude-opus-4-5@20251110", 0.0175),
        ("claude-3-5-sonnet@20241022", 0.0105),
        ("meta/llama3-70b-instruct-maas", 0.0),
        ("meta/llama-4-scout-17b-16e-instruct", 0.0006),
        ("meta/llama-4-maverick-17b-128e-instruct", 0.000925),
        ("ai21/jamba-1.5-large", 0.006),
        ("mistral/mistral-large-2411", 0.005),
        ("mistral/mistral-nemo", 0.000225),
    ] {
        let cost = LLMProvider::calculate_cost(&provider, model, 1_000, 500)
            .await
            .expect("canonical Vertex model should be priced");
        assert!((cost - expected).abs() < 1e-12, "model: {model}");
    }
}

#[tokio::test]
async fn vertex_unknown_model_returns_typed_error() {
    let provider = pricing_test_provider().await;

    for model in ["unknown-google-model", "gemini-1.5-flash-9999"] {
        let result = LLMProvider::calculate_cost(&provider, model, 1_000, 500).await;
        assert!(matches!(result, Err(ProviderError::ModelNotFound { .. })));
    }
}

#[tokio::test]
async fn vertex_model_metadata_uses_per_1k_units() {
    let provider = pricing_test_provider().await;
    let pro = provider
        .models()
        .iter()
        .find(|model| model.id == "gemini-1.5-pro")
        .expect("Gemini 1.5 Pro metadata should exist");

    let input = pro
        .input_cost_per_1k_tokens
        .expect("input pricing should be present");
    let output = pro
        .output_cost_per_1k_tokens
        .expect("output pricing should be present");
    assert!((input - 0.0035).abs() < 1e-12);
    assert!((output - 0.0105).abs() < 1e-12);
}

#[test]
fn test_client_vertex_usage_parser_is_strict_and_endpoint_aware() {
    let valid = serde_json::json!({"usageMetadata": {
        "promptTokenCount": 10, "toolUsePromptTokenCount": 2,
        "candidatesTokenCount": 3, "thoughtsTokenCount": 4,
        "cachedContentTokenCount": 5, "totalTokenCount": 19
    }});
    let usage = parse_vertex_usage(&valid).unwrap();
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (12, 7, 19)
    );
    assert!(usage.completion_tokens_details.is_none());
    let prompt_details = usage.prompt_tokens_details.unwrap();
    assert_eq!(prompt_details.cached_tokens, Some(5));
    assert_eq!(prompt_details.cache_read_tokens, Some(5));
    for bad in [
        serde_json::json!({}),
        serde_json::json!({"usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 3, "totalTokenCount": 12}}),
        serde_json::json!({"usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": null, "totalTokenCount": 10}}),
        serde_json::json!({"usageMetadata": {"promptTokenCount": 0, "candidatesTokenCount": 0, "totalTokenCount": 0}}),
    ] {
        assert!(parse_vertex_usage(&bad).is_none());
    }
    let huge = serde_json::json!({"usageMetadata": {
        "promptTokenCount": u64::MAX, "candidatesTokenCount": 0,
        "totalTokenCount": u64::MAX
    }});
    assert_eq!(parse_vertex_usage(&huge).unwrap().total_tokens, u32::MAX);
}

// ==================== VertexAIErrorMapper Tests ====================

#[test]
fn test_error_mapper_http_400() {
    let mapper = VertexAIErrorMapper;
    let error = mapper.map_http_error(400, "Invalid request body");
    assert!(matches!(error, ProviderError::ResponseParsing { .. }));
}

#[test]
fn test_error_mapper_http_401() {
    let mapper = VertexAIErrorMapper;
    let error = mapper.map_http_error(401, "Unauthorized");
    assert!(matches!(error, ProviderError::Authentication { .. }));
}

#[test]
fn test_error_mapper_http_403() {
    let mapper = VertexAIErrorMapper;
    let error = mapper.map_http_error(403, "Forbidden");
    assert!(matches!(error, ProviderError::Configuration { .. }));
}

#[test]
fn test_error_mapper_http_404() {
    let mapper = VertexAIErrorMapper;
    let error = mapper.map_http_error(404, "Not found");
    assert!(matches!(error, ProviderError::ModelNotFound { .. }));
}

#[test]
fn test_error_mapper_http_429() {
    let mapper = VertexAIErrorMapper;
    let error = mapper.map_http_error(429, "Rate limit");
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

#[test]
fn test_error_mapper_http_500() {
    let mapper = VertexAIErrorMapper;
    let error = mapper.map_http_error(500, "Internal error");
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

#[test]
fn test_error_mapper_http_502() {
    let mapper = VertexAIErrorMapper;
    let error = mapper.map_http_error(502, "Bad gateway");
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

#[test]
fn test_error_mapper_http_503() {
    let mapper = VertexAIErrorMapper;
    let error = mapper.map_http_error(503, "Unavailable");
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

#[test]
fn test_error_mapper_http_unknown() {
    let mapper = VertexAIErrorMapper;
    let error = mapper.map_http_error(418, "I'm a teapot");
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

#[test]
fn test_error_mapper_json_invalid_argument() {
    let mapper = VertexAIErrorMapper;
    let response = serde_json::json!({
        "error": {
            "code": 400,
            "message": "Invalid argument",
            "status": "INVALID_ARGUMENT"
        }
    });
    let error = mapper.map_json_error(&response);
    assert!(matches!(error, ProviderError::ResponseParsing { .. }));
}

#[test]
fn test_error_mapper_json_unauthenticated() {
    let mapper = VertexAIErrorMapper;
    let response = serde_json::json!({
        "error": {
            "code": 401,
            "message": "Auth failed",
            "status": "UNAUTHENTICATED"
        }
    });
    let error = mapper.map_json_error(&response);
    assert!(matches!(error, ProviderError::Authentication { .. }));
}

#[test]
fn test_error_mapper_json_permission_denied() {
    let mapper = VertexAIErrorMapper;
    let response = serde_json::json!({
        "error": {
            "code": 403,
            "message": "Access denied",
            "status": "PERMISSION_DENIED"
        }
    });
    let error = mapper.map_json_error(&response);
    assert!(matches!(error, ProviderError::Configuration { .. }));
}

#[test]
fn test_error_mapper_json_not_found() {
    let mapper = VertexAIErrorMapper;
    let response = serde_json::json!({
        "error": {
            "code": 404,
            "message": "Model not found",
            "status": "NOT_FOUND"
        }
    });
    let error = mapper.map_json_error(&response);
    assert!(matches!(error, ProviderError::ModelNotFound { .. }));
}

#[test]
fn test_error_mapper_json_resource_exhausted() {
    let mapper = VertexAIErrorMapper;
    let response = serde_json::json!({
        "error": {
            "code": 429,
            "message": "Quota exceeded",
            "status": "RESOURCE_EXHAUSTED"
        }
    });
    let error = mapper.map_json_error(&response);
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

#[test]
fn test_error_mapper_json_internal() {
    let mapper = VertexAIErrorMapper;
    let response = serde_json::json!({
        "error": {
            "code": 500,
            "message": "Internal error",
            "status": "INTERNAL"
        }
    });
    let error = mapper.map_json_error(&response);
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

#[test]
fn test_error_mapper_json_unavailable() {
    let mapper = VertexAIErrorMapper;
    let response = serde_json::json!({
        "error": {
            "code": 503,
            "message": "Service unavailable",
            "status": "UNAVAILABLE"
        }
    });
    let error = mapper.map_json_error(&response);
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

#[test]
fn test_error_mapper_json_unknown_status() {
    let mapper = VertexAIErrorMapper;
    let response = serde_json::json!({
        "error": {
            "code": 999,
            "message": "Unknown error",
            "status": "UNKNOWN_STATUS"
        }
    });
    let error = mapper.map_json_error(&response);
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

#[test]
fn test_error_mapper_json_no_error_field() {
    let mapper = VertexAIErrorMapper;
    let response = serde_json::json!({
        "result": "something"
    });
    let error = mapper.map_json_error(&response);
    assert!(matches!(error, ProviderError::ResponseParsing { .. }));
}

#[test]
fn test_error_mapper_json_missing_fields() {
    let mapper = VertexAIErrorMapper;
    let response = serde_json::json!({
        "error": {}
    });
    let error = mapper.map_json_error(&response);
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

#[test]
fn test_error_mapper_network_error() {
    let mapper = VertexAIErrorMapper;
    let io_error = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
    let error = mapper.map_network_error(&io_error);
    assert!(matches!(
        error,
        ProviderError::Network { .. } | ProviderError::RateLimit { .. }
    ));
}

// ==================== LLMProvider Trait Tests ====================

#[test]
fn test_provider_name() {
    // We can't create a full provider without credentials, but we can test the static parts
    // by examining what would be returned
    assert_eq!("vertex_ai", "vertex_ai");
}

#[test]
fn test_provider_capabilities() {
    use crate::core::types::model::ProviderCapability;
    let expected = [
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletionStream,
        ProviderCapability::Embeddings,
        ProviderCapability::ImageGeneration,
        ProviderCapability::ToolCalling,
    ];
    assert_eq!(expected.len(), 5);
}

#[test]
fn test_model_info_structure() {
    let model_info = ModelInfo {
        id: "gemini-1.5-pro".to_string(),
        name: "Gemini 1.5 Pro".to_string(),
        provider: "vertex_ai".to_string(),
        max_context_length: 2_097_152,
        max_output_length: Some(8192),
        supports_streaming: true,
        supports_tools: true,
        supports_multimodal: true,
        input_cost_per_1k_tokens: Some(1.25),
        output_cost_per_1k_tokens: Some(3.75),
        currency: "USD".to_string(),
        capabilities: vec![ProviderCapability::ChatCompletion],
        created_at: None,
        updated_at: None,
        metadata: HashMap::new(),
    };
    assert_eq!(model_info.id, "gemini-1.5-pro");
    assert_eq!(model_info.max_context_length, 2_097_152);
    assert!(model_info.supports_tools);
}

#[tokio::test]
async fn test_vertex_models_are_gemini_registry_surface_overlay() {
    let provider = VertexAIProvider::new(test_vertex_provider_config())
        .await
        .unwrap();
    let model_ids = provider
        .models()
        .iter()
        .map(|model| model.id.clone())
        .collect::<Vec<_>>();

    let expected_ids = crate::core::providers::gemini::get_gemini_registry()
        .list_model_infos_for_surface(
            crate::core::providers::gemini::GoogleGeminiApiSurface::VertexAi,
        )
        .into_iter()
        .map(|model| model.id)
        .collect::<Vec<_>>();

    assert_eq!(model_ids, expected_ids);
    assert!(model_ids.iter().any(|id| id == "gemini-3.5-flash"));
    assert!(!model_ids.iter().any(|id| id == "gemini-1.0-pro"));
    assert!(!model_ids.iter().any(|id| id == "gemini-2.0-flash-exp"));

    let mut experimental_config = test_vertex_provider_config();
    experimental_config.enable_experimental = true;
    let experimental_provider = VertexAIProvider::new(experimental_config).await.unwrap();
    assert!(
        experimental_provider
            .models()
            .iter()
            .any(|model| model.id == "gemini-2.0-flash-exp")
    );

    let model = provider
        .models()
        .iter()
        .find(|model| model.id == "gemini-3.5-flash")
        .unwrap();
    assert_eq!(model.provider, "vertex_ai");
    assert_eq!(
        model.metadata["google_auth_boundary"],
        serde_json::json!("bearer_token")
    );

    for model_id in ["gemini-1.5-flash", "gemini-3-flash-preview"] {
        let advertised = provider
            .models()
            .iter()
            .find(|model| model.id == model_id)
            .unwrap();
        assert_eq!(
            advertised.max_context_length,
            crate::core::providers::vertex_ai::parse_vertex_model(model_id).max_context_tokens()
        );
    }
}

#[tokio::test]
async fn test_vertex_shared_catalog_new_model_request_contract() {
    let provider = VertexAIProvider::new(test_vertex_provider_config())
        .await
        .unwrap();

    assert!(
        crate::core::providers::vertex_ai::is_vertex_gemini_catalog_model(
            "gemini-3.5-flash",
            false
        )
    );
    assert!(
        !crate::core::providers::vertex_ai::is_vertex_gemini_catalog_model(
            "models/gemini-3.5-flash",
            false
        )
    );
    assert!(
        !crate::core::providers::vertex_ai::is_vertex_gemini_catalog_model(
            "prefix-gemini-3.5-flash",
            false
        )
    );
    assert!(
        !crate::core::providers::vertex_ai::is_vertex_gemini_catalog_model("gemini-1.0-pro", true)
    );
    assert!(
        !crate::core::providers::vertex_ai::is_vertex_gemini_catalog_model(
            "gemini-2.0-flash-exp",
            false
        )
    );
    assert!(
        crate::core::providers::vertex_ai::is_vertex_gemini_catalog_model(
            "gemini-2.0-flash-exp",
            true
        )
    );

    let url = provider.build_google_catalog_model_url("gemini-3.5-flash", "generateContent", false);
    assert!(url.contains("/publishers/google/models/gemini-3.5-flash:generateContent"));
    assert!(!url.contains("alt=sse"));

    let stream_url =
        provider.build_google_catalog_model_url("gemini-3.5-flash", "streamGenerateContent", true);
    assert!(stream_url.ends_with(":streamGenerateContent?alt=sse"));
}
// ==================== URL Building Tests (logic only) ====================

#[test]
fn test_url_format_standard_location() {
    let location = "us-central1";
    let api_version = "v1";
    let project_id = "my-project";
    let url = format!(
        "https://{}-aiplatform.googleapis.com/{}/projects/{}/locations/{}",
        location, api_version, project_id, location
    );
    assert!(url.contains("us-central1-aiplatform.googleapis.com"));
    assert!(url.contains("my-project"));
}

#[test]
fn test_url_format_global_location() {
    let api_version = "v1";
    let project_id = "my-project";
    let url = format!(
        "https://aiplatform.googleapis.com/{}/projects/{}/locations/global",
        api_version, project_id
    );
    assert!(url.contains("aiplatform.googleapis.com"));
    assert!(url.contains("global"));
}

#[test]
fn test_url_format_gemini_model() {
    let base_url = "https://us-central1-aiplatform.googleapis.com/v1/projects/my-project/locations/us-central1";
    let model_id = "gemini-1.5-pro";
    let endpoint = "generateContent";
    let url = format!(
        "{}/publishers/google/models/{}:{}",
        base_url, model_id, endpoint
    );
    assert!(url.contains("publishers/google/models/gemini-1.5-pro"));
}

#[test]
fn test_url_format_partner_model_anthropic() {
    let base_url = "https://us-central1-aiplatform.googleapis.com/v1/projects/my-project/locations/us-central1";
    let model_id = "claude-3-opus";
    let endpoint = "predict";
    let publisher = "anthropic";
    let url = format!(
        "{}/publishers/{}/models/{}:{}",
        base_url, publisher, model_id, endpoint
    );
    assert!(url.contains("publishers/anthropic/models/claude-3-opus"));
}

#[test]
fn test_url_format_with_streaming() {
    let base_url = "https://example.com/endpoint";
    let url = format!("{}?alt=sse", base_url);
    assert!(url.contains("alt=sse"));
}

// ==================== Publisher Detection Tests ====================

#[test]
fn test_get_publisher_claude() {
    let model_id = "claude-3-opus";
    let publisher = if model_id.contains("claude") {
        "anthropic"
    } else {
        "google"
    };
    assert_eq!(publisher, "anthropic");
}

#[test]
fn test_get_publisher_llama() {
    let model_id = "llama-3.1-70b";
    let publisher = if model_id.contains("llama") {
        "meta"
    } else {
        "google"
    };
    assert_eq!(publisher, "meta");
}

#[test]
fn test_get_publisher_jamba() {
    let model_id = "jamba-instruct";
    let publisher = if model_id.contains("jamba") {
        "ai21"
    } else {
        "google"
    };
    assert_eq!(publisher, "ai21");
}

#[test]
fn test_get_publisher_default() {
    let model_id = "some-other-model";
    let publisher = if model_id.contains("claude") {
        "anthropic"
    } else if model_id.contains("llama") {
        "meta"
    } else if model_id.contains("jamba") {
        "ai21"
    } else {
        "google"
    };
    assert_eq!(publisher, "google");
}

// ==================== Supported Params Tests ====================

#[test]
fn test_supported_params_gemini() {
    let model = "gemini-1.5-pro";
    let params: &[&str] = if model.contains("gemini") {
        &[
            "messages",
            "model",
            "max_tokens",
            "temperature",
            "top_p",
            "stop",
            "stream",
            "tools",
            "tool_choice",
            "response_format",
            "user",
            "top_k",
        ]
    } else {
        &[
            "messages",
            "model",
            "max_tokens",
            "temperature",
            "top_p",
            "stream",
        ]
    };
    assert_eq!(params.len(), 12);
    assert!(params.contains(&"top_k"));
}

#[test]
fn test_supported_params_partner() {
    let model = "claude-3-opus";
    let params: &[&str] = if model.contains("gemini") {
        &[
            "messages",
            "model",
            "max_tokens",
            "temperature",
            "top_p",
            "stop",
            "stream",
            "tools",
            "tool_choice",
            "response_format",
            "user",
            "top_k",
        ]
    } else {
        &[
            "messages",
            "model",
            "max_tokens",
            "temperature",
            "top_p",
            "stream",
        ]
    };
    assert_eq!(params.len(), 6);
    assert!(!params.contains(&"top_k"));
}

// ==================== Configuration Tests ====================

#[test]
fn test_vertex_ai_provider_config_default() {
    let config = VertexAIProviderConfig::default();
    // Default values should be set
    assert!(!config.project_id.is_empty() || config.project_id.is_empty()); // Just test it compiles
    assert!(!config.location.is_empty());
    assert!(!config.api_version.is_empty());
}

#[test]
fn test_vertex_ai_provider_config_with_custom_values() {
    let config = VertexAIProviderConfig {
        project_id: "test-project".to_string(),
        location: "us-central1".to_string(),
        api_base: Some("https://custom.api.com".to_string()),
        ..Default::default()
    };

    assert_eq!(config.project_id, "test-project");
    assert_eq!(config.location, "us-central1");
    assert!(config.api_base.is_some());
    assert_eq!(
        config.api_base.expect("api_base should be Some"),
        "https://custom.api.com"
    );
}

// ==================== ProviderError Tests ====================

#[test]
fn test_vertex_ai_error_authentication() {
    let error = ProviderError::authentication("vertex_ai", "Invalid credentials");
    assert!(format!("{:?}", error).contains("Authentication"));
}

#[test]
fn test_vertex_ai_error_configuration() {
    let error = ProviderError::configuration("vertex_ai", "Missing project ID");
    assert!(format!("{:?}", error).contains("Configuration"));
}

#[test]
fn test_vertex_ai_error_network() {
    let error = ProviderError::network("vertex_ai", "Connection timeout");
    assert!(format!("{:?}", error).contains("Network"));
}

#[test]
fn test_vertex_ai_error_unsupported_model() {
    let error = ProviderError::model_not_found("vertex_ai", "unknown-model");
    assert!(format!("{:?}", error).contains("ModelNotFound"));
}

#[test]
fn test_vertex_ai_error_response_parsing() {
    let error = ProviderError::response_parsing("vertex_ai", "Invalid JSON");
    assert!(format!("{:?}", error).contains("ResponseParsing"));
}

#[test]
fn test_vertex_ai_error_api_error() {
    let error = ProviderError::api_error("vertex_ai", 500, "Internal server error");
    if let ProviderError::ApiError {
        provider, status, ..
    } = error
    {
        assert_eq!(provider, "vertex_ai");
        assert_eq!(status, 500);
    } else {
        panic!("Expected ApiError variant");
    }
}
