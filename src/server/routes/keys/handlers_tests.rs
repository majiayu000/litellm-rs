use super::super::access::{
    auth_result_from_request_extensions, check_auth_result_ownership, check_ownership,
    filter_and_paginate_keys, resolve_create_key_scope, validate_update_key_permissions,
};
use super::*;
use crate::auth::AuthResult;
use crate::core::keys::{KeyInfo, KeyPermissions, KeyRateLimits, KeyUsageStats};
use crate::core::models::user::types::{User, UserRole};
use crate::core::models::{ApiKey, Metadata, UsageStats};
use crate::core::types::context::{RequestContext, SharedRequestContext};
use actix_web::{HttpMessage, body::to_bytes, http::StatusCode, test as actix_test, web};
use chrono::Utc;
use std::sync::Arc;

#[test]
fn test_create_key_config_from_request() {
    let request = CreateKeyRequest {
        name: "Test Key".to_string(),
        description: Some("A test".to_string()),
        user_id: None,
        team_id: None,
        budget_id: None,
        max_budget: None,
        permissions: None,
        rate_limits: None,
        expires_at: None,
        metadata: None,
    };

    let config = CreateKeyConfig {
        name: request.name.clone(),
        description: request.description.clone(),
        user_id: request.user_id,
        team_id: request.team_id,
        budget_id: request.budget_id,
        permissions: request.permissions.clone().unwrap_or_default(),
        rate_limits: request.rate_limits.clone().unwrap_or_default(),
        expires_at: request.expires_at,
        metadata: request.metadata.clone().unwrap_or(serde_json::Value::Null),
    };

    assert_eq!(config.name, "Test Key");
    assert!(config.description.is_some());
}

fn make_user(role: UserRole, team_ids: Vec<Uuid>) -> User {
    use crate::core::models::Metadata;
    use crate::core::models::UsageStats;
    use crate::core::models::user::preferences::UserPreferences;
    use crate::core::models::user::types::{UserProfile, UserStatus};
    User {
        metadata: Metadata::new(),
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        display_name: None,
        password_hash: "hash".to_string(),
        role,
        status: UserStatus::Active,
        team_ids,
        preferences: UserPreferences::default(),
        usage_stats: UsageStats::default(),
        rate_limits: None,
        last_login_at: None,
        email_verified: true,
        two_factor_enabled: false,
        profile: UserProfile::default(),
    }
}

fn make_user_auth(user: User) -> AuthResult {
    AuthResult {
        success: true,
        user: Some(user),
        api_key: None,
        session: None,
        error: None,
        context: RequestContext::new(),
    }
}

fn make_team_auth(team_id: Uuid) -> AuthResult {
    let mut context = RequestContext::new();
    context.set_team_id(team_id);
    AuthResult {
        success: true,
        user: None,
        api_key: None,
        session: None,
        error: None,
        context,
    }
}

fn make_api_key(id: Uuid, user_id: Option<Uuid>, team_id: Option<Uuid>) -> ApiKey {
    ApiKey {
        metadata: Metadata {
            id,
            ..Metadata::new()
        },
        name: "caller-key".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-caller".to_string(),
        user_id,
        team_id,
        permissions: Vec::new(),
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    }
}

async fn auth_enabled_test_state() -> web::Data<AppState> {
    let mut config = crate::config::Config::default();
    config.gateway.auth.enable_jwt = true;
    config.gateway.auth.enable_api_key = true;
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
    let server = crate::server::http::HttpServer::new(&config)
        .await
        .expect("key-route test server should initialize");
    web::Data::new(server.state().clone())
}

fn request_with_auth_extensions(auth: &AuthResult) -> HttpRequest {
    let request = actix_test::TestRequest::default().to_http_request();
    request
        .extensions_mut()
        .insert::<SharedRequestContext>(Arc::new(auth.context.clone()));
    if let Some(user) = auth.user.clone() {
        request.extensions_mut().insert::<User>(user);
    }
    if let Some(api_key) = auth.api_key.clone() {
        request.extensions_mut().insert::<ApiKey>(api_key);
    }
    request
}

