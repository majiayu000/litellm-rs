//! Core entity types for user management

use super::roles::{TeamRole, UserRole};
use super::settings::{OrganizationSettings, TeamSettings, UserPreferences};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique user identifier
    pub user_id: String,
    /// User email
    pub email: String,
    /// User display name
    pub display_name: Option<String>,
    /// First name
    pub first_name: Option<String>,
    /// Last name
    pub last_name: Option<String>,
    /// User role
    pub role: UserRole,
    /// Teams the user belongs to
    pub teams: Vec<String>,
    /// User permissions
    pub permissions: Vec<String>,
    /// User metadata
    pub metadata: HashMap<String, String>,
    /// Maximum budget for the user
    pub max_budget: Option<f64>,
    /// Current spend
    pub spend: f64,
    /// Budget duration
    pub budget_duration: Option<String>,
    /// Budget reset timestamp
    pub budget_reset_at: Option<DateTime<Utc>>,
    /// Whether user is active
    pub is_active: bool,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last login timestamp
    pub last_login_at: Option<DateTime<Utc>>,
    /// User preferences
    pub preferences: UserPreferences,
}

/// Team entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    /// Unique team identifier
    pub team_id: String,
    /// Team name
    pub team_name: String,
    /// Team description
    pub description: Option<String>,
    /// Organization ID
    pub organization_id: Option<String>,
    /// Team members
    pub members: Vec<TeamMember>,
    /// Team permissions
    pub permissions: Vec<String>,
    /// Models the team can access
    pub models: Vec<String>,
    /// Maximum budget for the team
    pub max_budget: Option<f64>,
    /// Current spend
    pub spend: f64,
    /// Budget duration
    pub budget_duration: Option<String>,
    /// Budget reset timestamp
    pub budget_reset_at: Option<DateTime<Utc>>,
    /// Team metadata
    pub metadata: HashMap<String, String>,
    /// Whether team is active
    pub is_active: bool,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Team settings
    pub settings: TeamSettings,
}

/// Organization entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    /// Unique organization identifier
    pub organization_id: String,
    /// Organization name
    pub organization_name: String,
    /// Organization description
    pub description: Option<String>,
    /// Organization domain
    pub domain: Option<String>,
    /// Teams in the organization
    pub teams: Vec<String>,
    /// Organization admins
    pub admins: Vec<String>,
    /// Maximum budget for the organization
    pub max_budget: Option<f64>,
    /// Current spend
    pub spend: f64,
    /// Budget duration
    pub budget_duration: Option<String>,
    /// Budget reset timestamp
    pub budget_reset_at: Option<DateTime<Utc>>,
    /// Organization metadata
    pub metadata: HashMap<String, String>,
    /// Whether organization is active
    pub is_active: bool,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Organization settings
    pub settings: OrganizationSettings,
}

