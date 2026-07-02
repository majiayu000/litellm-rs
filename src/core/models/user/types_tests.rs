use super::*;

fn create_test_user() -> User {
    User::new(
        "testuser".to_string(),
        "test@example.com".to_string(),
        "hashed_password_123".to_string(),
    )
}

// ==================== UserRole Tests ====================

#[test]
fn test_user_role_super_admin_serialize() {
    let role = UserRole::SuperAdmin;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"super_admin\"");
}

#[test]
fn test_user_role_admin_serialize() {
    let role = UserRole::Admin;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"admin\"");
}

#[test]
fn test_user_role_manager_serialize() {
    let role = UserRole::Manager;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"manager\"");
}

#[test]
fn test_user_role_user_serialize() {
    let role = UserRole::User;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"user\"");
}

#[test]
fn test_user_role_viewer_serialize() {
    let role = UserRole::Viewer;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"viewer\"");
}

#[test]
fn test_user_role_api_user_serialize() {
    let role = UserRole::ApiUser;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"api_user\"");
}

#[test]
fn test_user_role_deserialize() {
    let role: UserRole = serde_json::from_str("\"super_admin\"").unwrap();
    assert!(matches!(role, UserRole::SuperAdmin));

    let role: UserRole = serde_json::from_str("\"api_user\"").unwrap();
    assert!(matches!(role, UserRole::ApiUser));
}

#[test]
fn test_user_role_display() {
    assert_eq!(UserRole::SuperAdmin.to_string(), "super_admin");
    assert_eq!(UserRole::Admin.to_string(), "admin");
    assert_eq!(UserRole::Manager.to_string(), "manager");
    assert_eq!(UserRole::User.to_string(), "user");
    assert_eq!(UserRole::Viewer.to_string(), "viewer");
    assert_eq!(UserRole::ApiUser.to_string(), "api_user");
}

#[test]
fn test_user_role_from_str() {
    assert!(matches!(
        "super_admin".parse::<UserRole>().unwrap(),
        UserRole::SuperAdmin
    ));
    assert!(matches!(
        "admin".parse::<UserRole>().unwrap(),
        UserRole::Admin
    ));
    assert!(matches!(
        "manager".parse::<UserRole>().unwrap(),
        UserRole::Manager
    ));
    assert!(matches!(
        "user".parse::<UserRole>().unwrap(),
        UserRole::User
    ));
    assert!(matches!(
        "viewer".parse::<UserRole>().unwrap(),
        UserRole::Viewer
    ));
    assert!(matches!(
        "api_user".parse::<UserRole>().unwrap(),
        UserRole::ApiUser
    ));
}

#[test]
fn test_user_role_from_str_invalid() {
    let result = "invalid_role".parse::<UserRole>();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid user role"));
}

#[test]
fn test_user_role_equality() {
    assert_eq!(UserRole::Admin, UserRole::Admin);
    assert_ne!(UserRole::Admin, UserRole::User);
}

// ==================== UserStatus Tests ====================

#[test]
fn test_user_status_active() {
    let status = UserStatus::Active;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"active\"");
}

#[test]
fn test_user_status_inactive() {
    let status = UserStatus::Inactive;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"inactive\"");
}

#[test]
fn test_user_status_suspended() {
    let status = UserStatus::Suspended;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"suspended\"");
}

#[test]
fn test_user_status_pending() {
    let status = UserStatus::Pending;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"pending\"");
}

#[test]
fn test_user_status_deleted() {
    let status = UserStatus::Deleted;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"deleted\"");
}

#[test]
fn test_user_status_deserialize() {
    let status: UserStatus = serde_json::from_str("\"active\"").unwrap();
    assert!(matches!(status, UserStatus::Active));

    let status: UserStatus = serde_json::from_str("\"suspended\"").unwrap();
    assert!(matches!(status, UserStatus::Suspended));
}

// ==================== UserRateLimits Tests ====================

#[test]
fn test_user_rate_limits_full() {
    let limits = UserRateLimits {
        rpm: Some(100),
        tpm: Some(10000),
        rpd: Some(1000),
        tpd: Some(100000),
        concurrent: Some(10),
        monthly_budget: Some(500.0),
    };

    assert_eq!(limits.rpm, Some(100));
    assert_eq!(limits.tpm, Some(10000));
    assert_eq!(limits.monthly_budget, Some(500.0));
}

#[test]
fn test_user_rate_limits_partial() {
    let limits = UserRateLimits {
        rpm: Some(50),
        tpm: None,
        rpd: None,
        tpd: None,
        concurrent: Some(5),
        monthly_budget: None,
    };

    assert_eq!(limits.rpm, Some(50));
    assert!(limits.tpm.is_none());
}

#[test]
fn test_user_rate_limits_serialize() {
    let limits = UserRateLimits {
        rpm: Some(100),
        tpm: Some(10000),
        rpd: None,
        tpd: None,
        concurrent: None,
        monthly_budget: Some(250.0),
    };

    let json = serde_json::to_string(&limits).unwrap();
    assert!(json.contains("\"rpm\":100"));
    assert!(json.contains("\"monthly_budget\":250.0"));
}

