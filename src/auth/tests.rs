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

#[tokio::test]
async fn jwt_canonical_user_conversion_error_propagates_from_auth_system() {
    use crate::core::models::user::types::{User, UserStatus};
    use crate::storage::database::entities::user;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};

    let mut config = crate::config::Config::default();
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;

    let storage = std::sync::Arc::new(
        crate::storage::StorageLayer::new(&config.gateway.storage)
            .await
            .expect("storage should initialize"),
    );
    let mut persisted = User::new(
        "jwt-corrupt-user".to_string(),
        "jwt-corrupt@example.com".to_string(),
        "secret-hash".to_string(),
    );
    persisted.status = UserStatus::Active;
    storage.db().create_user(&persisted).await.unwrap();

    let auth_system = super::system::AuthSystem::new(&config.gateway.auth, storage.clone())
        .await
        .expect("AuthSystem should initialize");
    let token = auth_system
        .jwt()
        .create_access_token(persisted.id(), "user".to_string(), vec![], None, None)
        .await
        .unwrap();

    user::ActiveModel {
        id: Set(persisted.id()),
        role: Set("sentinel-invalid-jwt-role".to_string()),
        ..Default::default()
    }
    .update(storage.db().connection())
    .await
    .expect("test should corrupt persisted role");

    let error = auth_system
        .authenticate(AuthMethod::Jwt(token.clone()), RequestContext::new())
        .await
        .expect_err("canonical conversion error must propagate from JWT authentication");

    let rendered = error.to_string();
    assert!(rendered.contains("role"));
    assert!(!rendered.contains("sentinel-invalid-jwt-role"));
    assert!(!rendered.contains("jwt-corrupt-user"));
    assert!(!rendered.contains("jwt-corrupt@example.com"));
    assert!(!rendered.contains("secret-hash"));
    assert!(!rendered.contains(&token));

    storage
        .db()
        .delete_user(&persisted.id().to_string())
        .await
        .expect("test should delete the legacy mirror");
    user::Entity::delete_by_id(persisted.id())
        .exec(storage.db().connection())
        .await
        .expect("test should delete the canonical user");

    let missing = auth_system
        .authenticate(AuthMethod::Jwt(token), RequestContext::new())
        .await
        .expect("a genuinely missing user remains an authentication result");
    assert!(!missing.success);
    assert_eq!(missing.error.as_deref(), Some("User not found"));
}

#[tokio::test]
async fn gh1130_active_team_proof_requires_active_team_and_exact_member() {
    use crate::core::models::team::{Team, TeamMember, TeamRole};
    use crate::core::models::user::types::{User, UserStatus};
    use crate::core::teams::TeamRepository;
    use crate::storage::database::SeaOrmTeamRepository;
    use uuid::Uuid;

    let mut config = crate::config::Config::default();
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;

    let storage = std::sync::Arc::new(
        crate::storage::StorageLayer::new(&config.gateway.storage)
            .await
            .expect("storage should initialize"),
    );
    let mut user = User::new(
        "team-proof-user".to_string(),
        "team-proof@example.com".to_string(),
        "password-hash".to_string(),
    );
    user.status = UserStatus::Active;
    storage.db().create_user(&user).await.unwrap();

    let repository = SeaOrmTeamRepository::new(storage.database.clone());
    let team = repository
        .create(Team::new("team-proof".to_string(), None))
        .await
        .unwrap();
    repository
        .add_member(TeamMember::new(
            team.id(),
            user.id(),
            TeamRole::Member,
            None,
        ))
        .await
        .unwrap();

    let auth_system = super::system::AuthSystem::new(&config.gateway.auth, storage)
        .await
        .unwrap();
    let proof = auth_system
        .validate_active_team(user.id(), team.id())
        .await
        .unwrap()
        .expect("active exact membership should create proof");
    assert!(proof.matches_user(user.id()));
    assert_eq!(proof.team_id(), team.id());
    assert!(
        auth_system
            .validate_active_team(Uuid::new_v4(), team.id())
            .await
            .unwrap()
            .is_none()
    );

    repository
        .remove_member(team.id(), user.id())
        .await
        .unwrap();
    assert!(
        auth_system
            .validate_active_team(user.id(), team.id())
            .await
            .unwrap()
            .is_none()
    );
}