/// Team member with role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    /// User ID
    pub user_id: String,
    /// Role in the team
    pub role: TeamRole,
    /// When the user joined the team
    pub joined_at: DateTime<Utc>,
    /// Whether the member is active
    pub is_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Helper Functions ====================

    fn create_test_user() -> User {
        User {
            user_id: "user-123".to_string(),
            email: "test@example.com".to_string(),
            display_name: Some("Test User".to_string()),
            first_name: Some("Test".to_string()),
            last_name: Some("User".to_string()),
            role: UserRole::User,
            teams: vec!["team-1".to_string()],
            permissions: vec!["api.chat".to_string()],
            metadata: HashMap::new(),
            max_budget: Some(100.0),
            spend: 25.0,
            budget_duration: Some("monthly".to_string()),
            budget_reset_at: Some(Utc::now()),
            is_active: true,
            created_at: Utc::now(),
            last_login_at: Some(Utc::now()),
            preferences: UserPreferences::default(),
        }
    }

    fn create_test_team() -> Team {
        Team {
            team_id: "team-123".to_string(),
            team_name: "Test Team".to_string(),
            description: Some("A test team".to_string()),
            organization_id: Some("org-123".to_string()),
            members: vec![create_test_team_member()],
            permissions: vec!["api.chat".to_string()],
            models: vec!["gpt-4".to_string()],
            max_budget: Some(1000.0),
            spend: 250.0,
            budget_duration: Some("monthly".to_string()),
            budget_reset_at: Some(Utc::now()),
            metadata: HashMap::new(),
            is_active: true,
            created_at: Utc::now(),
            settings: TeamSettings::default(),
        }
    }

    fn create_test_organization() -> Organization {
        Organization {
            organization_id: "org-123".to_string(),
            organization_name: "Test Organization".to_string(),
            description: Some("A test organization".to_string()),
            domain: Some("example.com".to_string()),
            teams: vec!["team-1".to_string()],
            admins: vec!["admin-1".to_string()],
            max_budget: Some(10000.0),
            spend: 2500.0,
            budget_duration: Some("monthly".to_string()),
            budget_reset_at: Some(Utc::now()),
            metadata: HashMap::new(),
            is_active: true,
            created_at: Utc::now(),
            settings: OrganizationSettings::default(),
        }
    }

    fn create_test_team_member() -> TeamMember {
        TeamMember {
            user_id: "user-123".to_string(),
            role: TeamRole::Member,
            joined_at: Utc::now(),
            is_active: true,
        }
    }

    // ==================== User Tests ====================

    #[test]
    fn test_user_creation() {
        let user = create_test_user();

        assert_eq!(user.user_id, "user-123");
        assert_eq!(user.email, "test@example.com");
        assert!(user.is_active);
        assert_eq!(user.spend, 25.0);
    }

    #[test]
    fn test_user_minimal() {
        let user = User {
            user_id: "user-456".to_string(),
            email: "minimal@example.com".to_string(),
            display_name: None,
            first_name: None,
            last_name: None,
            role: UserRole::User,
            teams: vec![],
            permissions: vec![],
            metadata: HashMap::new(),
            max_budget: None,
            spend: 0.0,
            budget_duration: None,
            budget_reset_at: None,
            is_active: true,
            created_at: Utc::now(),
            last_login_at: None,
            preferences: UserPreferences::default(),
        };

        assert!(user.display_name.is_none());
        assert!(user.teams.is_empty());
        assert!(user.max_budget.is_none());
    }

    #[test]
    fn test_user_clone() {
        let user = create_test_user();
        let cloned = user.clone();

        assert_eq!(cloned.user_id, user.user_id);
        assert_eq!(cloned.email, user.email);
        assert_eq!(cloned.spend, user.spend);
    }

    #[test]
    fn test_user_debug() {
        let user = create_test_user();
        let debug_str = format!("{:?}", user);

        assert!(debug_str.contains("User"));
        assert!(debug_str.contains("user-123"));
    }

    #[test]
    fn test_user_serialization() {
        let user = create_test_user();
        let json = serde_json::to_value(&user).unwrap();

        assert_eq!(json["user_id"], "user-123");
        assert_eq!(json["email"], "test@example.com");
        assert_eq!(json["spend"], 25.0);
    }

    #[test]
    fn test_user_with_multiple_teams() {
        let mut user = create_test_user();
        user.teams = vec![
            "team-1".to_string(),
            "team-2".to_string(),
            "team-3".to_string(),
        ];

        assert_eq!(user.teams.len(), 3);
    }

    #[test]
    fn test_user_with_metadata() {
        let mut user = create_test_user();
        user.metadata
            .insert("department".to_string(), "engineering".to_string());
        user.metadata
            .insert("location".to_string(), "remote".to_string());

        assert_eq!(
            user.metadata.get("department"),
            Some(&"engineering".to_string())
        );
    }

    #[test]
    fn test_user_budget_within_limit() {
        let user = create_test_user();

        let within_budget = user.max_budget.map(|max| user.spend < max).unwrap_or(true);
        assert!(within_budget);
    }

    #[test]
    fn test_user_budget_exceeded() {
        let mut user = create_test_user();
        user.spend = 150.0; // Exceeds max_budget of 100.0

        let within_budget = user.max_budget.map(|max| user.spend < max).unwrap_or(true);
        assert!(!within_budget);
    }

    #[test]
    fn test_user_different_roles() {
        let roles = vec![
            UserRole::SuperAdmin,
            UserRole::OrgAdmin,
            UserRole::TeamAdmin,
            UserRole::User,
            UserRole::ReadOnly,
            UserRole::ServiceAccount,
        ];

        for role in roles {
            let user = User {
                user_id: "test".to_string(),
                email: "test@example.com".to_string(),
                display_name: None,
                first_name: None,
                last_name: None,
                role: role.clone(),
                teams: vec![],
                permissions: vec![],
                metadata: HashMap::new(),
                max_budget: None,
                spend: 0.0,
                budget_duration: None,
                budget_reset_at: None,
                is_active: true,
                created_at: Utc::now(),
                last_login_at: None,
                preferences: UserPreferences::default(),
            };

            assert_eq!(user.role, role);
        }
    }

    #[test]
    fn test_inactive_user() {
        let mut user = create_test_user();
        user.is_active = false;

        assert!(!user.is_active);
    }

    // ==================== Team Tests ====================

    #[test]
    fn test_team_creation() {
        let team = create_test_team();

        assert_eq!(team.team_id, "team-123");
        assert_eq!(team.team_name, "Test Team");
        assert!(team.is_active);
    }

    #[test]
    fn test_team_minimal() {
        let team = Team {
            team_id: "team-456".to_string(),
            team_name: "Minimal Team".to_string(),
            description: None,
            organization_id: None,
            members: vec![],
            permissions: vec![],
            models: vec![],
            max_budget: None,
            spend: 0.0,
            budget_duration: None,
            budget_reset_at: None,
            metadata: HashMap::new(),
            is_active: true,
            created_at: Utc::now(),
            settings: TeamSettings::default(),
        };

        assert!(team.description.is_none());
        assert!(team.members.is_empty());
        assert!(team.organization_id.is_none());
    }

    #[test]
    fn test_team_clone() {
        let team = create_test_team();
        let cloned = team.clone();

        assert_eq!(cloned.team_id, team.team_id);
        assert_eq!(cloned.spend, team.spend);
    }

    #[test]
    fn test_team_debug() {
        let team = create_test_team();
        let debug_str = format!("{:?}", team);

        assert!(debug_str.contains("Team"));
        assert!(debug_str.contains("team-123"));
    }

    #[test]
    fn test_team_serialization() {
        let team = create_test_team();
        let json = serde_json::to_value(&team).unwrap();

        assert_eq!(json["team_id"], "team-123");
        assert_eq!(json["team_name"], "Test Team");
    }

    #[test]
    fn test_team_with_multiple_members() {
        let mut team = create_test_team();
        team.members = vec![
            TeamMember {
                user_id: "user-1".to_string(),
                role: TeamRole::Owner,
                joined_at: Utc::now(),
                is_active: true,
            },
            TeamMember {
                user_id: "user-2".to_string(),
                role: TeamRole::Admin,
                joined_at: Utc::now(),
                is_active: true,
            },
            TeamMember {
                user_id: "user-3".to_string(),
                role: TeamRole::Member,
                joined_at: Utc::now(),
                is_active: true,
            },
        ];

        assert_eq!(team.members.len(), 3);
    }

    #[test]
    fn test_team_with_multiple_models() {
        let mut team = create_test_team();
        team.models = vec![
            "gpt-4".to_string(),
            "gpt-4-turbo".to_string(),
            "claude-3-opus".to_string(),
            "gemini-pro".to_string(),
        ];

        assert_eq!(team.models.len(), 4);
    }

    #[test]
    fn test_team_budget_check() {
        let team = create_test_team();

        let within_budget = team.max_budget.map(|max| team.spend < max).unwrap_or(true);
        assert!(within_budget);
    }

    // ==================== Organization Tests ====================

    #[test]
    fn test_organization_creation() {
        let org = create_test_organization();

        assert_eq!(org.organization_id, "org-123");
        assert_eq!(org.organization_name, "Test Organization");
        assert!(org.is_active);
    }

    #[test]
    fn test_organization_minimal() {
        let org = Organization {
            organization_id: "org-456".to_string(),
            organization_name: "Minimal Org".to_string(),
            description: None,
            domain: None,
            teams: vec![],
            admins: vec![],
            max_budget: None,
            spend: 0.0,
            budget_duration: None,
            budget_reset_at: None,
            metadata: HashMap::new(),
            is_active: true,
            created_at: Utc::now(),
            settings: OrganizationSettings::default(),
        };

        assert!(org.description.is_none());
        assert!(org.teams.is_empty());
    }

    #[test]
    fn test_organization_clone() {
        let org = create_test_organization();
        let cloned = org.clone();

        assert_eq!(cloned.organization_id, org.organization_id);
        assert_eq!(cloned.spend, org.spend);
    }

    #[test]
    fn test_organization_debug() {
        let org = create_test_organization();
        let debug_str = format!("{:?}", org);

        assert!(debug_str.contains("Organization"));
        assert!(debug_str.contains("org-123"));
    }

    #[test]
    fn test_organization_serialization() {
        let org = create_test_organization();
        let json = serde_json::to_value(&org).unwrap();

        assert_eq!(json["organization_id"], "org-123");
        assert_eq!(json["organization_name"], "Test Organization");
    }

    #[test]
    fn test_organization_with_multiple_teams() {
        let mut org = create_test_organization();
        org.teams = vec![
            "team-1".to_string(),
            "team-2".to_string(),
            "team-3".to_string(),
            "team-4".to_string(),
        ];

        assert_eq!(org.teams.len(), 4);
    }

    #[test]
    fn test_organization_with_multiple_admins() {
        let mut org = create_test_organization();
        org.admins = vec!["admin-1".to_string(), "admin-2".to_string()];

        assert_eq!(org.admins.len(), 2);
    }

    // ==================== TeamMember Tests ====================

    #[test]
    fn test_team_member_creation() {
        let member = create_test_team_member();

        assert_eq!(member.user_id, "user-123");
        assert_eq!(member.role, TeamRole::Member);
        assert!(member.is_active);
    }

    #[test]
    fn test_team_member_different_roles() {
        let roles = vec![
            TeamRole::Owner,
            TeamRole::Admin,
            TeamRole::Member,
            TeamRole::ReadOnly,
        ];

        for role in roles {
            let member = TeamMember {
                user_id: "test-user".to_string(),
                role: role.clone(),
                joined_at: Utc::now(),
                is_active: true,
            };

            assert_eq!(member.role, role);
        }
    }

    #[test]
    fn test_team_member_clone() {
        let member = create_test_team_member();
        let cloned = member.clone();

        assert_eq!(cloned.user_id, member.user_id);
        assert_eq!(cloned.role, member.role);
    }

    #[test]
    fn test_team_member_debug() {
        let member = create_test_team_member();
        let debug_str = format!("{:?}", member);

        assert!(debug_str.contains("TeamMember"));
        assert!(debug_str.contains("user-123"));
    }

    #[test]
    fn test_team_member_serialization() {
        let member = create_test_team_member();
        let json = serde_json::to_value(&member).unwrap();

        assert_eq!(json["user_id"], "user-123");
        assert!(json["is_active"].as_bool().unwrap());
    }

    #[test]
    fn test_inactive_team_member() {
        let mut member = create_test_team_member();
        member.is_active = false;

        assert!(!member.is_active);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_user_team_relationship() {
        let user = create_test_user();
        let team = create_test_team();

        // User should be in team
        let user_in_team = team.members.iter().any(|m| m.user_id == user.user_id);
        assert!(user_in_team);
    }

    #[test]
    fn test_team_organization_relationship() {
        let team = create_test_team();
        let org = create_test_organization();

        // Team should reference organization
        assert_eq!(team.organization_id.as_ref(), Some(&org.organization_id));
    }

    #[test]
    fn test_budget_hierarchy() {
        let user = create_test_user();
        let team = create_test_team();
        let org = create_test_organization();

        // Organization budget should be highest
        let org_budget = org.max_budget.unwrap_or(0.0);
        let team_budget = team.max_budget.unwrap_or(0.0);
        let user_budget = user.max_budget.unwrap_or(0.0);

        assert!(org_budget > team_budget);
        assert!(team_budget > user_budget);
    }

    #[test]
    fn test_spend_aggregation_simulation() {
        let users = [
            User {
                spend: 100.0,
                ..create_test_user()
            },
            User {
                user_id: "user-2".to_string(),
                spend: 150.0,
                ..create_test_user()
            },
            User {
                user_id: "user-3".to_string(),
                spend: 75.0,
                ..create_test_user()
            },
        ];

        let total_spend: f64 = users.iter().map(|u| u.spend).sum();
        assert!((total_spend - 325.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_find_team_owners() {
        let team = Team {
            team_id: "team-test".to_string(),
            team_name: "Test".to_string(),
            description: None,
            organization_id: None,
            members: vec![
                TeamMember {
                    user_id: "owner-1".to_string(),
                    role: TeamRole::Owner,
                    joined_at: Utc::now(),
                    is_active: true,
                },
                TeamMember {
                    user_id: "admin-1".to_string(),
                    role: TeamRole::Admin,
                    joined_at: Utc::now(),
                    is_active: true,
                },
                TeamMember {
                    user_id: "member-1".to_string(),
                    role: TeamRole::Member,
                    joined_at: Utc::now(),
                    is_active: true,
                },
            ],
            permissions: vec![],
            models: vec![],
            max_budget: None,
            spend: 0.0,
            budget_duration: None,
            budget_reset_at: None,
            metadata: HashMap::new(),
            is_active: true,
            created_at: Utc::now(),
            settings: TeamSettings::default(),
        };

        let owners: Vec<&TeamMember> = team
            .members
            .iter()
            .filter(|m| m.role == TeamRole::Owner)
            .collect();

        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].user_id, "owner-1");
    }

    #[test]
    fn test_find_active_team_members() {
        let team = Team {
            team_id: "team-test".to_string(),
            team_name: "Test".to_string(),
            description: None,
            organization_id: None,
            members: vec![
                TeamMember {
                    user_id: "active-1".to_string(),
                    role: TeamRole::Member,
                    joined_at: Utc::now(),
                    is_active: true,
                },
                TeamMember {
                    user_id: "inactive-1".to_string(),
                    role: TeamRole::Member,
                    joined_at: Utc::now(),
                    is_active: false,
                },
                TeamMember {
                    user_id: "active-2".to_string(),
                    role: TeamRole::Admin,
                    joined_at: Utc::now(),
                    is_active: true,
                },
            ],
            permissions: vec![],
            models: vec![],
            max_budget: None,
            spend: 0.0,
            budget_duration: None,
            budget_reset_at: None,
            metadata: HashMap::new(),
            is_active: true,
            created_at: Utc::now(),
            settings: TeamSettings::default(),
        };

        let active_count = team.members.iter().filter(|m| m.is_active).count();
        assert_eq!(active_count, 2);
    }

    #[test]
    fn test_deserialization() {
        let json = r#"{
            "user_id": "deser-user",
            "email": "deser@example.com",
            "display_name": null,
            "first_name": null,
            "last_name": null,
            "role": "User",
            "teams": [],
            "permissions": [],
            "metadata": {},
            "max_budget": null,
            "spend": 0.0,
            "budget_duration": null,
            "budget_reset_at": null,
            "is_active": true,
            "created_at": "2024-01-01T00:00:00Z",
            "last_login_at": null,
            "preferences": {
                "language": "en",
                "timezone": "UTC",
                "email_notifications": true,
                "slack_notifications": false,
                "dashboard_config": {}
            }
        }"#;

        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.user_id, "deser-user");
        assert_eq!(user.email, "deser@example.com");
    }
}
