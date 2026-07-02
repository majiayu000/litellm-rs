use super::*;
use crate::core::teams::InMemoryTeamRepository;
use actix_web::{HttpMessage, test::TestRequest};
use std::sync::Arc;

fn make_user(role: UserRole) -> User {
    let mut user = User::new(
        "test-user".to_string(),
        "test@example.com".to_string(),
        "hash".to_string(),
    );
    user.role = role;
    user
}

async fn create_team_manager() -> TeamManager {
    TeamManager::new(Arc::new(InMemoryTeamRepository::new()))
}

#[test]
fn test_create_team_body_deserialize() {
    let json = r#"{
        "name": "test-team",
        "display_name": "Test Team",
        "description": "A test team"
    }"#;

    let body: CreateTeamBody = serde_json::from_str(json).unwrap();
    assert_eq!(body.name, "test-team");
    assert_eq!(body.display_name, Some("Test Team".to_string()));
    assert_eq!(body.description, Some("A test team".to_string()));
}

#[test]
fn test_create_team_body_minimal() {
    let json = r#"{"name": "minimal-team"}"#;

    let body: CreateTeamBody = serde_json::from_str(json).unwrap();
    assert_eq!(body.name, "minimal-team");
    assert!(body.display_name.is_none());
    assert!(body.description.is_none());
}

#[test]
fn test_update_team_body_deserialize() {
    let json = r#"{
        "name": "new-name",
        "description": "Updated description"
    }"#;

    let body: UpdateTeamBody = serde_json::from_str(json).unwrap();
    assert_eq!(body.name, Some("new-name".to_string()));
    assert!(body.display_name.is_none());
    assert_eq!(body.description, Some("Updated description".to_string()));
}

