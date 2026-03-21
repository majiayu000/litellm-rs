//! User and team role definitions

use serde::{Deserialize, Serialize};

/// User roles
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserRole {
    /// Super admin with all permissions
    SuperAdmin,
    /// Organization admin
    OrgAdmin,
    /// Team admin
    TeamAdmin,
    /// Regular user
    User,
    /// Read-only user
    ReadOnly,
    /// Service account
    ServiceAccount,
}

/// Team roles
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TeamRole {
    /// Team owner
    Owner,
    /// Team admin
    Admin,
    /// Team member
    Member,
    /// Read-only member
    ReadOnly,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== UserRole Tests ====================

    #[test]
    fn test_user_role_super_admin() {
        let role = UserRole::SuperAdmin;
        assert_eq!(role, UserRole::SuperAdmin);
    }

    #[test]
    fn test_user_role_org_admin() {
        let role = UserRole::OrgAdmin;
        assert_eq!(role, UserRole::OrgAdmin);
    }

    #[test]
    fn test_user_role_team_admin() {
        let role = UserRole::TeamAdmin;
        assert_eq!(role, UserRole::TeamAdmin);
    }

    #[test]
    fn test_user_role_user() {
        let role = UserRole::User;
        assert_eq!(role, UserRole::User);
    }

    #[test]
    fn test_user_role_read_only() {
        let role = UserRole::ReadOnly;
        assert_eq!(role, UserRole::ReadOnly);
    }

    #[test]
    fn test_user_role_service_account() {
        let role = UserRole::ServiceAccount;
        assert_eq!(role, UserRole::ServiceAccount);
    }

    #[test]
    fn test_user_role_serialization() {
        let roles = vec![
            (UserRole::SuperAdmin, "SuperAdmin"),
            (UserRole::OrgAdmin, "OrgAdmin"),
            (UserRole::TeamAdmin, "TeamAdmin"),
            (UserRole::User, "User"),
            (UserRole::ReadOnly, "ReadOnly"),
            (UserRole::ServiceAccount, "ServiceAccount"),
        ];

        for (role, expected) in roles {
            let json = serde_json::to_string(&role).unwrap();
            assert!(json.contains(expected));
        }
    }

    #[test]
    fn test_user_role_deserialization() {
        let json = "\"SuperAdmin\"";
        let role: UserRole = serde_json::from_str(json).unwrap();
        assert_eq!(role, UserRole::SuperAdmin);

        let json = "\"User\"";
        let role: UserRole = serde_json::from_str(json).unwrap();
        assert_eq!(role, UserRole::User);
    }

    #[test]
    fn test_user_role_clone() {
        let role = UserRole::OrgAdmin;
        let cloned = role.clone();
        assert_eq!(role, cloned);
    }

    #[test]
    fn test_user_role_debug() {
        let role = UserRole::TeamAdmin;
        let debug_str = format!("{:?}", role);
        assert!(debug_str.contains("TeamAdmin"));
    }

    #[test]
    fn test_user_role_equality() {
        assert_eq!(UserRole::SuperAdmin, UserRole::SuperAdmin);
        assert_ne!(UserRole::SuperAdmin, UserRole::User);
        assert_ne!(UserRole::OrgAdmin, UserRole::TeamAdmin);
    }

    #[test]
    fn test_user_role_all_variants_distinct() {
        let roles = [
            UserRole::SuperAdmin,
            UserRole::OrgAdmin,
            UserRole::TeamAdmin,
            UserRole::User,
            UserRole::ReadOnly,
            UserRole::ServiceAccount,
        ];

        // Ensure all roles are distinct
        for (i, role1) in roles.iter().enumerate() {
            for (j, role2) in roles.iter().enumerate() {
                if i == j {
                    assert_eq!(role1, role2);
                } else {
                    assert_ne!(role1, role2);
                }
            }
        }
    }

    // ==================== TeamRole Tests ====================

    #[test]
    fn test_team_role_owner() {
        let role = TeamRole::Owner;
        assert_eq!(role, TeamRole::Owner);
    }

    #[test]
    fn test_team_role_admin() {
        let role = TeamRole::Admin;
        assert_eq!(role, TeamRole::Admin);
    }

    #[test]
    fn test_team_role_member() {
        let role = TeamRole::Member;
        assert_eq!(role, TeamRole::Member);
    }

    #[test]
    fn test_team_role_read_only() {
        let role = TeamRole::ReadOnly;
        assert_eq!(role, TeamRole::ReadOnly);
    }

    #[test]
    fn test_team_role_serialization() {
        let roles = vec![
            (TeamRole::Owner, "Owner"),
            (TeamRole::Admin, "Admin"),
            (TeamRole::Member, "Member"),
            (TeamRole::ReadOnly, "ReadOnly"),
        ];

        for (role, expected) in roles {
            let json = serde_json::to_string(&role).unwrap();
            assert!(json.contains(expected));
        }
    }

    #[test]
    fn test_team_role_deserialization() {
        let json = "\"Owner\"";
        let role: TeamRole = serde_json::from_str(json).unwrap();
        assert_eq!(role, TeamRole::Owner);

        let json = "\"Member\"";
        let role: TeamRole = serde_json::from_str(json).unwrap();
        assert_eq!(role, TeamRole::Member);
    }

    #[test]
    fn test_team_role_clone() {
        let role = TeamRole::Admin;
        let cloned = role.clone();
        assert_eq!(role, cloned);
    }

    #[test]
    fn test_team_role_debug() {
        let role = TeamRole::Member;
        let debug_str = format!("{:?}", role);
        assert!(debug_str.contains("Member"));
    }

    #[test]
    fn test_team_role_equality() {
        assert_eq!(TeamRole::Owner, TeamRole::Owner);
        assert_ne!(TeamRole::Owner, TeamRole::Member);
        assert_ne!(TeamRole::Admin, TeamRole::ReadOnly);
    }

    #[test]
    fn test_team_role_all_variants_distinct() {
        let roles = [
            TeamRole::Owner,
            TeamRole::Admin,
            TeamRole::Member,
            TeamRole::ReadOnly,
        ];

        for (i, role1) in roles.iter().enumerate() {
            for (j, role2) in roles.iter().enumerate() {
                if i == j {
                    assert_eq!(role1, role2);
                } else {
                    assert_ne!(role1, role2);
                }
            }
        }
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_role_json_roundtrip() {
        let user_roles = vec![
            UserRole::SuperAdmin,
            UserRole::OrgAdmin,
            UserRole::TeamAdmin,
            UserRole::User,
            UserRole::ReadOnly,
            UserRole::ServiceAccount,
        ];

        for role in user_roles {
            let json = serde_json::to_string(&role).unwrap();
            let parsed: UserRole = serde_json::from_str(&json).unwrap();
            assert_eq!(role, parsed);
        }

        let team_roles = vec![
            TeamRole::Owner,
            TeamRole::Admin,
            TeamRole::Member,
            TeamRole::ReadOnly,
        ];

        for role in team_roles {
            let json = serde_json::to_string(&role).unwrap();
            let parsed: TeamRole = serde_json::from_str(&json).unwrap();
            assert_eq!(role, parsed);
        }
    }

    #[test]
    fn test_roles_in_struct() {
        #[derive(Serialize, Deserialize)]
        struct UserWithRole {
            name: String,
            user_role: UserRole,
            team_role: Option<TeamRole>,
        }

        let user = UserWithRole {
            name: "John".to_string(),
            user_role: UserRole::OrgAdmin,
            team_role: Some(TeamRole::Owner),
        };

        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("OrgAdmin"));
        assert!(json.contains("Owner"));

        let parsed: UserWithRole = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.user_role, UserRole::OrgAdmin);
        assert_eq!(parsed.team_role, Some(TeamRole::Owner));
    }
}
