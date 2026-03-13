//! Helper functions for creating specific error types

use super::types::GatewayError;

/// Helper functions for creating specific errors
#[allow(dead_code)]
impl GatewayError {
    pub fn auth<S: Into<String>>(message: S) -> Self {
        Self::Auth(message.into())
    }

    pub fn authorization<S: Into<String>>(message: S) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn bad_request<S: Into<String>>(message: S) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn not_found<S: Into<String>>(message: S) -> Self {
        Self::NotFound(message.into())
    }

    pub fn conflict<S: Into<String>>(message: S) -> Self {
        Self::Conflict(message.into())
    }

    pub fn internal<S: Into<String>>(message: S) -> Self {
        Self::Internal(message.into())
    }

    pub fn validation<S: Into<String>>(message: S) -> Self {
        Self::Validation(message.into())
    }

    pub fn rate_limit<S: Into<String>>(message: S) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    pub fn timeout<S: Into<String>>(message: S) -> Self {
        Self::Timeout(message.into())
    }

    pub fn service_unavailable<S: Into<String>>(message: S) -> Self {
        Self::ProviderUnavailable(message.into())
    }

    pub fn server<S: Into<String>>(message: S) -> Self {
        Self::Internal(message.into())
    }

    pub fn network<S: Into<String>>(message: S) -> Self {
        Self::Network(message.into())
    }

    pub fn external_service<S: Into<String>>(message: S) -> Self {
        Self::Internal(message.into())
    }

    pub fn invalid_request<S: Into<String>>(message: S) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn parsing<S: Into<String>>(message: S) -> Self {
        Self::Parsing(message.into())
    }

    pub fn alert<S: Into<String>>(message: S) -> Self {
        Self::Alert(message.into())
    }

    pub fn not_implemented<S: Into<String>>(message: S) -> Self {
        Self::NotImplemented(message.into())
    }

    pub fn unauthorized<S: Into<String>>(message: S) -> Self {
        Self::Auth(message.into())
    }

    pub fn forbidden<S: Into<String>>(message: S) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn external<S: Into<String>>(message: S) -> Self {
        Self::External(message.into())
    }

    pub fn invalid_request_error<S: Into<String>>(message: S) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn no_providers_available<S: Into<String>>(message: S) -> Self {
        Self::NoProvidersAvailable(message.into())
    }

    pub fn provider_not_found<S: Into<String>>(message: S) -> Self {
        Self::ProviderNotFound(message.into())
    }

    pub fn no_providers_for_model<S: Into<String>>(message: S) -> Self {
        Self::NoProvidersForModel(message.into())
    }

    pub fn no_healthy_providers<S: Into<String>>(message: S) -> Self {
        Self::NoHealthyProviders(message.into())
    }
}

#[allow(dead_code)]
impl GatewayError {
    pub fn api_error<S: Into<String>>(_status_code: u16, message: S, _provider: S) -> Self {
        // ApiError doesn't exist in unified ProviderError, map to Internal in GatewayError
        Self::Internal(message.into())
    }