#[test]
fn test_add_member_body_deserialize() {
    let json = r#"{
        "user_id": "550e8400-e29b-41d4-a716-446655440000",
        "role": "admin"
    }"#;

    let body: AddMemberBody = serde_json::from_str(json).unwrap();
    assert_eq!(
        body.user_id.to_string(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    assert!(matches!(body.role, TeamRole::Admin));
}

#[test]
fn test_update_role_body_deserialize() {
    let json = r#"{"role": "owner"}"#;

    let body: UpdateRoleBody = serde_json::from_str(json).unwrap();
    assert!(matches!(body.role, TeamRole::Owner));
}

#[test]
fn test_team_role_deserialize_all() {
    let roles = vec![
        (r#""owner""#, TeamRole::Owner),
        (r#""admin""#, TeamRole::Admin),
        (r#""manager""#, TeamRole::Manager),
        (r#""member""#, TeamRole::Member),
        (r#""viewer""#, TeamRole::Viewer),
    ];

    for (json, expected_role) in roles {
        let role: TeamRole = serde_json::from_str(json).unwrap();
        assert!(std::mem::discriminant(&role) == std::mem::discriminant(&expected_role));
    }
}

#[test]
fn test_team_response_serialize() {
    let team = Team::new("test-team".to_string(), Some("Test Team".to_string()));
    let response = TeamResponse {
        team,
        member_count: Some(5),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("test-team"));
    assert!(json.contains("member_count"));
    assert!(json.contains("5"));
}

#[test]
fn test_team_response_without_member_count() {
    let team = Team::new("test-team".to_string(), None);
    let response = TeamResponse {
        team,
        member_count: None,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("test-team"));
    assert!(!json.contains("member_count"));
}

#[test]
fn test_resolve_invited_by_from_authenticated_user() {
    let req = TestRequest::default().to_http_request();
    let user = User::new(
        "inviter".to_string(),
        "inviter@example.com".to_string(),
        "hash".to_string(),
    );
    let expected = user.id();
    req.extensions_mut().insert(user);

    assert_eq!(resolve_invited_by(&req), Some(expected));
}

#[test]
fn test_resolve_invited_by_from_request_context_user_id() {
    let req = TestRequest::default().to_http_request();
    let mut ctx = RequestContext::new();
    let expected = Uuid::new_v4();
    ctx.user_id = Some(expected.to_string());
    req.extensions_mut().insert(ctx);

    assert_eq!(resolve_invited_by(&req), Some(expected));
}

#[test]
fn test_resolve_invited_by_returns_none_for_invalid_context_user_id() {
    let req = TestRequest::default().to_http_request();
    let mut ctx = RequestContext::new();
    ctx.user_id = Some("not-a-uuid".to_string());
    req.extensions_mut().insert(ctx);

    assert_eq!(resolve_invited_by(&req), None);
}

#[test]
fn test_get_request_caller_prefers_user() {
    let req = TestRequest::default().to_http_request();
    let user = make_user(UserRole::User);
    let expected_user_id = user.id();
    req.extensions_mut().insert(user);

    let mut ctx = RequestContext::new();
    ctx.set_team_id(Uuid::new_v4());
    req.extensions_mut().insert(ctx);

    match get_request_caller(&req) {
        Some(RequestCaller::User(u)) => assert_eq!(u.id(), expected_user_id),
        other => panic!("expected user caller, got {:?}", other),
    }
}

#[test]
fn test_get_request_caller_team_from_context() {
    let req = TestRequest::default().to_http_request();
    let team_id = Uuid::new_v4();
    let mut ctx = RequestContext::new();
    ctx.set_team_id(team_id);
    req.extensions_mut().insert(ctx);

    match get_request_caller(&req) {
        Some(RequestCaller::Team(id)) => assert_eq!(id, team_id),
        other => panic!("expected team caller, got {:?}", other),
    }
}

#[tokio::test]
async fn test_has_team_access_admin_user_can_manage_team() {
    let manager = create_team_manager().await;
    let team = manager
        .create_team(CreateTeamRequest {
            name: "team-admin-access".to_string(),
            display_name: None,
            description: None,
            settings: None,
        })
        .await
        .unwrap();

    let caller = RequestCaller::User(Box::new(make_user(UserRole::Admin)));
    let can_manage = has_team_access(&manager, &caller, team.id(), TeamPermission::Admin)
        .await
        .unwrap();
    assert!(can_manage);
}

#[tokio::test]
async fn test_has_team_access_member_user_cannot_manage_team() {
    let manager = create_team_manager().await;
    let team = manager
        .create_team(CreateTeamRequest {
            name: "team-member-access".to_string(),
            display_name: None,
            description: None,
            settings: None,
        })
        .await
        .unwrap();

    let user = make_user(UserRole::User);
    manager
        .add_member(
            team.id(),
            AddMemberRequest {
                user_id: user.id(),
                role: TeamRole::Member,
            },
            None,
        )
        .await
        .unwrap();
    let caller = RequestCaller::User(Box::new(user));

    let can_read = has_team_access(&manager, &caller, team.id(), TeamPermission::Member)
        .await
        .unwrap();
    let can_manage = has_team_access(&manager, &caller, team.id(), TeamPermission::Admin)
        .await
        .unwrap();
    assert!(can_read);
    assert!(!can_manage);
}

#[tokio::test]
async fn test_has_team_access_team_scoped_caller_member_only() {
    let manager = create_team_manager().await;
    let team = manager
        .create_team(CreateTeamRequest {
            name: "team-scoped-access".to_string(),
            display_name: None,
            description: None,
            settings: None,
        })
        .await
        .unwrap();
    let other_team = manager
        .create_team(CreateTeamRequest {
            name: "team-scoped-access-other".to_string(),
            display_name: None,
            description: None,
            settings: None,
        })
        .await
        .unwrap();

    let caller = RequestCaller::Team(team.id());
    let same_team_member = has_team_access(&manager, &caller, team.id(), TeamPermission::Member)
        .await
        .unwrap();
    let same_team_admin = has_team_access(&manager, &caller, team.id(), TeamPermission::Admin)
        .await
        .unwrap();
    let other_team_member =
        has_team_access(&manager, &caller, other_team.id(), TeamPermission::Member)
            .await
            .unwrap();

    assert!(same_team_member);
    assert!(!same_team_admin);
    assert!(!other_team_member);
}