fn create_request_with_permissions(permissions: Option<KeyPermissions>) -> CreateKeyRequest {
    CreateKeyRequest {
        name: "scoped".to_string(),
        description: None,
        user_id: None,
        team_id: None,
        budget_id: None,
        max_budget: None,
        permissions,
        rate_limits: None,
        expires_at: None,
        metadata: None,
    }
}

fn update_request_with_permissions(permissions: Option<KeyPermissions>) -> UpdateKeyRequest {
    UpdateKeyRequest {
        name: None,
        description: None,
        permissions,
        rate_limits: None,
        budget_id: None,
        max_budget: None,
        expires_at: None,
        metadata: None,
    }
}

#[test]
fn key_usage_response_serializes_unpriced_fields() {
    let key_id = Uuid::new_v4();
    let mut usage = KeyUsageStats::new();
    usage.record_usage_record(&crate::core::keys::UsageRecord::unpriced(
        25,
        0.01,
        "allow_unpriced",
    ));
    let response = KeyUsageResponse { key_id, usage };

    let value = serde_json::to_value(response).expect("key usage response should serialize");

    assert_eq!(value["key_id"], key_id.to_string());
    assert_eq!(value["usage"]["unpriced_requests"], 1);
    assert_eq!(value["usage"]["unpriced_tokens"], 25);
    assert_eq!(value["usage"]["unpriced_cost"], 0.01);
    assert!(value["usage"]["last_unpriced_at"].is_string());
}

fn make_key_info(id: Uuid, status: KeyStatus) -> KeyInfo {
    KeyInfo {
        id,
        key_prefix: "gw-test".to_string(),
        name: format!("key-{id}"),
        description: None,
        user_id: None,
        team_id: None,
        budget_id: None,
        status,
        permissions: KeyPermissions::default(),
        rate_limits: KeyRateLimits::default(),
        expires_at: None,
        created_at: Utc::now(),
        last_used_at: None,
        usage_stats: KeyUsageStats::default(),
    }
}

#[test]
fn test_check_ownership_admin_bypasses() {
    let admin = make_user(UserRole::Admin, vec![]);
    let other_user_id = Uuid::new_v4();
    assert!(check_ownership(&admin, Some(other_user_id), None));
    assert!(check_ownership(&admin, None, None));
    assert!(check_ownership(&admin, None, Some(Uuid::new_v4())));
}

#[test]
fn test_check_ownership_super_admin_bypasses() {
    let super_admin = make_user(UserRole::SuperAdmin, vec![]);
    let other_user_id = Uuid::new_v4();
    assert!(check_ownership(&super_admin, Some(other_user_id), None));
    assert!(check_ownership(&super_admin, None, None));
}

#[test]
fn test_check_ownership_user_owns_key() {
    let user = make_user(UserRole::User, vec![]);
    let user_id = user.id();
    assert!(check_ownership(&user, Some(user_id), None));
    assert!(!check_ownership(&user, Some(Uuid::new_v4()), None));
    assert!(!check_ownership(&user, None, None));
}

#[test]
fn test_check_ownership_manager_team_access() {
    let team_id = Uuid::new_v4();
    let other_team = Uuid::new_v4();
    let manager = make_user(UserRole::Manager, vec![team_id]);

    assert!(check_ownership(&manager, None, Some(team_id)));
    assert!(!check_ownership(&manager, None, Some(other_team)));
    assert!(!check_ownership(&manager, Some(Uuid::new_v4()), None));
}

#[test]
fn test_check_ownership_regular_user_cannot_access_team_key() {
    let team_id = Uuid::new_v4();
    let user = make_user(UserRole::User, vec![team_id]);
    assert!(!check_ownership(&user, None, Some(team_id)));
}

#[test]
fn test_is_auth_enabled_logic() {
    let regular_user = make_user(UserRole::User, vec![]);
    assert!(!check_ownership(&regular_user, None, None));
}

