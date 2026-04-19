//! Middleware tests

use super::auth_rate_limiter::AuthRateLimiter;
use super::helpers::{
    check_admin_authorization, extract_auth_method, extract_auth_method_with_api_key_header,
    is_admin_route, is_api_route, is_public_route,
};
use crate::auth::AuthMethod;
use crate::core::models::user::preferences::UserPreferences;
use crate::core::models::user::types::{User, UserProfile, UserRole, UserStatus};
use crate::core::models::{ApiKey, Metadata, UsageStats};
use actix_web::http::header::{HeaderMap, HeaderName, HeaderValue};

fn make_user(role: UserRole) -> User {
    User {
        metadata: Metadata::new(),
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        display_name: None,
        password_hash: "hash".to_string(),
        role,
        status: UserStatus::Active,
        team_ids: vec![],
        preferences: UserPreferences::default(),
        usage_stats: UsageStats::default(),
        rate_limits: None,
        last_login_at: None,
        email_verified: true,
        two_factor_enabled: false,
        profile: UserProfile::default(),
    }
}

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

// These tests exercise the authorization decision that the production auth
// middleware runs (check_admin_authorization), not just the route classifier.
#[test]
fn test_check_admin_authorization_non_admin_paths_always_pass() {
    let user = make_user(UserRole::User);
    assert!(check_admin_authorization("/v1/chat/completions", Some(&user), None));
    assert!(check_admin_authorization("/health", Some(&user), None));
    assert!(check_admin_authorization("/v1/chat/completions", None, None));
}

#[test]
fn test_check_admin_authorization_admin_path_with_admin_user() {
    let admin = make_user(UserRole::Admin);
    let super_admin = make_user(UserRole::SuperAdmin);
    assert!(check_admin_authorization("/admin/users", Some(&admin), None));
    assert!(check_admin_authorization("/api/admin/config", Some(&admin), None));
    assert!(check_admin_authorization("/admin", Some(&super_admin), None));
}

#[test]
fn test_check_admin_authorization_admin_path_with_non_admin_user() {
    let user = make_user(UserRole::User);
    let manager = make_user(UserRole::Manager);
    let viewer = make_user(UserRole::Viewer);
    assert!(!check_admin_authorization("/admin/users", Some(&user), None));
    assert!(!check_admin_authorization("/api/admin/config", Some(&manager), None));
    assert!(!check_admin_authorization("/admin/settings", Some(&viewer), None));
}

#[test]
fn test_check_admin_authorization_admin_path_no_user_no_key() {
    // Neither user nor API key → denied.
    assert!(!check_admin_authorization("/admin/users", None, None));
    assert!(!check_admin_authorization("/api/admin/config", None, None));
}

#[test]
fn test_check_admin_authorization_admin_path_admin_api_key_no_user() {
    // API key with "*" or "system.admin" grants access even without a user.
    let mut key = ApiKey {
        metadata: Metadata::new(),
        name: "admin-key".to_string(),
        key_hash: "h".to_string(),
        key_prefix: "gw-".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec!["*".to_string()],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };
    assert!(check_admin_authorization("/admin/users", None, Some(&key)));
    assert!(check_admin_authorization("/api/admin/config", None, Some(&key)));

    key.permissions = vec!["system.admin".to_string()];
    assert!(check_admin_authorization("/admin/users", None, Some(&key)));
}

#[test]
fn test_check_admin_authorization_admin_path_non_admin_api_key_no_user() {
    // API key with only "use:api" is insufficient for admin routes.
    let key = ApiKey {
        metadata: Metadata::new(),
        name: "regular-key".to_string(),
        key_hash: "h".to_string(),
        key_prefix: "gw-".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec!["use:api".to_string()],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };
    assert!(!check_admin_authorization("/admin/users", None, Some(&key)));
}

#[test]
fn test_check_admin_authorization_prefix_confusion_not_blocked() {
    // /adminevil is not an admin route; any caller should pass.
    let user = make_user(UserRole::User);
    assert!(check_admin_authorization("/adminevil", Some(&user), None));
    assert!(check_admin_authorization("/administrator", Some(&user), None));
    assert!(check_admin_authorization("/adminevil", None, None));
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
