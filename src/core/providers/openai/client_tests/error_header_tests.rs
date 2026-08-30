use super::*;

// ==================== Error Mapper Tests ====================

#[test]
fn test_error_mapper_authentication() {
    let mapper = OpenAIErrorMapper;
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let error = mapper.map_http_error(401, "Unauthorized");
    match error {
        OpenAIError::Authentication { provider, .. } => {
            assert_eq!(provider, "openai");
        }
        _ => panic!("Expected Authentication error"),
    }
}

#[test]
fn test_error_mapper_rate_limit() {
    let mapper = OpenAIErrorMapper;
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let error = mapper.map_http_error(429, "Rate limit exceeded");
    match error {
        OpenAIError::RateLimit { provider, .. } => {
            assert_eq!(provider, "openai");
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[test]
fn test_error_mapper_invalid_request() {
    let mapper = OpenAIErrorMapper;
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let error = mapper.map_http_error(400, "Invalid request");
    match error {
        OpenAIError::InvalidRequest { provider, message } => {
            assert_eq!(provider, "openai");
            assert_eq!(message, "Invalid request");
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_error_mapper_api_error() {
    let mapper = OpenAIErrorMapper;
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let error = mapper.map_http_error(500, "Server error");
    match error {
        OpenAIError::ApiError {
            provider, status, ..
        } => {
            assert_eq!(provider, "openai");
            assert_eq!(status, 500);
        }
        _ => panic!("Expected ApiError error"),
    }
}

// ==================== Request Headers Tests ====================

#[test]
fn test_get_request_headers_with_api_key() {
    let provider = create_test_provider();
    let headers = provider.get_request_headers();

    assert!(!headers.is_empty());
    // Should have at least the Authorization header
    let has_auth = headers.iter().any(|h| h.0.as_ref() == "Authorization");
    assert!(has_auth);
}

#[test]
fn test_get_request_headers_with_organization() {
    let mut config = create_test_config();
    config.organization = Some("org-test123".to_string());

    let provider = OpenAIProvider {
        pool_manager: Arc::new(GlobalPoolManager::default()),
        config,
        model_registry: get_openai_registry(),
        model_identity: None,
    };

    let headers = provider.get_request_headers();
    let has_org = headers
        .iter()
        .any(|h| h.0.as_ref() == "OpenAI-Organization");
    assert!(has_org);
}

#[test]
fn test_get_request_headers_with_project() {
    let mut config = create_test_config();
    config.project = Some("proj-test123".to_string());

    let provider = OpenAIProvider {
        pool_manager: Arc::new(GlobalPoolManager::default()),
        config,
        model_registry: get_openai_registry(),
        model_identity: None,
    };

    let headers = provider.get_request_headers();
    let has_project = headers.iter().any(|h| h.0.as_ref() == "OpenAI-Project");
    assert!(has_project);
}

// ==================== Clone/Debug Tests ====================

#[test]
fn test_provider_clone() {
    let provider = create_test_provider();
    let cloned = provider.clone();

    assert_eq!(provider.name(), cloned.name());
    assert_eq!(provider.capabilities().len(), cloned.capabilities().len());
}

#[test]
fn test_provider_debug() {
    let provider = create_test_provider();
    let debug_str = format!("{:?}", provider);

    assert!(debug_str.contains("OpenAIProvider"));
}