#[test]
fn test_resolve_create_key_scope_user_defaults_to_self() {
    let user = make_user(UserRole::User, vec![]);
    let user_id = user.id();
    let auth = make_user_auth(user);
    let request = CreateKeyRequest {
        name: "self".to_string(),
        description: None,
        user_id: None,
        team_id: None,
        budget_id: None,
        max_budget: None,
        permissions: None,
        rate_limits: None,
        expires_at: None,
        metadata: None,
    };

    let resolved = resolve_create_key_scope(&auth, &request).unwrap();
    assert_eq!(resolved, (Some(user_id), None));
}

#[test]
fn test_resolve_create_key_scope_user_cannot_target_other_user() {
    let user = make_user(UserRole::User, vec![]);
    let auth = make_user_auth(user);
    let request = CreateKeyRequest {
        name: "other".to_string(),
        description: None,
        user_id: Some(Uuid::new_v4()),
        team_id: None,
        budget_id: None,
        max_budget: None,
        permissions: None,
        rate_limits: None,
        expires_at: None,
        metadata: None,
    };

    assert!(resolve_create_key_scope(&auth, &request).is_err());
}

#[test]
fn test_resolve_create_key_scope_user_cannot_create_admin_key() {
    let user = make_user(UserRole::User, vec![]);
    let auth = make_user_auth(user);
    let request = create_request_with_permissions(Some(KeyPermissions::admin()));

    assert_eq!(
        resolve_create_key_scope(&auth, &request),
        Err("Only admin can create API keys with management permissions")
    );
}

#[test]
fn test_resolve_create_key_scope_user_cannot_create_system_admin_custom_permission() {
    let user = make_user(UserRole::User, vec![]);
    let auth = make_user_auth(user);
    let permissions = KeyPermissions {
        custom_permissions: vec!["system.admin".to_string()],
        ..Default::default()
    };
    let request = create_request_with_permissions(Some(permissions));

    assert_eq!(
        resolve_create_key_scope(&auth, &request),
        Err("Only admin can create API keys with management permissions")
    );
}

#[test]
fn test_resolve_create_key_scope_team_api_key_cannot_create_management_permission() {
    let team_id = Uuid::new_v4();
    let auth = make_team_auth(team_id);
    let permissions = KeyPermissions {
        custom_permissions: vec!["users.manage".to_string()],
        ..Default::default()
    };
    let request = create_request_with_permissions(Some(permissions));

    assert_eq!(
        resolve_create_key_scope(&auth, &request),
        Err("Team-scoped API keys cannot create API keys with management permissions")
    );
}

#[test]
fn test_resolve_create_key_scope_admin_can_create_management_key() {
    let admin = make_user(UserRole::Admin, vec![]);
    let auth = make_user_auth(admin);
    let request = create_request_with_permissions(Some(KeyPermissions::admin()));

    assert!(resolve_create_key_scope(&auth, &request).is_ok());
}

#[test]
fn test_resolve_create_key_scope_manager_can_target_own_team() {
    let team_id = Uuid::new_v4();
    let manager = make_user(UserRole::Manager, vec![team_id]);
    let auth = make_user_auth(manager);
    let request = CreateKeyRequest {
        name: "team".to_string(),
        description: None,
        user_id: None,
        team_id: Some(team_id),
        budget_id: None,
        max_budget: None,
        permissions: None,
        rate_limits: None,
        expires_at: None,
        metadata: None,
    };

    let resolved = resolve_create_key_scope(&auth, &request).unwrap();
    assert_eq!(resolved, (None, Some(team_id)));
}

#[test]
fn test_resolve_create_key_scope_team_api_key_defaults_to_team_scope() {
    let team_id = Uuid::new_v4();
    let auth = make_team_auth(team_id);
    let request = CreateKeyRequest {
        name: "team-api".to_string(),
        description: None,
        user_id: None,
        team_id: None,
        budget_id: None,
        max_budget: None,
        permissions: None,
        rate_limits: None,
        expires_at: None,
        metadata: None,
    };

    let resolved = resolve_create_key_scope(&auth, &request).unwrap();
    assert_eq!(resolved, (None, Some(team_id)));
}

