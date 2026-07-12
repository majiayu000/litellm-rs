//! Tests for authentication module

#[cfg(test)]
use crate::auth::types::{AuthMethod, AuthResult, AuthzResult};
use crate::core::types::context::RequestContext;

#[test]
fn test_auth_result_creation() {
    let context = RequestContext::new();
    let result = AuthResult {
        success: true,
        user: None,
        api_key: None,
        session: None,
        error: None,
        context,
    };

    assert!(result.success);
    assert!(result.error.is_none());
}

#[test]
fn test_auth_result_failed() {
    let context = RequestContext::new();
    let result = AuthResult {
        success: false,
        user: None,
        api_key: None,
        session: None,
        error: Some("Authentication failed".to_string()),
        context,
    };

    assert!(!result.success);
    assert!(result.error.is_some());
    assert_eq!(result.error.unwrap(), "Authentication failed");
}

#[test]
fn test_authz_result_creation() {
    let result = AuthzResult {
        allowed: true,
        required_permissions: vec!["read".to_string()],
        user_permissions: vec!["read".to_string(), "write".to_string()],
        reason: None,
    };

    assert!(result.allowed);
    assert_eq!(result.required_permissions.len(), 1);
    assert_eq!(result.user_permissions.len(), 2);
}

#[test]
fn test_authz_result_denied() {
    let result = AuthzResult {
        allowed: false,
        required_permissions: vec!["admin".to_string()],
        user_permissions: vec!["read".to_string()],
        reason: Some("Insufficient permissions".to_string()),
    };

    assert!(!result.allowed);
    assert!(result.reason.is_some());
    assert_eq!(result.reason.unwrap(), "Insufficient permissions");
}

#[test]
fn test_authz_result_empty_permissions() {
    let result = AuthzResult {
        allowed: false,
        required_permissions: vec!["read".to_string()],
        user_permissions: vec![],
        reason: Some("No permissions".to_string()),
    };

    assert!(!result.allowed);
    assert!(result.user_permissions.is_empty());
}

#[test]
fn test_auth_method_variants() {
    let jwt_method = AuthMethod::Jwt("token".to_string());
    let api_key_method = AuthMethod::ApiKey("key".to_string());
    let session_method = AuthMethod::Session("session".to_string());
    let none_method = AuthMethod::None;

    assert!(matches!(jwt_method, AuthMethod::Jwt(_)));
    assert!(matches!(api_key_method, AuthMethod::ApiKey(_)));
    assert!(matches!(session_method, AuthMethod::Session(_)));
    assert!(matches!(none_method, AuthMethod::None));
}

#[test]
fn test_auth_method_jwt_extraction() {
    let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
    let method = AuthMethod::Jwt(token.to_string());

    if let AuthMethod::Jwt(extracted) = method {
        assert_eq!(extracted, token);
    } else {
        panic!("Expected Jwt variant");
    }
}

#[test]
fn test_auth_method_api_key_extraction() {
    let key = "sk-test-key-12345";
    let method = AuthMethod::ApiKey(key.to_string());

    if let AuthMethod::ApiKey(extracted) = method {
        assert_eq!(extracted, key);
    } else {
        panic!("Expected ApiKey variant");
    }
}

#[test]
fn test_auth_method_session_extraction() {
    let session_id = "session-uuid-12345";
    let method = AuthMethod::Session(session_id.to_string());

    if let AuthMethod::Session(extracted) = method {
        assert_eq!(extracted, session_id);
    } else {
        panic!("Expected Session variant");
    }
}

#[test]
fn test_auth_result_clone() {
    let context = RequestContext::new();
    let result = AuthResult {
        success: true,
        user: None,
        api_key: None,
        session: None,
        error: None,
        context,
    };

    let cloned = result.clone();
    assert_eq!(result.success, cloned.success);
}

#[test]
fn test_authz_result_clone() {
    let result = AuthzResult {
        allowed: true,
        required_permissions: vec!["read".to_string()],
        user_permissions: vec!["read".to_string()],
        reason: None,
    };

    let cloned = result.clone();
    assert_eq!(result.allowed, cloned.allowed);
    assert_eq!(result.required_permissions, cloned.required_permissions);
}

#[test]
fn test_auth_method_clone() {
    let method = AuthMethod::Jwt("token".to_string());
    let cloned = method.clone();

    if let (AuthMethod::Jwt(orig), AuthMethod::Jwt(cloned_token)) = (&method, &cloned) {
        assert_eq!(orig, cloned_token);
    } else {
        panic!("Clone failed");
    }
}

#[tokio::test]
async fn test_session_auth_always_rejected() {
    // Build a real AuthSystem via the same path the server uses,
    // then call authenticate(AuthMethod::Session(…)) and verify rejection.
    // This guards against regressions on issue #37 (JWT-as-session bypass).
    let mut config = crate::config::Config::default();
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;

    let storage = std::sync::Arc::new(
        crate::storage::StorageLayer::new(&config.gateway.storage)
            .await
            .expect("failed to create storage layer for session auth test"),
    );

    let auth_system = super::system::AuthSystem::new(&config.gateway.auth, storage)
        .await
        .expect("failed to create AuthSystem for session auth test");

    let context = RequestContext::new();
    let result = auth_system
        .authenticate(AuthMethod::Session("any-session-id".into()), context)
        .await
        .expect("authenticate() should not return Err for session auth");

    assert!(!result.success, "session auth must always be rejected");
    assert!(result.user.is_none(), "rejected session must not set user");
    assert_eq!(
        result.error.as_deref(),
        Some("Session authentication is not yet implemented"),
        "session auth error message must match expected value"
    );
}

#[tokio::test]
async fn api_key_storage_error_propagates_from_auth_system() {
    let mut config = crate::config::Config::default();
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;

    let storage = std::sync::Arc::new(
        crate::storage::StorageLayer::new(&config.gateway.storage)
            .await
            .expect("storage should initialize before the failure is injected"),
    );
    let auth_system = super::system::AuthSystem::new(&config.gateway.auth, storage.clone())
        .await
        .expect("AuthSystem should initialize before the failure is injected");
    storage
        .db()
        .connection()
        .close_by_ref()
        .await
        .expect("test should close the database pool");

    let error = auth_system
        .authenticate(
            AuthMethod::ApiKey("gw-infrastructure-failure".to_string()),
            RequestContext::new(),
        )
        .await
        .expect_err("database failure must not become an invalid-credential AuthResult");

    assert!(matches!(
        error,
        crate::utils::error::gateway_error::GatewayError::Storage(_)
    ));
}
