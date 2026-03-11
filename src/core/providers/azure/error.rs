//! Azure OpenAI Error Handling
//!
//! Simplified error handling for Azure OpenAI Service using ProviderError directly

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

/// Azure error mapper for unified error handling
#[derive(Debug)]
pub struct AzureErrorMapper;

impl ErrorMapper<ProviderError> for AzureErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        match status_code {
            400 => {
                ProviderError::invalid_request("azure", format!("Bad request: {}", response_body))
            }
            401 => ProviderError::authentication("azure", "Invalid Azure API key or credentials"),
            403 => ProviderError::authentication("azure", "Forbidden: insufficient permissions"),
            404 => azure_deployment_error("Azure deployment not found"),
            429 => ProviderError::rate_limit("azure", Some(60)),
            500..=599 => ProviderError::api_error(
                "azure",
                status_code,
                format!("Server error: {}", response_body),
            ),
            _ => HttpErrorMapper::map_status_code("azure", status_code, response_body),
        }
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> ProviderError {
        ProviderError::network("azure", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> ProviderError {
        ProviderError::serialization("azure", error.to_string())
    }
}

// Azure-specific error helper functions

/// Create an Azure AD authentication error
pub fn azure_ad_error(msg: impl Into<String>) -> ProviderError {
    ProviderError::authentication("azure", format!("Azure AD: {}", msg.into()))
}

/// Create an Azure deployment error
pub fn azure_deployment_error(msg: impl Into<String>) -> ProviderError {
    ProviderError::model_not_found("azure", msg.into())
}

/// Create an Azure configuration error
pub fn azure_config_error(msg: impl Into<String>) -> ProviderError {
    ProviderError::configuration("azure", msg.into())
}

/// Create an Azure API error with status code
pub fn azure_api_error(status: u16, msg: impl Into<String>) -> ProviderError {
    ProviderError::api_error("azure", status, msg.into())
}

/// Create an Azure header validation error
pub fn azure_header_error(msg: impl Into<String>) -> ProviderError {
    ProviderError::invalid_request("azure", format!("Invalid header: {}", msg.into()))
}

// Conversion implementations are in unified_provider.rs to avoid conflicts

/// Extract error message from Azure response
pub fn extract_azure_error_message(response: &serde_json::Value) -> String {
    if let Some(error) = response.get("error") {
        if let Some(message) = error.get("message")
            && let Some(msg_str) = message.as_str()
        {
            return msg_str.to_string();
        }
        // Try Azure-specific error format
        if let Some(code) = error.get("code")
            && let Some(code_str) = code.as_str()
        {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            return format!("{}: {}", code_str, message);
        }
    }

    // Fallback to generic message
    response.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_error_mapper_400() {
        let mapper = AzureErrorMapper;
        let err = mapper.map_http_error(400, "Invalid request body");
        assert!(matches!(err, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn test_azure_error_mapper_401() {
        let mapper = AzureErrorMapper;
        let err = mapper.map_http_error(401, "");
        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_azure_error_mapper_403() {
        let mapper = AzureErrorMapper;
        let err = mapper.map_http_error(403, "");
        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_azure_error_mapper_404() {
        let mapper = AzureErrorMapper;
        let err = mapper.map_http_error(404, "");
        assert!(matches!(err, ProviderError::ModelNotFound { .. }));
    }

    #[test]
    fn test_azure_error_mapper_429() {
        let mapper = AzureErrorMapper;
        let err = mapper.map_http_error(429, "");
        assert!(matches!(err, ProviderError::RateLimit { .. }));
    }

    #[test]
    fn test_azure_error_mapper_500() {
        let mapper = AzureErrorMapper;
        let err = mapper.map_http_error(500, "Internal error");
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_azure_error_mapper_503() {
        let mapper = AzureErrorMapper;
        let err = mapper.map_http_error(503, "Service unavailable");
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_azure_error_mapper_unknown() {
        let mapper = AzureErrorMapper;
        let err = mapper.map_http_error(418, "I'm a teapot");
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_azure_ad_error() {
        let err = azure_ad_error("token expired");
        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_azure_deployment_error() {
        let err = azure_deployment_error("deployment not found");
        assert!(matches!(err, ProviderError::ModelNotFound { .. }));
    }

    #[test]
    fn test_azure_config_error() {
        let err = azure_config_error("missing endpoint");
        assert!(matches!(err, ProviderError::Configuration { .. }));
    }

    #[test]
    fn test_azure_api_error() {
        let err = azure_api_error(500, "server error");
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_azure_header_error() {
        let err = azure_header_error("missing api key");
        assert!(matches!(err, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn test_extract_azure_error_message_with_error_message() {
        let response = serde_json::json!({
            "error": {
                "message": "The model does not exist"
            }
        });
        let msg = extract_azure_error_message(&response);
        assert_eq!(msg, "The model does not exist");
    }

    #[test]
    fn test_extract_azure_error_message_with_code() {
        // When message is present, it returns the message directly
        let response = serde_json::json!({
            "error": {
                "code": "InvalidRequest",
                "message": "Missing parameter"
            }
        });
        let msg = extract_azure_error_message(&response);
        assert_eq!(msg, "Missing parameter");
    }

    #[test]
    fn test_extract_azure_error_message_code_only() {
        // When only code is present (no message as string), it formats with code
        let response = serde_json::json!({
            "error": {
                "code": "InvalidRequest"
            }
        });
        let msg = extract_azure_error_message(&response);
        assert_eq!(msg, "InvalidRequest: Unknown error");
    }

    #[test]
    fn test_extract_azure_error_message_fallback() {
        let response = serde_json::json!({"status": "error"});
        let msg = extract_azure_error_message(&response);
        assert!(msg.contains("status"));
    }

    #[test]
    fn test_azure_error_mapper_network_error() {
        let mapper = AzureErrorMapper;
        let io_err =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused");
        let err = mapper.map_network_error(&io_err);
        assert!(matches!(err, ProviderError::Network { .. }));
    }

    #[test]
    fn test_azure_error_mapper_parsing_error() {
        let mapper = AzureErrorMapper;
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let err = mapper.map_parsing_error(&json_err);
        assert!(matches!(err, ProviderError::Serialization { .. }));
    }
}
