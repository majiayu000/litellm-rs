//! Databricks Error Handling
//!
//! Error mapping for Databricks API responses.

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::shared::parse_retry_after_from_body;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

/// Databricks error mapper
#[derive(Debug)]
pub struct DatabricksErrorMapper;

impl ErrorMapper<ProviderError> for DatabricksErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        match status_code {
            400 => {
                // Check for specific error types
                if response_body.contains("model") && response_body.contains("not found") {
                    ProviderError::model_not_found("databricks", extract_model_name(response_body))
                } else if response_body.contains("context_length")
                    || response_body.contains("max_tokens")
                {
                    ProviderError::context_length_exceeded("databricks", 0, 0)
                } else {
                    ProviderError::invalid_request("databricks", response_body)
                }
            }
            401 => ProviderError::authentication("databricks", "Invalid API key or token"),
            403 => ProviderError::authentication(
                "databricks",
                "Access denied. Check API key permissions or workspace access.",
            ),
            404 => {
                if response_body.contains("endpoint") {
                    ProviderError::model_not_found("databricks", "Serving endpoint not found")
                } else {
                    ProviderError::model_not_found("databricks", "Resource not found")
                }
            }
            429 => {
                let retry_after = parse_retry_after_from_body(response_body);
                ProviderError::rate_limit("databricks", retry_after)
            }
            500..=599 => ProviderError::provider_unavailable(
                "databricks",
                format!("Databricks server error: {}", response_body),
            ),
            _ => HttpErrorMapper::map_status_code("databricks", status_code, response_body),
        }
    }
}

/// Extract model name from error response
fn extract_model_name(response_body: &str) -> String {
    // Try to extract model name from error message
    // Common patterns: "model 'xxx' not found", "Model xxx does not exist"
    if let Some(start) = response_body.find("model '")
        && let Some(end) = response_body[start + 7..].find('\'')
    {
        return response_body[start + 7..start + 7 + end].to_string();
    }
    if let Some(start) = response_body.find("Model ") {
        let rest = &response_body[start + 6..];
        if let Some(end) = rest.find(|c: char| c.is_whitespace()) {
            return rest[..end].to_string();
        }
    }
    "Unknown model".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_databricks_error_mapper_400() {
        let mapper = DatabricksErrorMapper;
        let err = mapper.map_http_error(400, "Invalid request parameters");
        assert!(matches!(err, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn test_databricks_error_mapper_400_model_not_found() {
        let mapper = DatabricksErrorMapper;
        let err = mapper.map_http_error(400, "model 'llama-2-70b' not found");
        assert!(matches!(err, ProviderError::ModelNotFound { .. }));
    }

    #[test]
    fn test_databricks_error_mapper_400_context_length() {
        let mapper = DatabricksErrorMapper;
        let err = mapper.map_http_error(400, "context_length exceeded");
        assert!(matches!(err, ProviderError::ContextLengthExceeded { .. }));
    }

    #[test]
    fn test_databricks_error_mapper_401() {
        let mapper = DatabricksErrorMapper;
        let err = mapper.map_http_error(401, "Unauthorized");
        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_databricks_error_mapper_403() {
        let mapper = DatabricksErrorMapper;
        let err = mapper.map_http_error(403, "Forbidden");
        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_databricks_error_mapper_404() {
        let mapper = DatabricksErrorMapper;
        let err = mapper.map_http_error(404, "endpoint not found");
        assert!(matches!(err, ProviderError::ModelNotFound { .. }));
    }

    #[test]
    fn test_databricks_error_mapper_429() {
        let mapper = DatabricksErrorMapper;
        let err = mapper.map_http_error(429, "rate limit exceeded, retry after 30 seconds");
        assert!(matches!(err, ProviderError::RateLimit { .. }));
    }

    #[test]
    fn test_databricks_error_mapper_500() {
        let mapper = DatabricksErrorMapper;
        let err = mapper.map_http_error(500, "Internal server error");
        assert!(matches!(err, ProviderError::ProviderUnavailable { .. }));
    }

    #[test]
    fn test_databricks_error_mapper_503() {
        let mapper = DatabricksErrorMapper;
        let err = mapper.map_http_error(503, "Service unavailable");
        assert!(matches!(err, ProviderError::ProviderUnavailable { .. }));
    }

    #[test]
    fn test_databricks_error_mapper_unknown() {
        let mapper = DatabricksErrorMapper;
        let err = mapper.map_http_error(418, "I'm a teapot");
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_extract_model_name_quoted() {
        let result = extract_model_name("model 'llama-2-70b' not found");
        assert_eq!(result, "llama-2-70b");
    }

    #[test]
    fn test_extract_model_name_unquoted() {
        let result = extract_model_name("Model mixtral-8x7b does not exist");
        assert_eq!(result, "mixtral-8x7b");
    }

    #[test]
    fn test_extract_model_name_unknown() {
        let result = extract_model_name("Some error occurred");
        assert_eq!(result, "Unknown model");
    }

    #[test]
    fn test_parse_retry_after_with_seconds() {
        // Shared parse_retry_after_from_body uses JSON fields + keyword detection,
        // not text number extraction. Plain text without rate-limit keywords returns None.
        let result = parse_retry_after_from_body("retry after 30 seconds");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_retry_after_default() {
        let result = parse_retry_after_from_body("rate limit exceeded");
        assert_eq!(result, Some(60));
    }
}