    pub fn unavailable<S: Into<String>>(message: S) -> Self {
        Self::ProviderUnavailable(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Auth Error Tests ====================

    #[test]
    fn test_auth_error_from_string() {
        let error = GatewayError::auth("Invalid API key");
        assert!(matches!(error, GatewayError::Auth(msg) if msg == "Invalid API key"));
    }

    #[test]
    fn test_auth_error_from_str() {
        let error = GatewayError::auth("Token expired");
        assert!(matches!(error, GatewayError::Auth(_)));
    }

    #[test]
    fn test_authorization_error() {
        let error = GatewayError::authorization("Insufficient permissions");
        assert!(matches!(error, GatewayError::Forbidden(msg) if msg == "Insufficient permissions"));
    }

    // ==================== Request Error Tests ====================

    #[test]
    fn test_bad_request_error() {
        let error = GatewayError::bad_request("Missing required field");
        assert!(matches!(error, GatewayError::BadRequest(msg) if msg == "Missing required field"));
    }

    #[test]
    fn test_not_found_error() {
        let error = GatewayError::not_found("Resource not found");
        assert!(matches!(error, GatewayError::NotFound(msg) if msg == "Resource not found"));
    }

    #[test]
    fn test_conflict_error() {
        let error = GatewayError::conflict("Resource already exists");
        assert!(matches!(error, GatewayError::Conflict(msg) if msg == "Resource already exists"));
    }

    #[test]
    fn test_validation_error() {
        let error = GatewayError::validation("Invalid input format");
        assert!(matches!(error, GatewayError::Validation(msg) if msg == "Invalid input format"));
    }

    #[test]
    fn test_invalid_request_error() {
        let error = GatewayError::invalid_request("Malformed JSON");
        assert!(matches!(error, GatewayError::BadRequest(msg) if msg == "Malformed JSON"));
    }

    #[test]
    fn test_invalid_request_error_method() {
        let error = GatewayError::invalid_request_error("Invalid format");
        assert!(matches!(error, GatewayError::BadRequest(msg) if msg == "Invalid format"));
    }

    // ==================== Server Error Tests ====================

    #[test]
    fn test_internal_error() {
        let error = GatewayError::internal("Internal server error");
        assert!(matches!(error, GatewayError::Internal(msg) if msg == "Internal server error"));
    }

    #[test]
    fn test_server_error_maps_to_internal() {
        let error = GatewayError::server("Server failure");
        assert!(matches!(error, GatewayError::Internal(msg) if msg == "Server failure"));
    }

    #[test]
    fn test_external_service_maps_to_internal() {
        let error = GatewayError::external_service("External API failed");
        assert!(matches!(error, GatewayError::Internal(msg) if msg == "External API failed"));
    }

    #[test]
    fn test_api_error_maps_to_internal() {
        let error = GatewayError::api_error(500, "Server error", "openai");
        assert!(matches!(error, GatewayError::Internal(msg) if msg == "Server error"));
    }

    // ==================== Rate Limiting Tests ====================

    #[test]
    fn test_rate_limit_error() {
        let error = GatewayError::rate_limit("Rate limit exceeded");
        assert!(
            matches!(error, GatewayError::RateLimit { ref message, .. } if message == "Rate limit exceeded")
        );
    }

    #[test]
    fn test_timeout_error() {
        let error = GatewayError::timeout("Request timed out after 30s");
        assert!(
            matches!(error, GatewayError::Timeout(msg) if msg == "Request timed out after 30s")
        );
    }

    // ==================== Provider Error Tests ====================

    #[test]
    fn test_service_unavailable_error() {
        let error = GatewayError::service_unavailable("Service under maintenance");
        assert!(
            matches!(error, GatewayError::ProviderUnavailable(msg) if msg == "Service under maintenance")
        );
    }

    #[test]
    fn test_unavailable_error() {
        let error = GatewayError::unavailable("Provider unavailable");
        assert!(
            matches!(error, GatewayError::ProviderUnavailable(msg) if msg == "Provider unavailable")
        );
    }

    #[test]
    fn test_no_providers_available() {
        let error = GatewayError::no_providers_available("No providers configured");
        assert!(
            matches!(error, GatewayError::NoProvidersAvailable(msg) if msg == "No providers configured")
        );
    }

    #[test]
    fn test_provider_not_found() {
        let error = GatewayError::provider_not_found("openai");
        assert!(matches!(error, GatewayError::ProviderNotFound(msg) if msg == "openai"));
    }

    #[test]
    fn test_no_providers_for_model() {
        let error = GatewayError::no_providers_for_model("gpt-5");
        assert!(matches!(error, GatewayError::NoProvidersForModel(msg) if msg == "gpt-5"));
    }

    #[test]
    fn test_no_healthy_providers() {
        let error = GatewayError::no_healthy_providers("All providers are down");
        assert!(
            matches!(error, GatewayError::NoHealthyProviders(msg) if msg == "All providers are down")
        );
    }

    // ==================== Network Error Tests ====================

    #[test]
    fn test_network_error() {
        let error = GatewayError::network("Connection refused");
        assert!(matches!(error, GatewayError::Network(msg) if msg == "Connection refused"));
    }

    // ==================== Parsing Error Tests ====================

    #[test]
    fn test_parsing_error() {
        let error = GatewayError::parsing("Invalid JSON syntax");
        assert!(matches!(error, GatewayError::Parsing(msg) if msg == "Invalid JSON syntax"));
    }

    // ==================== Alert Error Tests ====================

    #[test]
    fn test_alert_error() {
        let error = GatewayError::alert("Critical threshold exceeded");
        assert!(matches!(error, GatewayError::Alert(msg) if msg == "Critical threshold exceeded"));
    }

    // ==================== Not Implemented Error Tests ====================

    #[test]
    fn test_not_implemented_error() {
        let error = GatewayError::not_implemented("Feature not yet available");
        assert!(
            matches!(error, GatewayError::NotImplemented(msg) if msg == "Feature not yet available")
        );
    }

    // ==================== Authorization Error Tests ====================

    #[test]
    fn test_unauthorized_error() {
        let error = GatewayError::unauthorized("Invalid credentials");
        assert!(matches!(error, GatewayError::Auth(msg) if msg == "Invalid credentials"));
    }

    #[test]
    fn test_forbidden_error() {
        let error = GatewayError::forbidden("Access denied");
        assert!(matches!(error, GatewayError::Forbidden(msg) if msg == "Access denied"));
    }

    // ==================== External Error Tests ====================

    #[test]
    fn test_external_error() {
        let error = GatewayError::external("Third-party service error");
        assert!(matches!(error, GatewayError::External(msg) if msg == "Third-party service error"));
    }

    // ==================== String Conversion Tests ====================

    #[test]
    fn test_error_from_owned_string() {
        let message = String::from("Dynamic error message");
        let error = GatewayError::auth(message);
        assert!(matches!(error, GatewayError::Auth(msg) if msg == "Dynamic error message"));
    }

    #[test]
    fn test_error_from_string_slice() {
        let error = GatewayError::auth("Static error message");
        assert!(matches!(error, GatewayError::Auth(msg) if msg == "Static error message"));
    }

    #[test]
    fn test_error_with_format_string() {
        let model = "gpt-4";
        let error = GatewayError::no_providers_for_model(format!("No provider supports {}", model));
        assert!(matches!(error, GatewayError::NoProvidersForModel(msg) if msg.contains("gpt-4")));
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_error_with_empty_string() {
        let error = GatewayError::auth("");
        assert!(matches!(error, GatewayError::Auth(msg) if msg.is_empty()));
    }

    #[test]
    fn test_error_with_unicode() {
        let error = GatewayError::auth("认证失败 - Authentication failed");
        assert!(matches!(error, GatewayError::Auth(msg) if msg.contains("认证失败")));
    }

    #[test]
    fn test_error_with_special_characters() {
        let error = GatewayError::parsing("JSON error at line 5: unexpected '}'");
        assert!(matches!(error, GatewayError::Parsing(msg) if msg.contains("unexpected '}'")));
    }

    #[test]
    fn test_error_with_newlines() {
        let error = GatewayError::internal("Error details:\n- Issue 1\n- Issue 2");
        assert!(matches!(error, GatewayError::Internal(msg) if msg.contains('\n')));
    }

    // ==================== Consistency Tests ====================

    #[test]
    fn test_service_unavailable_matches_unavailable() {
        let error1 = GatewayError::service_unavailable("test");
        let error2 = GatewayError::unavailable("test");

        // Both should produce ProviderUnavailable
        assert!(matches!(error1, GatewayError::ProviderUnavailable(_)));
        assert!(matches!(error2, GatewayError::ProviderUnavailable(_)));
    }

    #[test]
    fn test_server_matches_internal() {
        let error1 = GatewayError::server("test");
        let error2 = GatewayError::internal("test");

        // Both should produce Internal
        assert!(matches!(error1, GatewayError::Internal(_)));
        assert!(matches!(error2, GatewayError::Internal(_)));
    }

    #[test]
    fn test_invalid_request_matches_bad_request() {
        let error1 = GatewayError::invalid_request("test");
        let error2 = GatewayError::bad_request("test");

        // Both should produce BadRequest
        assert!(matches!(error1, GatewayError::BadRequest(_)));
        assert!(matches!(error2, GatewayError::BadRequest(_)));
    }

    // ==================== All Helper Methods Coverage ====================

    #[test]
    fn test_all_helper_methods_exist() {
        // This test ensures all helper methods are callable
        let _ = GatewayError::auth("test");
        let _ = GatewayError::authorization("test");
        let _ = GatewayError::bad_request("test");
        let _ = GatewayError::not_found("test");
        let _ = GatewayError::conflict("test");
        let _ = GatewayError::internal("test");
        let _ = GatewayError::validation("test");
        let _ = GatewayError::rate_limit("test");
        let _ = GatewayError::timeout("test");
        let _ = GatewayError::service_unavailable("test");
        let _ = GatewayError::server("test");
        let _ = GatewayError::network("test");
        let _ = GatewayError::external_service("test");
        let _ = GatewayError::invalid_request("test");
        let _ = GatewayError::parsing("test");
        let _ = GatewayError::alert("test");
        let _ = GatewayError::not_implemented("test");
        let _ = GatewayError::unauthorized("test");
        let _ = GatewayError::forbidden("test");
        let _ = GatewayError::external("test");
        let _ = GatewayError::invalid_request_error("test");
        let _ = GatewayError::no_providers_available("test");
        let _ = GatewayError::provider_not_found("test");
        let _ = GatewayError::no_providers_for_model("test");
        let _ = GatewayError::no_healthy_providers("test");
        let _ = GatewayError::api_error(500, "test", "provider");
        let _ = GatewayError::unavailable("test");
    }
}