#[test]
fn test_validate_update_key_permissions_user_cannot_promote_to_admin() {
    let user = make_user(UserRole::User, vec![]);
    let auth = make_user_auth(user);
    let request = update_request_with_permissions(Some(KeyPermissions::admin()));

    assert_eq!(
        validate_update_key_permissions(Some(&auth), &request),
        Err("Only admin can update API keys with management permissions")
    );
}

#[test]
fn test_validate_update_key_permissions_manager_cannot_grant_system_admin() {
    let team_id = Uuid::new_v4();
    let manager = make_user(UserRole::Manager, vec![team_id]);
    let auth = make_user_auth(manager);
    let permissions = KeyPermissions {
        custom_permissions: vec!["system.admin".to_string()],
        ..Default::default()
    };
    let request = update_request_with_permissions(Some(permissions));

    assert_eq!(
        validate_update_key_permissions(Some(&auth), &request),
        Err("Only admin can update API keys with management permissions")
    );
}

#[test]
fn test_validate_update_key_permissions_team_api_key_cannot_grant_wildcard() {
    let auth = make_team_auth(Uuid::new_v4());
    let permissions = KeyPermissions {
        custom_permissions: vec!["*".to_string()],
        ..Default::default()
    };
    let request = update_request_with_permissions(Some(permissions));

    assert_eq!(
        validate_update_key_permissions(Some(&auth), &request),
        Err("Only admin can update API keys with management permissions")
    );
}

#[test]
fn test_validate_update_key_permissions_admin_can_grant_management_access() {
    let admin = make_user(UserRole::Admin, vec![]);
    let auth = make_user_auth(admin);
    let request = update_request_with_permissions(Some(KeyPermissions::admin()));

    assert!(validate_update_key_permissions(Some(&auth), &request).is_ok());
}

#[test]
fn test_validate_update_key_permissions_allows_non_management_permissions() {
    let user = make_user(UserRole::User, vec![]);
    let auth = make_user_auth(user);
    let permissions = KeyPermissions {
        custom_permissions: vec!["api.chat".to_string()],
        ..Default::default()
    };
    let request = update_request_with_permissions(Some(permissions));

    assert!(validate_update_key_permissions(Some(&auth), &request).is_ok());
}

#[test]
fn test_filter_and_paginate_keys_reports_filtered_total() {
    let first_active = Uuid::new_v4();
    let revoked = Uuid::new_v4();
    let second_active = Uuid::new_v4();
    let keys = vec![
        make_key_info(first_active, KeyStatus::Active),
        make_key_info(revoked, KeyStatus::Revoked),
        make_key_info(second_active, KeyStatus::Active),
    ];

    let (page, total) = filter_and_paginate_keys(keys, Some(KeyStatus::Active), 1, 1);

    assert_eq!(total, 2);
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].id, second_active);
}

#[test]
fn test_filter_and_paginate_keys_applies_limit_without_status_filter() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let third = Uuid::new_v4();
    let keys = vec![
        make_key_info(first, KeyStatus::Active),
        make_key_info(second, KeyStatus::Revoked),
        make_key_info(third, KeyStatus::Active),
    ];

    let (page, total) = filter_and_paginate_keys(keys, None, 2, 1);

    assert_eq!(total, 3);
    assert_eq!(
        page.iter().map(|key| key.id).collect::<Vec<_>>(),
        vec![second, third]
    );
}

#[test]
fn test_verify_key_access_allows_presenting_own_key_secret() {
    let key_id = Uuid::new_v4();
    let key_info = make_key_info(key_id, KeyStatus::Active);
    let mut context = RequestContext::new();
    context.set_api_key_id(key_id);
    let auth = AuthResult {
        success: true,
        user: None,
        api_key: None,
        session: None,
        error: None,
        context,
    };

    assert!(verify_key_access_allowed(&auth, &key_info));
}

#[test]
fn test_verify_key_access_rejects_api_key_caller_for_foreign_key() {
    let auth = make_team_auth(Uuid::new_v4());
    let mut key_info = make_key_info(Uuid::new_v4(), KeyStatus::Active);
    key_info.user_id = Some(Uuid::new_v4());

    assert!(!verify_key_access_allowed(&auth, &key_info));
}

