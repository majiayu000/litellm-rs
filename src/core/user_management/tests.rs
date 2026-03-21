//! Tests for user management module

use super::roles::{TeamRole, UserRole};
use super::settings::TeamSettings;
use super::types::{Team, TeamMember};
use chrono::Utc;
use std::collections::HashMap;

/// Test UserRole enum equality and variants
#[test]
fn test_user_roles() {
    assert_eq!(UserRole::SuperAdmin, UserRole::SuperAdmin);
    assert_ne!(UserRole::User, UserRole::OrgAdmin);

    // Test all variants exist
    let roles = [
        UserRole::SuperAdmin,
        UserRole::OrgAdmin,
        UserRole::TeamAdmin,
        UserRole::User,
        UserRole::ReadOnly,
        UserRole::ServiceAccount,
    ];
    assert_eq!(roles.len(), 6);
}

/// Test TeamRole enum equality and variants
#[test]
fn test_team_roles() {
    assert_eq!(TeamRole::Owner, TeamRole::Owner);
    assert_ne!(TeamRole::Member, TeamRole::Admin);

    // Test all variants exist
    let roles = [
        TeamRole::Owner,
        TeamRole::Admin,
        TeamRole::Member,
        TeamRole::ReadOnly,
    ];
    assert_eq!(roles.len(), 4);
}

/// Test Team structure
#[test]
fn test_team_structure() {
    let owner = TeamMember {
        user_id: "owner123".to_string(),
        role: TeamRole::Owner,
        joined_at: Utc::now(),
        is_active: true,
    };

    let team = Team {
        team_id: "team123".to_string(),
        team_name: "Test Team".to_string(),
        description: Some("A test team".to_string()),
        organization_id: None,
        members: vec![owner.clone()],
        permissions: vec![],
        models: vec![],
        max_budget: Some(1000.0),
        spend: 0.0,
        budget_duration: Some("1m".to_string()),
        budget_reset_at: None,
        metadata: HashMap::new(),
        is_active: true,
        created_at: Utc::now(),
        settings: TeamSettings::default(),
    };

    assert_eq!(team.team_name, "Test Team");
    assert_eq!(team.members.len(), 1);
    assert_eq!(team.members[0].role, TeamRole::Owner);
}

/// Test TeamMember role assignment
#[test]
fn test_team_member_roles() {
    let owner = TeamMember {
        user_id: "u1".to_string(),
        role: TeamRole::Owner,
        joined_at: Utc::now(),
        is_active: true,
    };

    let admin = TeamMember {
        user_id: "u2".to_string(),
        role: TeamRole::Admin,
        joined_at: Utc::now(),
        is_active: true,
    };

    let member = TeamMember {
        user_id: "u3".to_string(),
        role: TeamRole::Member,
        joined_at: Utc::now(),
        is_active: true,
    };

    assert_eq!(owner.role, TeamRole::Owner);
    assert_eq!(admin.role, TeamRole::Admin);
    assert_eq!(member.role, TeamRole::Member);
}

/// Test TeamSettings default values
#[test]
fn test_team_settings_defaults() {
    let settings = TeamSettings::default();

    // Verify default settings are reasonable
    assert!(settings.auto_approve_members);
    assert_eq!(settings.high_cost_threshold, Some(10.0));
}

/// Test UserRole hierarchy (conceptually)
#[test]
fn test_user_role_hierarchy() {
    // SuperAdmin > OrgAdmin > TeamAdmin > User > ReadOnly
    let super_admin = UserRole::SuperAdmin;
    let org_admin = UserRole::OrgAdmin;
    let team_admin = UserRole::TeamAdmin;
    let user = UserRole::User;
    let read_only = UserRole::ReadOnly;

    // Verify they are distinct
    assert_ne!(super_admin, org_admin);
    assert_ne!(org_admin, team_admin);
    assert_ne!(team_admin, user);
    assert_ne!(user, read_only);
}
