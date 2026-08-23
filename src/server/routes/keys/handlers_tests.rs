use super::super::access::{
    check_ownership, filter_and_paginate_keys, resolve_create_key_scope,
    validate_update_key_permissions,
};
use super::*;
use crate::auth::AuthResult;
use crate::core::keys::{KeyInfo, KeyPermissions, KeyRateLimits, KeyUsageStats};
use crate::core::models::user::types::{User, UserRole};
use crate::core::types::context::RequestContext;
use chrono::Utc;

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
fn test_verify_key_access_rejects_unrelated_authenticated_user() {
    let auth = make_user_auth(make_user(UserRole::User, vec![]));
    let mut key_info = make_key_info(Uuid::new_v4(), KeyStatus::Active);
    key_info.user_id = Some(Uuid::new_v4());

    assert!(!verify_key_access_allowed(&auth, &key_info));
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