#[test]
fn test_verify_key_access_allows_api_key_caller_in_same_team() {
    let team_id = Uuid::new_v4();
    let auth = make_team_auth(team_id);
    let mut key_info = make_key_info(Uuid::new_v4(), KeyStatus::Active);
    key_info.team_id = Some(team_id);

    assert!(verify_key_access_allowed(&auth, &key_info));
}

#[test]
fn test_verify_key_access_rejects_foreign_keys_for_every_status_like_unknown_key() {
    let auth = make_user_auth(make_user(UserRole::User, vec![]));
    let unknown_allowed = verify_unknown_key_access_allowed(&auth);

    assert!(!unknown_allowed);
    for status in [KeyStatus::Active, KeyStatus::Revoked, KeyStatus::Expired] {
        let mut key_info = make_key_info(Uuid::new_v4(), status);
        key_info.user_id = Some(Uuid::new_v4());

        assert_eq!(
            verify_key_access_allowed(&auth, &key_info),
            unknown_allowed,
            "foreign {status} key must authorize identically to a missing key"
        );
    }
}

#[test]
fn test_verify_key_access_allows_owner_and_admin() {
    let owner = make_user(UserRole::User, vec![]);
    let mut owned = make_key_info(Uuid::new_v4(), KeyStatus::Active);
    owned.user_id = Some(owner.id());
    assert!(verify_key_access_allowed(&make_user_auth(owner), &owned));

    let admin = make_user_auth(make_user(UserRole::Admin, vec![]));
    let mut foreign = make_key_info(Uuid::new_v4(), KeyStatus::Active);
    foreign.user_id = Some(Uuid::new_v4());
    assert!(verify_key_access_allowed(&admin, &foreign));
}

#[test]
fn test_verify_key_access_honors_existing_management_permissions() {
    let target = make_key_info(Uuid::new_v4(), KeyStatus::Active);

    for permission in ["*", "system.admin", "keys.list_all"] {
        let caller_id = Uuid::new_v4();
        let mut caller_key = make_api_key(caller_id, None, None);
        caller_key.permissions = vec![permission.to_string()];
        let mut context = RequestContext::new();
        context.set_api_key_id(caller_id);
        let auth = AuthResult {
            success: true,
            user: None,
            api_key: Some(caller_key),
            session: None,
            error: None,
            context,
        };

        assert!(
            verify_key_access_allowed(&auth, &target),
            "permission {permission} should grant foreign-key verification"
        );
        assert!(
            verify_unknown_key_access_allowed(&auth),
            "permission {permission} should grant missing-key verification"
        );
    }
}

#[test]
fn test_verify_limited_admin_api_key_cannot_probe_foreign_or_unknown_keys() {
    let admin = make_user(UserRole::Admin, vec![]);
    let caller_id = Uuid::new_v4();
    let mut caller_key = make_api_key(caller_id, Some(admin.id()), None);
    caller_key.permissions = vec!["api.chat".to_string()];
    let mut context = RequestContext::new();
    context.set_api_key_id(caller_id);
    let auth = AuthResult {
        success: true,
        user: Some(admin),
        api_key: Some(caller_key),
        session: None,
        error: None,
        context,
    };
    let mut foreign = make_key_info(Uuid::new_v4(), KeyStatus::Active);
    foreign.user_id = Some(Uuid::new_v4());

    assert!(!verify_key_access_allowed(&auth, &foreign));
    assert!(!verify_unknown_key_access_allowed(&auth));
}

#[test]
fn test_verify_key_access_preserves_api_key_team_when_user_is_loaded() {
    let team_id = Uuid::new_v4();
    let user = make_user(UserRole::User, vec![]);
    let caller_id = Uuid::new_v4();
    let caller_key = make_api_key(caller_id, Some(user.id()), Some(team_id));
    let mut context = RequestContext::new();
    context.set_api_key_id(caller_id);
    context.set_team_id(team_id);
    let auth = AuthResult {
        success: true,
        user: Some(user),
        api_key: Some(caller_key),
        session: None,
        error: None,
        context,
    };
    let mut target = make_key_info(Uuid::new_v4(), KeyStatus::Active);
    target.team_id = Some(team_id);

    assert!(verify_key_access_allowed(&auth, &target));
    assert!(
        !check_auth_result_ownership(&auth, target.user_id, target.team_id),
        "general key routes must retain user-first ownership semantics"
    );
}