// ==================== UserProfile Tests ====================

#[test]
fn test_user_profile_default() {
    let profile = UserProfile::default();
    assert!(profile.first_name.is_none());
    assert!(profile.last_name.is_none());
    assert!(profile.company.is_none());
    assert!(profile.social_links.is_empty());
}

#[test]
fn test_user_profile_full() {
    let mut social_links = std::collections::HashMap::new();
    social_links.insert(
        "twitter".to_string(),
        "https://twitter.com/test".to_string(),
    );
    social_links.insert("github".to_string(), "https://github.com/test".to_string());

    let profile = UserProfile {
        first_name: Some("John".to_string()),
        last_name: Some("Doe".to_string()),
        company: Some("Acme Corp".to_string()),
        title: Some("Engineer".to_string()),
        phone: Some("+1234567890".to_string()),
        avatar_url: Some("https://example.com/avatar.png".to_string()),
        bio: Some("Software developer".to_string()),
        location: Some("San Francisco, CA".to_string()),
        website: Some("https://johndoe.com".to_string()),
        social_links,
    };

    assert_eq!(profile.first_name, Some("John".to_string()));
    assert_eq!(profile.company, Some("Acme Corp".to_string()));
    assert_eq!(profile.social_links.len(), 2);
}

#[test]
fn test_user_profile_serialize() {
    let profile = UserProfile {
        first_name: Some("Jane".to_string()),
        last_name: Some("Smith".to_string()),
        ..Default::default()
    };

    let json = serde_json::to_string(&profile).unwrap();
    assert!(json.contains("Jane"));
    assert!(json.contains("Smith"));
}

#[test]
fn test_user_profile_clone() {
    let profile = UserProfile {
        first_name: Some("Test".to_string()),
        ..Default::default()
    };
    let cloned = profile.clone();
    assert_eq!(profile.first_name, cloned.first_name);
}

// ==================== User Creation Tests ====================

#[test]
fn test_user_new() {
    let user = create_test_user();

    assert_eq!(user.username, "testuser");
    assert_eq!(user.email, "test@example.com");
    assert_eq!(user.password_hash, "hashed_password_123");
    assert!(matches!(user.role, UserRole::User));
    assert!(matches!(user.status, UserStatus::Pending));
    assert!(user.team_ids.is_empty());
    assert!(!user.email_verified);
    assert!(!user.two_factor_enabled);
}

#[test]
fn test_user_id() {
    let user = create_test_user();
    let id = user.id();
    assert_eq!(id, user.metadata.id);
}

// ==================== User is_active Tests ====================

#[test]
fn test_user_is_active_when_pending() {
    let user = create_test_user();
    assert!(!user.is_active());
}

#[test]
fn test_user_is_active_when_active() {
    let mut user = create_test_user();
    user.status = UserStatus::Active;
    assert!(user.is_active());
}

#[test]
fn test_user_is_active_when_suspended() {
    let mut user = create_test_user();
    user.status = UserStatus::Suspended;
    assert!(!user.is_active());
}

// ==================== User has_role Tests ====================

#[test]
fn test_user_has_role_super_admin_has_all() {
    let mut user = create_test_user();
    user.role = UserRole::SuperAdmin;

    assert!(user.has_role(&UserRole::SuperAdmin));
    assert!(user.has_role(&UserRole::Admin));
    assert!(user.has_role(&UserRole::Manager));
    assert!(user.has_role(&UserRole::User));
    assert!(user.has_role(&UserRole::Viewer));
    assert!(user.has_role(&UserRole::ApiUser));
}

#[test]
fn test_user_has_role_admin_has_subordinates() {
    let mut user = create_test_user();
    user.role = UserRole::Admin;

    assert!(!user.has_role(&UserRole::SuperAdmin));
    assert!(user.has_role(&UserRole::Admin));
    assert!(user.has_role(&UserRole::Manager));
    assert!(user.has_role(&UserRole::User));
    assert!(user.has_role(&UserRole::Viewer));
    assert!(user.has_role(&UserRole::ApiUser));
}

#[test]
fn test_user_has_role_manager_has_subordinates() {
    let mut user = create_test_user();
    user.role = UserRole::Manager;

    assert!(!user.has_role(&UserRole::SuperAdmin));
    assert!(!user.has_role(&UserRole::Admin));
    assert!(user.has_role(&UserRole::Manager));
    assert!(user.has_role(&UserRole::User));
    assert!(user.has_role(&UserRole::Viewer));
    assert!(user.has_role(&UserRole::ApiUser));
}

#[test]
fn test_user_has_role_user_only_self() {
    let user = create_test_user();

    assert!(!user.has_role(&UserRole::SuperAdmin));
    assert!(!user.has_role(&UserRole::Admin));
    assert!(!user.has_role(&UserRole::Manager));
    assert!(user.has_role(&UserRole::User));
    assert!(!user.has_role(&UserRole::Viewer));
}

