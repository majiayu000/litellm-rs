//! Middleware tests

use super::auth_rate_limiter::AuthRateLimiter;
use super::helpers::{
    extract_auth_method, extract_auth_method_with_api_key_header, is_admin_route, is_api_route,
    is_public_route,
};
use crate::auth::AuthMethod;
use actix_web::http::header::{HeaderMap, HeaderName, HeaderValue};

#[test]
fn test_extract_auth_method_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_static("Bearer token123"),
    );

    let auth_method = extract_auth_method(&headers);
    assert!(matches!(auth_method, AuthMethod::Jwt(token) if token == "token123"));
}

#[test]
fn test_extract_auth_method_api_key() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_static("ApiKey key123"),
    );

    let auth_method = extract_auth_method(&headers);
    assert!(matches!(auth_method, AuthMethod::ApiKey(key) if key == "key123"));
}

#[test]
fn test_extract_auth_method_x_api_key() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-api-key"),
        HeaderValue::from_static("key123"),
    );

    let auth_method = extract_auth_method(&headers);
    assert!(matches!(auth_method, AuthMethod::ApiKey(key) if key == "key123"));
}

#[test]
fn test_extract_auth_method_custom_api_key_header() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-gateway-api-key"),
        HeaderValue::from_static("gw-custom-123"),
    );

    let auth_method = extract_auth_method_with_api_key_header(&headers, "x-gateway-api-key");
    assert!(matches!(auth_method, AuthMethod::ApiKey(key) if key == "gw-custom-123"));
}

#[test]
fn test_extract_auth_method_custom_header_fallback_x_api_key() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-api-key"),
        HeaderValue::from_static("fallback-key"),
    );

    let auth_method = extract_auth_method_with_api_key_header(&headers, "x-gateway-api-key");
    assert!(matches!(auth_method, AuthMethod::ApiKey(key) if key == "fallback-key"));
}

#[test]
fn test_extract_auth_method_session() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("cookie"),
        HeaderValue::from_static("session=sess123; other=value"),
    );

    let auth_method = extract_auth_method(&headers);
    assert!(matches!(auth_method, AuthMethod::Session(session) if session == "sess123"));
}

#[test]
fn test_extract_auth_method_none() {
    let headers = HeaderMap::new();
    let auth_method = extract_auth_method(&headers);
    assert!(matches!(auth_method, AuthMethod::None));
}

#[test]
fn test_is_public_route() {
    assert!(is_public_route("/health"));
    assert!(is_public_route("/auth/login"));
    assert!(is_public_route("/auth/login/callback"));
    // /metrics requires authentication (not in PUBLIC_ROUTES)
    assert!(!is_public_route("/metrics"));
    // Prefix bypass must be prevented
    assert!(!is_public_route("/auth/login_evil"));
    assert!(!is_public_route("/healthz"));
    assert!(!is_public_route("/api/users"));
    assert!(!is_public_route("/v1/chat/completions"));
}

#[test]
fn test_is_admin_route() {
    assert!(is_admin_route("/admin/users"));
    assert!(is_admin_route("/api/admin/config"));
    assert!(!is_admin_route("/api/users"));
    assert!(!is_admin_route("/health"));
}

#[test]
fn test_is_api_route() {
    assert!(is_api_route("/v1/chat/completions"));
    assert!(is_api_route("/v1/embeddings"));
    assert!(is_api_route("/v1/models"));
    assert!(!is_api_route("/api/users"));
    assert!(!is_api_route("/health"));
}

#[test]
fn test_auth_rate_limiter_allows_initial_attempts() {
    let limiter = AuthRateLimiter::new(3, 60, 30);
    let client_id = "test_client_1";

    assert!(limiter.check_allowed(client_id).is_ok());
    assert!(limiter.record_failure(client_id).is_none());

    assert!(limiter.check_allowed(client_id).is_ok());
    assert!(limiter.record_failure(client_id).is_none());
}

#[test]
fn test_auth_rate_limiter_locks_after_max_attempts() {
    let limiter = AuthRateLimiter::new(3, 60, 30);
    let client_id = "test_client_2";

    limiter.record_failure(client_id);
    limiter.record_failure(client_id);

    let lockout = limiter.record_failure(client_id);
    assert!(lockout.is_some());
    assert_eq!(lockout.unwrap(), 30);

    let check = limiter.check_allowed(client_id);
    assert!(check.is_err());
}

#[test]
fn test_auth_rate_limiter_exponential_backoff() {
    let limiter = AuthRateLimiter::new(2, 60, 10);
    let client_id = "test_client_3";

    limiter.record_failure(client_id);
    let lockout1 = limiter.record_failure(client_id);
    assert_eq!(lockout1.unwrap(), 10);

    let client_id2 = "test_client_3b";
    limiter.record_failure(client_id2);
    limiter.record_failure(client_id2);
}

#[test]
fn test_auth_rate_limiter_success_resets_failure_count() {
    let limiter = AuthRateLimiter::new(3, 60, 30);
    let client_id = "test_client_4";

    limiter.record_failure(client_id);
    limiter.record_failure(client_id);

    limiter.record_success(client_id);

    assert!(limiter.record_failure(client_id).is_none());
    assert!(limiter.record_failure(client_id).is_none());
}

#[test]
fn test_auth_rate_limiter_different_clients_independent() {
    let limiter = AuthRateLimiter::new(2, 60, 30);
    let client_a = "client_a";
    let client_b = "client_b";

    limiter.record_failure(client_a);
    limiter.record_failure(client_a);

    assert!(limiter.check_allowed(client_a).is_err());
    assert!(limiter.check_allowed(client_b).is_ok());
}

#[test]
fn test_auth_rate_limiter_blocked_count() {
    let limiter = AuthRateLimiter::new(1, 60, 30);
    let client_id = "test_client_5";

    limiter.record_failure(client_id);

    assert_eq!(limiter.blocked_attempts(), 0);

    let _ = limiter.check_allowed(client_id);

    assert_eq!(limiter.blocked_attempts(), 1);

    let _ = limiter.check_allowed(client_id);
    assert_eq!(limiter.blocked_attempts(), 2);
}