#[test]
fn test_auth_result_reuses_middleware_extensions() {
    let team_id = Uuid::new_v4();
    let user = make_user(UserRole::User, vec![]);
    let caller_id = Uuid::new_v4();
    let caller_key = make_api_key(caller_id, Some(user.id()), Some(team_id));
    let mut context = RequestContext::new();
    context.set_api_key_id(caller_id);
    context.set_team_id(team_id);
    let expected = AuthResult {
        success: true,
        user: Some(user),
        api_key: Some(caller_key),
        session: None,
        error: None,
        context,
    };
    let request = request_with_auth_extensions(&expected);

    let actual = auth_result_from_request_extensions(&request)
        .expect("middleware extensions should reconstruct authentication");

    assert_eq!(
        actual.user.as_ref().map(User::id),
        expected.user.as_ref().map(User::id)
    );
    assert_eq!(
        actual.api_key.as_ref().map(|key| key.metadata.id),
        expected.api_key.as_ref().map(|key| key.metadata.id)
    );
    assert_eq!(actual.context.api_key_id(), Some(caller_id));
    assert_eq!(actual.context.team_id(), Some(team_id));
}

#[actix_web::test]
async fn verify_handler_reuses_middleware_auth_without_reauthenticating() {
    let state = auth_enabled_test_state().await;
    let (_, raw_key) = state
        .key_manager
        .generate_key(CreateKeyConfig {
            name: "target".to_string(),
            ..Default::default()
        })
        .await
        .expect("target key should be generated");
    let request = request_with_auth_extensions(&make_user_auth(make_user(UserRole::Admin, vec![])));

    let response = verify_key(request, state, web::Json(VerifyKeyRequest { key: raw_key }))
        .await
        .expect("verification handler should respond");

    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn verify_handler_hides_foreign_key_existence() {
    let state = auth_enabled_test_state().await;
    let (_, active_raw_key) = state
        .key_manager
        .generate_key(CreateKeyConfig {
            name: "active foreign target".to_string(),
            ..Default::default()
        })
        .await
        .expect("active foreign target should be generated");
    let (revoked_id, revoked_raw_key) = state
        .key_manager
        .generate_key(CreateKeyConfig {
            name: "revoked foreign target".to_string(),
            ..Default::default()
        })
        .await
        .expect("revoked foreign target should be generated");
    state
        .key_manager
        .revoke_key(revoked_id)
        .await
        .expect("foreign target should be revoked");
    let (expired_id, expired_raw_key) = state
        .key_manager
        .generate_key(CreateKeyConfig {
            name: "expired foreign target".to_string(),
            ..Default::default()
        })
        .await
        .expect("expired foreign target should be generated");
    state
        .key_manager
        .update_key(
            expired_id,
            UpdateKeyConfig {
                expires_at: Some(Some(Utc::now() - chrono::Duration::hours(1))),
                ..Default::default()
            },
        )
        .await
        .expect("foreign target should be expired");
    let caller = make_user_auth(make_user(UserRole::User, vec![]));
    let targets = [
        ("active", active_raw_key),
        ("revoked", revoked_raw_key),
        ("expired", expired_raw_key),
        ("missing", "gw-nonexistent-key-material".to_string()),
    ];
    let mut expected_body = None;

    for (kind, key) in targets {
        let response = verify_key(
            request_with_auth_extensions(&caller),
            state.clone(),
            web::Json(VerifyKeyRequest { key }),
        )
        .await
        .unwrap_or_else(|error| panic!("{kind} target verification should respond: {error}"));
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "{kind} target must be hidden"
        );
        let body = to_bytes(response.into_body())
            .await
            .unwrap_or_else(|error| panic!("{kind} target response body should render: {error}"));

        if let Some(expected) = expected_body.as_ref() {
            assert_eq!(&body, expected, "{kind} target response body must match");
        } else {
            expected_body = Some(body);
        }
    }
}