// ==================== User Team Tests ====================

#[test]
fn test_user_is_in_team_empty() {
    let user = create_test_user();
    let team_id = Uuid::new_v4();
    assert!(!user.is_in_team(team_id));
}

#[test]
fn test_user_add_to_team() {
    let mut user = create_test_user();
    let team_id = Uuid::new_v4();

    user.add_to_team(team_id);

    assert!(user.is_in_team(team_id));
    assert_eq!(user.team_ids.len(), 1);
}

#[test]
fn test_user_add_to_team_no_duplicate() {
    let mut user = create_test_user();
    let team_id = Uuid::new_v4();

    user.add_to_team(team_id);
    user.add_to_team(team_id);

    assert_eq!(user.team_ids.len(), 1);
}

#[test]
fn test_user_add_to_multiple_teams() {
    let mut user = create_test_user();
    let team1 = Uuid::new_v4();
    let team2 = Uuid::new_v4();
    let team3 = Uuid::new_v4();

    user.add_to_team(team1);
    user.add_to_team(team2);
    user.add_to_team(team3);

    assert_eq!(user.team_ids.len(), 3);
    assert!(user.is_in_team(team1));
    assert!(user.is_in_team(team2));
    assert!(user.is_in_team(team3));
}

#[test]
fn test_user_remove_from_team() {
    let mut user = create_test_user();
    let team_id = Uuid::new_v4();

    user.add_to_team(team_id);
    user.remove_from_team(team_id);

    assert!(!user.is_in_team(team_id));
    assert!(user.team_ids.is_empty());
}

#[test]
fn test_user_remove_from_team_nonexistent() {
    let mut user = create_test_user();
    let team_id = Uuid::new_v4();

    // Should not panic
    user.remove_from_team(team_id);

    assert!(user.team_ids.is_empty());
}

// ==================== User Authentication Tests ====================

#[test]
fn test_user_update_last_login() {
    let mut user = create_test_user();
    assert!(user.last_login_at.is_none());

    user.update_last_login();

    assert!(user.last_login_at.is_some());
}

#[test]
fn test_user_verify_email_activates_pending() {
    let mut user = create_test_user();
    assert!(!user.email_verified);
    assert!(matches!(user.status, UserStatus::Pending));

    user.verify_email();

    assert!(user.email_verified);
    assert!(matches!(user.status, UserStatus::Active));
}

#[test]
fn test_user_verify_email_keeps_other_status() {
    let mut user = create_test_user();
    user.status = UserStatus::Suspended;

    user.verify_email();

    assert!(user.email_verified);
    assert!(matches!(user.status, UserStatus::Suspended));
}

#[test]
fn test_user_enable_two_factor() {
    let mut user = create_test_user();
    assert!(!user.two_factor_enabled);

    user.enable_two_factor();

    assert!(user.two_factor_enabled);
}

#[test]
fn test_user_disable_two_factor() {
    let mut user = create_test_user();
    user.two_factor_enabled = true;

    user.disable_two_factor();

    assert!(!user.two_factor_enabled);
}

// ==================== User Usage Tests ====================

#[test]
fn test_user_update_usage() {
    let mut user = create_test_user();

    user.update_usage(10, 1000, 0.05);

    assert_eq!(user.usage_stats.total_requests, 10);
    assert_eq!(user.usage_stats.total_tokens, 1000);
    assert!((user.usage_stats.total_cost - 0.05).abs() < f64::EPSILON);
}

#[test]
fn test_user_update_usage_accumulates() {
    let mut user = create_test_user();

    user.update_usage(10, 1000, 0.05);
    user.update_usage(20, 2000, 0.10);

    assert_eq!(user.usage_stats.total_requests, 30);
    assert_eq!(user.usage_stats.total_tokens, 3000);
    assert!((user.usage_stats.total_cost - 0.15).abs() < f64::EPSILON);
}

// ==================== User Serialization Tests ====================

#[test]
fn test_user_serialize_skips_password() {
    let user = create_test_user();
    let json = serde_json::to_string(&user).unwrap();

    // Password hash should NOT be serialized
    assert!(!json.contains("hashed_password_123"));
    // Username should be serialized
    assert!(json.contains("testuser"));
    // Email should be serialized
    assert!(json.contains("test@example.com"));
}

#[test]
fn test_user_clone() {
    let user = create_test_user();
    let cloned = user.clone();

    assert_eq!(user.username, cloned.username);
    assert_eq!(user.email, cloned.email);
    assert_eq!(user.metadata.id, cloned.metadata.id);
}

#[test]
fn test_user_debug() {
    let user = create_test_user();
    let debug_str = format!("{:?}", user);

    assert!(debug_str.contains("User"));
    assert!(debug_str.contains("testuser"));
}
