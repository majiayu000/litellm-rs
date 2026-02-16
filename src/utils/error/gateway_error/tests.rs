//! Tests for error handling

#[cfg(test)]
use super::types::GatewayError;
use crate::core::providers::unified_provider::ProviderError;

// ==================== Basic Error Creation Tests ====================

#[test]
fn test_error_creation() {
    let error = GatewayError::auth("Invalid token");
    assert!(matches!(error, GatewayError::Auth(_)));

    let error = GatewayError::bad_request("Missing parameter");
    assert!(matches!(error, GatewayError::BadRequest(_)));
}

#[test]
fn test_provider_error_creation() {
    let error = ProviderError::other("openai", "Bad request");
    assert!(matches!(error, ProviderError::Other { .. }));

    let error = ProviderError::rate_limit("openai", Some(60));
    assert!(matches!(error, ProviderError::RateLimit { .. }));
}

// ==================== Helper Function Tests ====================

#[test]
fn test_auth_helper() {
    let error = GatewayError::auth("Invalid API key");
    assert!(matches!(error, GatewayError::Auth(msg) if msg == "Invalid API key"));
}

#[test]
fn test_authorization_helper() {
    let error = GatewayError::authorization("Access denied");
    assert!(matches!(error, GatewayError::Forbidden(msg) if msg == "Access denied"));
}

#[test]
fn test_bad_request_helper() {
    let error = GatewayError::bad_request("Invalid JSON");
    assert!(matches!(error, GatewayError::BadRequest(msg) if msg == "Invalid JSON"));
}

#[test]
fn test_not_found_helper() {
    let error = GatewayError::not_found("Resource not found");
    assert!(matches!(error, GatewayError::NotFound(msg) if msg == "Resource not found"));
}

#[test]
fn test_conflict_helper() {
    let error = GatewayError::conflict("Resource already exists");
    assert!(matches!(error, GatewayError::Conflict(msg) if msg == "Resource already exists"));
}

#[test]
fn test_internal_helper() {
    let error = GatewayError::internal("Internal error");
    assert!(matches!(error, GatewayError::Internal(msg) if msg == "Internal error"));
}

#[test]
fn test_validation_helper() {
    let error = GatewayError::validation("Invalid input");
    assert!(matches!(error, GatewayError::Validation(msg) if msg == "Invalid input"));
}

#[test]
fn test_rate_limit_helper() {
    let error = GatewayError::rate_limit("Too many requests");
    assert!(matches!(error, GatewayError::RateLimit(msg) if msg == "Too many requests"));
}

#[test]
fn test_timeout_helper() {
    let error = GatewayError::timeout("Request timed out");
    assert!(matches!(error, GatewayError::Timeout(msg) if msg == "Request timed out"));
}

#[test]
fn test_service_unavailable_helper() {
    let error = GatewayError::service_unavailable("Service down");
    assert!(matches!(error, GatewayError::ProviderUnavailable(msg) if msg == "Service down"));
}

#[test]
fn test_server_helper() {
    let error = GatewayError::server("Server error");
    assert!(matches!(error, GatewayError::Internal(msg) if msg == "Server error"));
}

#[test]
fn test_network_helper() {
    let error = GatewayError::network("Connection refused");
    assert!(matches!(error, GatewayError::Network(msg) if msg == "Connection refused"));
}

#[test]
fn test_external_service_helper() {
    let error = GatewayError::external_service("External API failed");
    assert!(matches!(error, GatewayError::Internal(msg) if msg == "External API failed"));
}

#[test]
fn test_invalid_request_helper() {
    let error = GatewayError::invalid_request("Bad parameters");
    assert!(matches!(error, GatewayError::BadRequest(msg) if msg == "Bad parameters"));
}

#[test]
fn test_parsing_helper() {
    let error = GatewayError::parsing("Invalid format");
    assert!(matches!(error, GatewayError::Parsing(msg) if msg == "Invalid format"));
}

#[test]
fn test_alert_helper() {
    let error = GatewayError::alert("Critical alert");
    assert!(matches!(error, GatewayError::Alert(msg) if msg == "Critical alert"));
}

#[test]
fn test_not_implemented_helper() {
    let error = GatewayError::not_implemented("Feature not available");
    assert!(matches!(error, GatewayError::NotImplemented(msg) if msg == "Feature not available"));
}

