//! Heroku Error Handling
//!
//! Error handling for Heroku AI Inference API

use super::config::PROVIDER_NAME;
use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::shared::parse_retry_after_from_body;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

/// Heroku Error Mapper
#[derive(Debug)]
pub struct HerokuErrorMapper;

impl ErrorMapper<ProviderError> for HerokuErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        match status_code {
            401 => ProviderError::authentication(PROVIDER_NAME, "Invalid API key"),
            403 => ProviderError::authentication(
                PROVIDER_NAME,
                "Permission denied - check your Heroku Inference access",
            ),
            404 => {
                // Could be model not found or endpoint not found
                if response_body.contains("model") {
                    ProviderError::model_not_found(
                        PROVIDER_NAME,
                        extract_model_from_body(response_body),
                    )
                } else {
                    ProviderError::api_error(PROVIDER_NAME, status_code, "Resource not found")
                }
            }
            429 => {
                let retry_after = parse_retry_after_from_body(response_body);
                ProviderError::rate_limit(PROVIDER_NAME, retry_after)
            }
            400 => {
                // Parse the error message for more context
                let message = extract_error_message(response_body)
                    .unwrap_or_else(|| response_body.to_string());
                ProviderError::invalid_request(PROVIDER_NAME, message)
            }
            500..=599 => {
                HttpErrorMapper::map_status_code(PROVIDER_NAME, status_code, response_body)
            }
            _ => HttpErrorMapper::map_status_code(PROVIDER_NAME, status_code, response_body),
        }
    }
}

/// Extract model name from error response body
fn extract_model_from_body(response_body: &str) -> String {
    // Try to parse as JSON and extract model info
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(response_body)
        && let Some(model) = json
            .get("error")
            .and_then(|e| e.get("param"))
            .and_then(|p| p.as_str())
    {
        return model.to_string();
    }
    "unknown".to_string()
}

/// Extract error message from response body
fn extract_error_message(response_body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(response_body)
        .ok()
        .and_then(|json| {
            json.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heroku_error_mapper_401() {
        let mapper = HerokuErrorMapper;
        let err = mapper.map_http_error(401, "Unauthorized");
        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_heroku_error_mapper_403() {
        let mapper = HerokuErrorMapper;
        let err = mapper.map_http_error(403, "Forbidden");
        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_heroku_error_mapper_404_model() {
        let mapper = HerokuErrorMapper;
        let err = mapper.map_http_error(
            404,
            r#"{"error": {"message": "Model not found", "param": "invalid-model"}}"#,
        );
        assert!(matches!(err, ProviderError::ModelNotFound { .. }));
    }

    #[test]
    fn test_heroku_error_mapper_404_generic() {
        let mapper = HerokuErrorMapper;
        let err = mapper.map_http_error(404, "Not found");
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_heroku_error_mapper_429() {
        let mapper = HerokuErrorMapper;
        let err = mapper.map_http_error(429, "rate limit exceeded");
        assert!(matches!(err, ProviderError::RateLimit { .. }));
    }

    #[test]
    fn test_heroku_error_mapper_429_with_retry_after() {
        let mapper = HerokuErrorMapper;
        let err = mapper.map_http_error(
            429,
            r#"{"error": {"message": "Rate limit", "retry_after": 30}}"#,
        );
        if let ProviderError::RateLimit { retry_after, .. } = err {
            assert_eq!(retry_after, Some(30));
        } else {
            panic!("Expected RateLimit error");
        }
    }

    #[test]
    fn test_heroku_error_mapper_400() {
        let mapper = HerokuErrorMapper;
        let err = mapper.map_http_error(
            400,
            r#"{"error": {"message": "Invalid request parameter"}}"#,
        );
        assert!(matches!(err, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn test_heroku_error_mapper_500() {
        let mapper = HerokuErrorMapper;
        let err = mapper.map_http_error(500, "Internal error");
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_heroku_error_mapper_503() {
        let mapper = HerokuErrorMapper;
        let err = mapper.map_http_error(503, "Service unavailable");
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_heroku_error_mapper_unknown() {
        let mapper = HerokuErrorMapper;
        let err = mapper.map_http_error(418, "I'm a teapot");
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_parse_retry_after_with_rate_limit() {
        let result = parse_retry_after_from_body("rate limit exceeded");
        assert_eq!(result, Some(60));
    }

    #[test]
    fn test_parse_retry_after_with_json() {
        let result = parse_retry_after_from_body(r#"{"error": {"retry_after": 120}}"#);
        assert_eq!(result, Some(120));
    }

    #[test]
    fn test_parse_retry_after_without_rate_limit() {
        let result = parse_retry_after_from_body("other error");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_retry_after_empty() {
        let result = parse_retry_after_from_body("");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_error_message() {
        let body = r#"{"error": {"message": "Test error message"}}"#;
        let result = extract_error_message(body);
        assert_eq!(result, Some("Test error message".to_string()));
    }

    #[test]
    fn test_extract_error_message_invalid_json() {
        let result = extract_error_message("not json");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_model_from_body() {
        let body = r#"{"error": {"message": "Model not found", "param": "my-model"}}"#;
        let result = extract_model_from_body(body);
        assert_eq!(result, "my-model");
    }

    #[test]
    fn test_extract_model_from_body_invalid() {
        let result = extract_model_from_body("not json");
        assert_eq!(result, "unknown");
    }
}
