use super::*;
use crate::core::teams::repository::InMemoryTeamRepository;

fn create_manager() -> TeamManager {
    let repo = Arc::new(InMemoryTeamRepository::new());
    TeamManager::new(repo)
}

#[tokio::test]
async fn test_create_team() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "test-team".to_string(),
        display_name: Some("Test Team".to_string()),
        description: Some("A test team".to_string()),
        settings: None,
    };

    let team = manager.create_team(request).await.unwrap();
    assert_eq!(team.name, "test-team");
    assert_eq!(team.display_name, Some("Test Team".to_string()));
}

#[tokio::test]
async fn test_create_team_invalid_name() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "invalid name with spaces".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let result = manager.create_team(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_team_empty_name() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let result = manager.create_team(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_duplicate_team() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "duplicate".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    manager.create_team(request.clone()).await.unwrap();
    let result = manager.create_team(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_team() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "get-test".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let created = manager.create_team(request).await.unwrap();
    let fetched = manager.get_team(created.id()).await.unwrap();

    assert_eq!(fetched.name, "get-test");
}

#[tokio::test]
async fn test_get_team_not_found() {
    let manager = create_manager();
    let result = manager.get_team(Uuid::new_v4()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_team() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "update-test".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let created = manager.create_team(request).await.unwrap();

    let update_request = UpdateTeamRequest {
        name: None,
        display_name: Some("Updated Display".to_string()),
        description: Some("Updated description".to_string()),
        settings: None,
        status: None,
    };

    let updated = manager
        .update_team(created.id(), update_request)
        .await
        .unwrap();
    assert_eq!(updated.display_name, Some("Updated Display".to_string()));
    assert_eq!(updated.description, Some("Updated description".to_string()));
}

#[tokio::test]
async fn test_delete_team() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "delete-test".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let created = manager.create_team(request).await.unwrap();
    manager.delete_team(created.id()).await.unwrap();

    let result = manager.get_team(created.id()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_teams() {
    let manager = create_manager();

    for i in 0..5 {
        let request = CreateTeamRequest {
            name: format!("team-{}", i),
            display_name: None,
            description: None,
            settings: None,
        };
        manager.create_team(request).await.unwrap();
    }

    let (teams, total) = manager.list_teams(0, 10).await.unwrap();
    assert_eq!(teams.len(), 5);
    assert_eq!(total, 5);
}

#[tokio::test]
async fn test_add_member() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "member-test".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let team = manager.create_team(request).await.unwrap();
    let user_id = Uuid::new_v4();

    let add_request = AddMemberRequest {
        user_id,
        role: TeamRole::Member,
    };

    let member = manager
        .add_member(team.id(), add_request, None)
        .await
        .unwrap();
    assert_eq!(member.user_id, user_id);
    assert!(matches!(member.role, TeamRole::Member));
}

#[tokio::test]
async fn test_update_member_role() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "role-test".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let team = manager.create_team(request).await.unwrap();
    let user_id = Uuid::new_v4();

    let add_request = AddMemberRequest {
        user_id,
        role: TeamRole::Member,
    };
    manager
        .add_member(team.id(), add_request, None)
        .await
        .unwrap();

    let update_request = UpdateRoleRequest {
        role: TeamRole::Admin,
    };

    let updated = manager
        .update_member_role(team.id(), user_id, update_request)
        .await
        .unwrap();
    assert!(matches!(updated.role, TeamRole::Admin));
}

#[tokio::test]
async fn test_remove_member() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "remove-test".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let team = manager.create_team(request).await.unwrap();

    // Add owner first
    let owner_id = Uuid::new_v4();
    let owner_request = AddMemberRequest {
        user_id: owner_id,
        role: TeamRole::Owner,
    };
    manager
        .add_member(team.id(), owner_request, None)
        .await
        .unwrap();

    // Add regular member
    let member_id = Uuid::new_v4();
    let member_request = AddMemberRequest {
        user_id: member_id,
        role: TeamRole::Member,
    };
    manager
        .add_member(team.id(), member_request, None)
        .await
        .unwrap();

    // Remove regular member
    manager.remove_member(team.id(), member_id).await.unwrap();

    let result = manager.get_member(team.id(), member_id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_cannot_remove_last_owner() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "owner-test".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let team = manager.create_team(request).await.unwrap();
    let owner_id = Uuid::new_v4();

    let add_request = AddMemberRequest {
        user_id: owner_id,
        role: TeamRole::Owner,
    };
    manager
        .add_member(team.id(), add_request, None)
        .await
        .unwrap();

    // Try to remove the only owner
    let result = manager.remove_member(team.id(), owner_id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_members() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "list-members-test".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let team = manager.create_team(request).await.unwrap();

    for _ in 0..3 {
        let add_request = AddMemberRequest {
            user_id: Uuid::new_v4(),
            role: TeamRole::Member,
        };
        manager
            .add_member(team.id(), add_request, None)
            .await
            .unwrap();
    }

    let members = manager.list_members(team.id()).await.unwrap();
    assert_eq!(members.len(), 3);
}

#[tokio::test]
async fn test_get_team_usage() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "usage-test".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let team = manager.create_team(request).await.unwrap();

    let add_request = AddMemberRequest {
        user_id: Uuid::new_v4(),
        role: TeamRole::Member,
    };
    manager
        .add_member(team.id(), add_request, None)
        .await
        .unwrap();

    let usage = manager.get_team_usage(team.id()).await.unwrap();
    assert_eq!(usage.team_name, "usage-test");
    assert_eq!(usage.member_count, 1);
}

#[tokio::test]
async fn test_is_team_admin() {
    let manager = create_manager();

    let request = CreateTeamRequest {
        name: "admin-check-test".to_string(),
        display_name: None,
        description: None,
        settings: None,
    };

    let team = manager.create_team(request).await.unwrap();

    let admin_id = Uuid::new_v4();
    let member_id = Uuid::new_v4();

    let admin_request = AddMemberRequest {
        user_id: admin_id,
        role: TeamRole::Admin,
    };
    manager
        .add_member(team.id(), admin_request, None)
        .await
        .unwrap();

    let member_request = AddMemberRequest {
        user_id: member_id,
        role: TeamRole::Member,
    };
    manager
        .add_member(team.id(), member_request, None)
        .await
        .unwrap();

    assert!(manager.is_team_admin(team.id(), admin_id).await.unwrap());
    assert!(!manager.is_team_admin(team.id(), member_id).await.unwrap());
}
