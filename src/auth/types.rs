//! Authentication and authorization types

use crate::core::models::ApiKey;
use crate::core::models::user::session::UserSession;
use crate::core::models::user::types::User;
use crate::core::types::context::RequestContext;

/// Authentication result
#[derive(Debug, Clone)]
pub struct AuthResult {
    /// Whether authentication was successful
    pub success: bool,
    /// Authenticated user (if any)
    pub user: Option<User>,
    /// API key used (if any)
    pub api_key: Option<ApiKey>,
    /// Session information (if any)
    pub session: Option<UserSession>,
    /// Error message (if authentication failed)
    pub error: Option<String>,
    /// Request context
    pub context: RequestContext,
}

/// Authorization result
#[derive(Debug, Clone)]
pub struct AuthzResult {
    /// Whether authorization was successful
    pub allowed: bool,
    /// Required permissions that were checked
    pub required_permissions: Vec<String>,
    /// User's actual permissions
    pub user_permissions: Vec<String>,
    /// Reason for denial (if not allowed)
    pub reason: Option<String>,
}

/// Authentication method
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// JWT token authentication
    Jwt(String),
    /// API key authentication
    ApiKey(String),
    /// Session-based authentication
    Session(String),
    /// No authentication
    None,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::Metadata;
    use crate::core::models::user::preferences::UserPreferences;
    use crate::core::models::user::session::SessionType;
    use crate::core::models::user::types::{UserProfile, UserRole, UserStatus};
    use std::collections::HashMap;

    // ==================== Helper Functions ====================

    fn create_test_request_context() -> RequestContext {
        RequestContext::new()
    }

    fn create_test_user() -> User {
        User {
            metadata: Metadata::new(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            display_name: Some("Test User".to_string()),
            password_hash: "hash".to_string(),
            role: UserRole::User,
            status: UserStatus::Active,
            team_ids: vec![],
            preferences: UserPreferences::default(),
            usage_stats: crate::core::models::UsageStats::default(),
            rate_limits: None,
            last_login_at: None,
            email_verified: true,
            two_factor_enabled: false,
            profile: UserProfile::default(),
        }
    }

    fn create_test_api_key() -> ApiKey {
        ApiKey {
            metadata: Metadata::new(),
            name: "Test Key".to_string(),
            key_hash: "hash123".to_string(),
            key_prefix: "sk-test".to_string(),
            user_id: None,
            team_id: None,
            permissions: vec!["api:read".to_string()],
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: crate::core::models::UsageStats::default(),
        }
    }

    fn create_test_session() -> UserSession {
        UserSession {
            metadata: Metadata::new(),
            user_id: uuid::Uuid::new_v4(),
            token: "test-token".to_string(),
            session_type: SessionType::Web,
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("Test Agent".to_string()),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            last_activity: chrono::Utc::now(),
            data: HashMap::new(),
        }
    }

    // ==================== AuthResult Tests ====================

    #[test]
    fn test_auth_result_success() {
        let result = AuthResult {
            success: true,
            user: Some(create_test_user()),
            api_key: None,
            session: None,
            error: None,
            context: create_test_request_context(),
        };

        assert!(result.success);
        assert!(result.user.is_some());
        assert!(result.error.is_none());
    }

    #[test]
    fn test_auth_result_failure() {
        let result = AuthResult {
            success: false,
            user: None,
            api_key: None,
            session: None,
            error: Some("Invalid credentials".to_string()),
            context: create_test_request_context(),
        };

        assert!(!result.success);
        assert!(result.user.is_none());
        assert_eq!(result.error, Some("Invalid credentials".to_string()));
    }

    #[test]
    fn test_auth_result_with_api_key() {
        let result = AuthResult {
            success: true,
            user: None,
            api_key: Some(create_test_api_key()),
            session: None,
            error: None,
            context: create_test_request_context(),
        };

        assert!(result.success);
        assert!(result.api_key.is_some());
        assert_eq!(result.api_key.as_ref().unwrap().name, "Test Key");
    }

    #[test]
    fn test_auth_result_with_user_and_api_key() {
        let result = AuthResult {
            success: true,
            user: Some(create_test_user()),
            api_key: Some(create_test_api_key()),
            session: None,
            error: None,
            context: create_test_request_context(),
        };

        assert!(result.success);
        assert!(result.user.is_some());
        assert!(result.api_key.is_some());
    }

    #[test]
    fn test_auth_result_clone() {
        let result = AuthResult {
            success: true,
            user: Some(create_test_user()),
            api_key: None,
            session: None,
            error: None,
            context: create_test_request_context(),
        };

        let cloned = result.clone();
        assert_eq!(cloned.success, result.success);
        assert!(cloned.user.is_some());
    }

    #[test]
    fn test_auth_result_debug() {
        let result = AuthResult {
            success: true,
            user: None,
            api_key: None,
            session: None,
            error: None,
            context: create_test_request_context(),
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("AuthResult"));
        assert!(debug_str.contains("success: true"));
    }

    // ==================== AuthzResult Tests ====================

    #[test]
    fn test_authz_result_allowed() {
        let result = AuthzResult {
            allowed: true,
            required_permissions: vec!["read".to_string()],
            user_permissions: vec!["read".to_string(), "write".to_string()],
            reason: None,
        };

        assert!(result.allowed);
        assert!(result.reason.is_none());
    }

    #[test]
    fn test_authz_result_denied() {
        let result = AuthzResult {
            allowed: false,
            required_permissions: vec!["admin".to_string()],
            user_permissions: vec!["read".to_string()],
            reason: Some("Missing admin permission".to_string()),
        };

        assert!(!result.allowed);
        assert_eq!(result.reason, Some("Missing admin permission".to_string()));
    }

    #[test]
    fn test_authz_result_empty_permissions() {
        let result = AuthzResult {
            allowed: true,
            required_permissions: vec![],
            user_permissions: vec![],
            reason: None,
        };

        assert!(result.allowed);
        assert!(result.required_permissions.is_empty());
    }

    #[test]
    fn test_authz_result_multiple_permissions() {
        let result = AuthzResult {
            allowed: true,
            required_permissions: vec!["read".to_string(), "write".to_string()],
            user_permissions: vec![
                "read".to_string(),
                "write".to_string(),
                "delete".to_string(),
            ],
            reason: None,
        };

        assert!(result.allowed);
        assert_eq!(result.required_permissions.len(), 2);
        assert_eq!(result.user_permissions.len(), 3);
    }

    #[test]
    fn test_authz_result_clone() {
        let result = AuthzResult {
            allowed: true,
            required_permissions: vec!["test".to_string()],
            user_permissions: vec!["test".to_string()],
            reason: None,
        };

        let cloned = result.clone();
        assert_eq!(cloned.allowed, result.allowed);
        assert_eq!(cloned.required_permissions, result.required_permissions);
    }

    #[test]
    fn test_authz_result_debug() {
        let result = AuthzResult {
            allowed: false,
            required_permissions: vec![],
            user_permissions: vec![],
            reason: Some("Denied".to_string()),
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("AuthzResult"));
        assert!(debug_str.contains("allowed: false"));
    }

    // ==================== AuthMethod Tests ====================

    #[test]
    fn test_auth_method_jwt() {
        let method = AuthMethod::Jwt("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9".to_string());

        if let AuthMethod::Jwt(token) = method {
            assert!(token.starts_with("eyJ"));
        } else {
            panic!("Expected Jwt variant");
        }
    }

    #[test]
    fn test_auth_method_api_key() {
        let method = AuthMethod::ApiKey("sk-test-12345".to_string());

        if let AuthMethod::ApiKey(key) = method {
            assert!(key.starts_with("sk-"));
        } else {
            panic!("Expected ApiKey variant");
        }
    }

    #[test]
    fn test_auth_method_session() {
        let method = AuthMethod::Session("session-abc123".to_string());

        if let AuthMethod::Session(session_id) = method {
            assert!(session_id.starts_with("session-"));
        } else {
            panic!("Expected Session variant");
        }
    }

    #[test]
    fn test_auth_method_none() {
        let method = AuthMethod::None;
        assert!(matches!(method, AuthMethod::None));
    }

    #[test]
    fn test_auth_method_clone() {
        let method = AuthMethod::Jwt("token".to_string());
        let cloned = method.clone();

        if let (AuthMethod::Jwt(a), AuthMethod::Jwt(b)) = (method, cloned) {
            assert_eq!(a, b);
        } else {
            panic!("Clone failed");
        }
    }

    #[test]
    fn test_auth_method_debug() {
        let method = AuthMethod::ApiKey("secret".to_string());
        let debug_str = format!("{:?}", method);

        assert!(debug_str.contains("ApiKey"));
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_auth_flow_simulation() {
        // Simulate successful authentication
        let auth_result = AuthResult {
            success: true,
            user: Some(create_test_user()),
            api_key: None,
            session: None,
            error: None,
            context: create_test_request_context(),
        };

        assert!(auth_result.success);

        // Simulate authorization check based on user role
        let user = auth_result.user.as_ref().unwrap();
        let authz_result = AuthzResult {
            allowed: matches!(
                user.role,
                UserRole::User | UserRole::Admin | UserRole::SuperAdmin
            ),
            required_permissions: vec!["read".to_string()],
            user_permissions: vec!["read".to_string()],
            reason: None,
        };

        assert!(authz_result.allowed);
    }

    #[test]
    fn test_auth_method_extraction() {
        let methods = [
            AuthMethod::Jwt("jwt-token".to_string()),
            AuthMethod::ApiKey("api-key".to_string()),
            AuthMethod::Session("session-id".to_string()),
            AuthMethod::None,
        ];

        let method_types: Vec<&str> = methods
            .iter()
            .map(|m| match m {
                AuthMethod::Jwt(_) => "jwt",
                AuthMethod::ApiKey(_) => "api_key",
                AuthMethod::Session(_) => "session",
                AuthMethod::None => "none",
            })
            .collect();

        assert_eq!(method_types, vec!["jwt", "api_key", "session", "none"]);
    }

    #[test]
    fn test_permission_check_simulation() {
        let user_permissions = ["read".to_string(), "write".to_string()];
        let required_permissions = ["read".to_string(), "delete".to_string()];

        let has_all = required_permissions
            .iter()
            .all(|p| user_permissions.contains(p));

        let missing: Vec<_> = required_permissions
            .iter()
            .filter(|p| !user_permissions.contains(p))
            .collect();

        assert!(!has_all);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "delete");
    }

    #[test]
    fn test_auth_result_with_all_fields() {
        let user = create_test_user();
        let api_key = create_test_api_key();
        let session = create_test_session();

        let result = AuthResult {
            success: true,
            user: Some(user),
            api_key: Some(api_key),
            session: Some(session),
            error: None,
            context: create_test_request_context(),
        };

        assert!(result.success);
        assert!(result.user.is_some());
        assert!(result.api_key.is_some());
        assert!(result.session.is_some());
    }

    #[test]
    fn test_auth_result_with_session() {
        let result = AuthResult {
            success: true,
            user: None,
            api_key: None,
            session: Some(create_test_session()),
            error: None,
            context: create_test_request_context(),
        };

        assert!(result.success);
        assert!(result.session.is_some());
        assert!(result.session.as_ref().unwrap().ip_address.is_some());
    }

    #[test]
    fn test_authz_result_with_reason() {
        let result = AuthzResult {
            allowed: false,
            required_permissions: vec!["admin:write".to_string()],
            user_permissions: vec!["user:read".to_string()],
            reason: Some("Insufficient permissions for admin operations".to_string()),
        };

        assert!(!result.allowed);
        assert!(result.reason.is_some());
        assert!(result.reason.as_ref().unwrap().contains("admin"));
    }
}