#[test]
fn test_unauthorized_helper() {
    let error = GatewayError::unauthorized("No credentials");
    assert!(matches!(error, GatewayError::Auth(msg) if msg == "No credentials"));
}

#[test]
fn test_forbidden_helper() {
    let error = GatewayError::forbidden("Access forbidden");
    assert!(matches!(error, GatewayError::Forbidden(msg) if msg == "Access forbidden"));
}

#[test]
fn test_external_helper() {
    let error = GatewayError::external("External error");
    assert!(matches!(error, GatewayError::External(msg) if msg == "External error"));
}

#[test]
fn test_invalid_request_error_helper() {
    let error = GatewayError::invalid_request_error("Invalid data");
    assert!(matches!(error, GatewayError::BadRequest(msg) if msg == "Invalid data"));
}

#[test]
fn test_no_providers_available_helper() {
    let error = GatewayError::no_providers_available("No providers");
    assert!(matches!(error, GatewayError::NoProvidersAvailable(msg) if msg == "No providers"));
}

#[test]
fn test_provider_not_found_helper() {
    let error = GatewayError::provider_not_found("openai");
    assert!(matches!(error, GatewayError::ProviderNotFound(msg) if msg == "openai"));
}

#[test]
fn test_no_providers_for_model_helper() {
    let error = GatewayError::no_providers_for_model("gpt-5");
    assert!(matches!(error, GatewayError::NoProvidersForModel(msg) if msg == "gpt-5"));
}

#[test]
fn test_no_healthy_providers_helper() {
    let error = GatewayError::no_healthy_providers("All providers down");
    assert!(matches!(error, GatewayError::NoHealthyProviders(msg) if msg == "All providers down"));
}

#[test]
fn test_api_error_helper() {
    let error = GatewayError::api_error(500, "Server error", "openai");
    assert!(matches!(error, GatewayError::Internal(msg) if msg == "Server error"));
}

#[test]
fn test_unavailable_helper() {
    let error = GatewayError::unavailable("Provider unavailable");
    assert!(
        matches!(error, GatewayError::ProviderUnavailable(msg) if msg == "Provider unavailable")
    );
}

// ==================== Error Display Tests ====================

#[test]
fn test_error_display() {
    let error = GatewayError::auth("test message");
    let display = format!("{}", error);
    assert!(display.contains("test message"));
}

#[test]
fn test_all_error_variants_display() {
    // Test that all error variants have proper Display implementation
    let errors = vec![
        GatewayError::Config("config error".to_string()),
        GatewayError::Auth("auth error".to_string()),
        GatewayError::RateLimit("rate limit".to_string()),
        GatewayError::Validation("validation".to_string()),
        GatewayError::Cache("cache".to_string()),
        GatewayError::CircuitBreaker("circuit breaker".to_string()),
        GatewayError::Timeout("timeout".to_string()),
        GatewayError::NotFound("not found".to_string()),
        GatewayError::Conflict("conflict".to_string()),
        GatewayError::BadRequest("bad request".to_string()),
        GatewayError::Internal("internal".to_string()),
        GatewayError::ProviderUnavailable("unavailable".to_string()),
        GatewayError::Crypto("crypto".to_string()),
        GatewayError::FileStorage("file storage".to_string()),
        GatewayError::VectorDb("vector db".to_string()),
        GatewayError::Network("network".to_string()),
        GatewayError::Parsing("parsing".to_string()),
        GatewayError::Alert("alert".to_string()),
        GatewayError::NotImplemented("not impl".to_string()),
        GatewayError::Forbidden("forbidden".to_string()),
        GatewayError::External("external".to_string()),
        GatewayError::NoProvidersAvailable("no providers".to_string()),
        GatewayError::ProviderNotFound("provider not found".to_string()),
        GatewayError::NoProvidersForModel("no model".to_string()),
        GatewayError::NoHealthyProviders("no healthy".to_string()),
    ];

    for error in errors {
        let display = format!("{}", error);
        assert!(!display.is_empty(), "Error display should not be empty");
    }
}

// ==================== String Conversion Tests ====================

#[test]
fn test_helper_with_string() {
    let error = GatewayError::auth(String::from("test"));
    assert!(matches!(error, GatewayError::Auth(_)));
}

#[test]
fn test_helper_with_str() {
    let error = GatewayError::auth("test");
    assert!(matches!(error, GatewayError::Auth(_)));
}
