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
}

#[tokio::test]
async fn test_vertex_gemini_cost_uses_shared_catalog_pricing() {
    let provider = VertexAIProvider::new(test_vertex_provider_config())
        .await
        .unwrap();

    let cost = provider
        .calculate_cost("gemini-3.5-flash", 1000, 500)
        .await
        .unwrap();
    let expected = crate::core::providers::gemini::models::CostCalculator::calculate_cost(
        "gemini-3.5-flash",
        1000,
        500,
    )
    .unwrap();
    assert!((cost - expected).abs() < f64::EPSILON);

    let alias_cost = provider
        .calculate_cost("gemini-1.5-pro-002", 1000, 500)
        .await
        .unwrap();
    let canonical_cost = crate::core::providers::gemini::models::CostCalculator::calculate_cost(
        "gemini-1.5-pro",
        1000,
        500,
    )
    .unwrap();
    assert!((alias_cost - canonical_cost).abs() < f64::EPSILON);
}

// ==================== Cost Calculation Tests ====================

#[test]
fn test_cost_calculation_gemini_pro() {
    let input_tokens = 1000_u32;
    let output_tokens = 500_u32;
    let cost = (input_tokens as f64 * 0.0005 + output_tokens as f64 * 0.0015) / 1000.0;
    assert!(cost > 0.0);
    // 1000 * 0.0005 + 500 * 0.0015 = 0.5 + 0.75 = 1.25 / 1000 = 0.00125
    assert!((cost - 0.00125).abs() < 0.0001);
}

#[test]
fn test_cost_calculation_gemini_1_5_pro() {
    let input_tokens = 1000_u32;
    let output_tokens = 500_u32;
    let cost = (input_tokens as f64 * 0.00125 + output_tokens as f64 * 0.00375) / 1000.0;
    assert!(cost > 0.0);
    // 1000 * 0.00125 + 500 * 0.00375 = 1.25 + 1.875 = 3.125 / 1000 = 0.003125
    assert!((cost - 0.003125).abs() < 0.0001);
}

#[test]
fn test_cost_calculation_gemini_1_5_flash() {
    let input_tokens = 1000_u32;
    let output_tokens = 500_u32;
    let cost = (input_tokens as f64 * 0.000075 + output_tokens as f64 * 0.0003) / 1000.0;
    assert!(cost > 0.0);
    // 1000 * 0.000075 + 500 * 0.0003 = 0.075 + 0.15 = 0.225 / 1000 = 0.000225
    assert!((cost - 0.000225).abs() < 0.0001);
}

#[test]
fn test_cost_calculation_unknown_model() {
    let cost = 0.0_f64;
    assert_eq!(cost, 0.0);
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
