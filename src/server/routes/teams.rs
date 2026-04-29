//! Team management HTTP endpoints
//!
//! This module provides RESTful API endpoints for team management operations.
//!
//! ## Endpoints
//!
//! - `POST   /v1/teams`                           - Create a new team
//! - `GET    /v1/teams`                           - List all teams
//! - `GET    /v1/teams/{id}`                      - Get team by ID
//! - `PUT    /v1/teams/{id}`                      - Update team
//! - `DELETE /v1/teams/{id}`                      - Delete team
//! - `POST   /v1/teams/{id}/members`              - Add member to team
//! - `DELETE /v1/teams/{id}/members/{user_id}`    - Remove member from team
//! - `PUT    /v1/teams/{id}/members/{user_id}/role` - Update member role
//! - `GET    /v1/teams/{id}/usage`                - Get team usage statistics

use crate::core::models::user::types::{User, UserRole};
use crate::core::teams::{
    AddMemberRequest, CreateTeamRequest, Team, TeamManager, TeamRole, TeamStatus,
    UpdateRoleRequest, UpdateTeamRequest,
};
use crate::core::types::context::RequestContext;
use crate::server::routes::{ApiResponse, PaginatedResponse, PaginationQuery, errors};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Result as ActixResult, web};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

/// Create team request body
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateTeamBody {
    /// Team name (unique, alphanumeric with hyphens and underscores)
    pub name: String,
    /// Display name for the team
    pub display_name: Option<String>,
    /// Team description
    pub description: Option<String>,
}

/// Update team request body
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UpdateTeamBody {
    /// New team name
    pub name: Option<String>,
    /// New display name
    pub display_name: Option<String>,
    /// New description
    pub description: Option<String>,
}

/// Add member request body
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AddMemberBody {
    /// User ID to add
    pub user_id: Uuid,
    /// Role to assign (owner, admin, manager, member, viewer)
    pub role: TeamRole,
}

/// Update role request body
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UpdateRoleBody {
    /// New role
    pub role: TeamRole,
}

/// Team response with additional metadata
#[derive(Debug, Clone, serde::Serialize)]
pub struct TeamResponse {
    /// Team data
    #[serde(flatten)]
    pub team: Team,
    /// Number of members (optional, for list responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_count: Option<usize>,
}

/// Path parameters for team endpoints
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TeamPath {
    /// Team ID
    pub id: Uuid,
}

/// Path parameters for member endpoints
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MemberPath {
    /// Team ID
    pub id: Uuid,
    /// User ID
    pub user_id: Uuid,
}

/// Get the shared team manager from app state
fn get_team_manager(state: &web::Data<AppState>) -> Arc<TeamManager> {
    state.team_manager.clone()
}

#[derive(Debug, Clone)]
enum RequestCaller {
    User(Box<User>),
    Team(Uuid),
}

#[derive(Debug, Clone, Copy)]
enum TeamPermission {
    Member,
    Admin,
}

/// Returns `true` when at least one auth backend is enabled.
fn is_auth_enabled(state: &web::Data<AppState>) -> bool {
    let cfg = state.config.load();
    cfg.auth().enable_jwt || cfg.auth().enable_api_key
}

fn unauthorized_response(message: &str) -> HttpResponse {
    HttpResponse::Unauthorized().json(ApiResponse::<()>::error(message.to_string()))
}

fn forbidden_response(message: &str) -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::<()>::error(message.to_string()))
}

fn get_request_caller(req: &HttpRequest) -> Option<RequestCaller> {
    if let Some(user) = req.extensions().get::<User>() {
        return Some(RequestCaller::User(Box::new(user.clone())));
    }

    req.extensions()
        .get::<RequestContext>()
        .and_then(|ctx| ctx.team_id())
        .map(RequestCaller::Team)
}

async fn has_team_access(
    manager: &TeamManager,
    caller: &RequestCaller,
    team_id: Uuid,
    required: TeamPermission,
) -> Result<bool, GatewayError> {
    match caller {
        RequestCaller::User(user) => {
            if user.has_role(&UserRole::Admin) {
                return Ok(true);
            }

            match required {
                TeamPermission::Member => {
                    manager
                        .check_user_role(
                            team_id,
                            user.id(),
                            &[
                                TeamRole::Owner,
                                TeamRole::Admin,
                                TeamRole::Manager,
                                TeamRole::Member,
                                TeamRole::Viewer,
                            ],
                        )
                        .await
                }
                TeamPermission::Admin => manager.is_team_admin(team_id, user.id()).await,
            }
        }
        RequestCaller::Team(caller_team_id) => {
            Ok(*caller_team_id == team_id && matches!(required, TeamPermission::Member))
        }
    }
}

async fn authorize_team_operation(
    req: &HttpRequest,
    state: &web::Data<AppState>,
    team_id: Uuid,
    required: TeamPermission,
) -> Result<(), HttpResponse> {
    if !is_auth_enabled(state) {
        return Ok(());
    }

    let caller = match get_request_caller(req) {
        Some(caller) => caller,
        None => return Err(unauthorized_response("Authentication required")),
    };

    let manager = get_team_manager(state);
    match has_team_access(&manager, &caller, team_id, required).await {
        Ok(true) => Ok(()),
        Ok(false) => {
            if matches!(&caller, RequestCaller::Team(_))
                && matches!(required, TeamPermission::Admin)
            {
                Err(forbidden_response(
                    "Team-scoped API keys cannot perform this operation",
                ))
            } else {
                Err(forbidden_response("Not authorized for this team operation"))
            }
        }
        Err(e) => Err(errors::gateway_error_to_response(e)),
    }
}

/// Resolve inviter user ID from request auth context.
///
/// Priority:
/// 1. Auth middleware injected `User` extension.
/// 2. Fallback to `RequestContext.user_id` when present and parseable.
fn resolve_invited_by(req: &HttpRequest) -> Option<Uuid> {
    if let Some(user) = req.extensions().get::<User>() {
        return Some(user.id());
    }

    req.extensions()
        .get::<RequestContext>()
        .and_then(|ctx| ctx.user_id.as_deref())
        .and_then(|user_id| Uuid::parse_str(user_id).ok())
}

/// Create a new team
///
/// POST /v1/teams
pub async fn create_team(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<CreateTeamBody>,
) -> ActixResult<HttpResponse> {
    info!("Creating team: {}", body.name);

    if is_auth_enabled(&state) {
        match get_request_caller(&req) {
            Some(RequestCaller::User(user)) if user.has_role(&UserRole::Admin) => {}
            Some(_) => {
                return Ok(forbidden_response(
                    "Admin privileges required to create teams",
                ));
            }
            None => return Ok(unauthorized_response("Authentication required")),
        }
    }

    let manager = get_team_manager(&state);

    let request = CreateTeamRequest {
        name: body.name.clone(),
        display_name: body.display_name.clone(),
        description: body.description.clone(),
        settings: None,
    };

    match manager.create_team(request).await {
        Ok(team) => {
            info!("Team created: {} ({})", team.name, team.id());
            Ok(
                HttpResponse::Created().json(ApiResponse::success(TeamResponse {
                    team,
                    member_count: Some(0),
                })),
            )
        }
        Err(e) => {
            error!("Failed to create team: {}", e);
            Ok(errors::gateway_error_to_response(e))
        }
    }
}

/// List all teams
///
/// GET /v1/teams
pub async fn list_teams(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<PaginationQuery>,
) -> ActixResult<HttpResponse> {
    if let Err(e) = query.validate() {
        return Ok(errors::validation_error(&e));
    }

    let manager = get_team_manager(&state);
    let list_result = if !is_auth_enabled(&state) {
        manager.list_teams(query.offset(), query.limit).await
    } else {
        match get_request_caller(&req) {
            Some(RequestCaller::User(user)) if user.has_role(&UserRole::Admin) => {
                manager.list_teams(query.offset(), query.limit).await
            }
            Some(RequestCaller::User(user)) => match manager.get_user_teams(user.id()).await {
                Ok(teams) => {
                    let visible: Vec<Team> = teams
                        .into_iter()
                        .filter(|t| !matches!(t.status, TeamStatus::Deleted))
                        .collect();
                    let total = visible.len() as u64;
                    let paginated = visible
                        .into_iter()
                        .skip(query.offset() as usize)
                        .take(query.limit as usize)
                        .collect();
                    Ok((paginated, total))
                }
                Err(e) => Err(e),
            },
            Some(RequestCaller::Team(team_id)) => match manager.get_team(team_id).await {
                Ok(team) if !matches!(team.status, TeamStatus::Deleted) => {
                    let mut teams = vec![team];
                    let total = teams.len() as u64;
                    let paginated = teams
                        .drain(..)
                        .skip(query.offset() as usize)
                        .take(query.limit as usize)
                        .collect();
                    Ok((paginated, total))
                }
                Ok(_) => Ok((Vec::new(), 0)),
                Err(GatewayError::NotFound(_)) => Ok((Vec::new(), 0)),
                Err(e) => Err(e),
            },
            None => return Ok(unauthorized_response("Authentication required")),
        }
    };

    match list_result {
        Ok((teams, total)) => {
            let team_responses: Vec<TeamResponse> = teams
                .into_iter()
                .map(|team| TeamResponse {
                    team,
                    member_count: None,
                })
                .collect();

            Ok(
                HttpResponse::Ok().json(ApiResponse::success(PaginatedResponse::new(
                    team_responses,
                    query.page,
                    query.limit,
                    total,
                ))),
            )
        }
        Err(e) => {
            error!("Failed to list teams: {}", e);
            Ok(errors::gateway_error_to_response(e))
        }
    }
}

/// Get team by ID
///
/// GET /v1/teams/{id}
pub async fn get_team(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
) -> ActixResult<HttpResponse> {
    if let Err(resp) = authorize_team_operation(&req, &state, path.id, TeamPermission::Member).await
    {
        return Ok(resp);
    }

    let manager = get_team_manager(&state);

    match manager.get_team(path.id).await {
        Ok(team) => {
            // Get member count
            let member_count = manager
                .list_members(path.id)
                .await
                .map(|m| m.len())
                .unwrap_or(0);

            Ok(HttpResponse::Ok().json(ApiResponse::success(TeamResponse {
                team,
                member_count: Some(member_count),
            })))
        }
        Err(e) => {
            error!("Failed to get team {}: {}", path.id, e);
            Ok(errors::gateway_error_to_response(e))
        }
    }
}

/// Update team
///
/// PUT /v1/teams/{id}
pub async fn update_team(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
    body: web::Json<UpdateTeamBody>,
) -> ActixResult<HttpResponse> {
    info!("Updating team: {}", path.id);

    if let Err(resp) = authorize_team_operation(&req, &state, path.id, TeamPermission::Admin).await
    {
        return Ok(resp);
    }

    let manager = get_team_manager(&state);

    let request = UpdateTeamRequest {
        name: body.name.clone(),
        display_name: body.display_name.clone(),
        description: body.description.clone(),
        settings: None,
        status: None,
    };

    match manager.update_team(path.id, request).await {
        Ok(team) => {
            info!("Team updated: {} ({})", team.name, team.id());
            Ok(HttpResponse::Ok().json(ApiResponse::success(TeamResponse {
                team,
                member_count: None,
            })))
        }
        Err(e) => {
            error!("Failed to update team {}: {}", path.id, e);
            Ok(errors::gateway_error_to_response(e))
        }
    }
}

/// Delete team
///
/// DELETE /v1/teams/{id}
pub async fn delete_team(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
) -> ActixResult<HttpResponse> {
    info!("Deleting team: {}", path.id);

    if let Err(resp) = authorize_team_operation(&req, &state, path.id, TeamPermission::Admin).await
    {
        return Ok(resp);
    }

    let manager = get_team_manager(&state);

    match manager.delete_team(path.id).await {
        Ok(()) => {
            info!("Team deleted: {}", path.id);
            Ok(HttpResponse::NoContent().finish())
        }
        Err(e) => {
            error!("Failed to delete team {}: {}", path.id, e);
            Ok(errors::gateway_error_to_response(e))
        }
    }
}

/// Add member to team
///
/// POST /v1/teams/{id}/members
pub async fn add_member(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
    body: web::Json<AddMemberBody>,
) -> ActixResult<HttpResponse> {
    info!(
        "Adding member {} to team {} with role {:?}",
        body.user_id, path.id, body.role
    );

    if let Err(resp) = authorize_team_operation(&req, &state, path.id, TeamPermission::Admin).await
    {
        return Ok(resp);
    }

    let manager = get_team_manager(&state);

    let request = AddMemberRequest {
        user_id: body.user_id,
        role: body.role.clone(),
    };

    let invited_by = resolve_invited_by(&req);

    match manager.add_member(path.id, request, invited_by).await {
        Ok(member) => {
            info!("Member {} added to team {}", member.user_id, path.id);
            Ok(HttpResponse::Created().json(ApiResponse::success(member)))
        }
        Err(e) => {
            error!(
                "Failed to add member {} to team {}: {}",
                body.user_id, path.id, e
            );
            Ok(errors::gateway_error_to_response(e))
        }
    }
}

/// Get team members
///
/// GET /v1/teams/{id}/members
pub async fn list_members(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
) -> ActixResult<HttpResponse> {
    if let Err(resp) = authorize_team_operation(&req, &state, path.id, TeamPermission::Member).await
    {
        return Ok(resp);
    }

    let manager = get_team_manager(&state);

    match manager.list_members(path.id).await {
        Ok(members) => Ok(HttpResponse::Ok().json(ApiResponse::success(members))),
        Err(e) => {
            error!("Failed to list members for team {}: {}", path.id, e);
            Ok(errors::gateway_error_to_response(e))
        }
    }
}

/// Remove member from team
///
/// DELETE /v1/teams/{id}/members/{user_id}
pub async fn remove_member(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<MemberPath>,
) -> ActixResult<HttpResponse> {
    info!("Removing member {} from team {}", path.user_id, path.id);

    if let Err(resp) = authorize_team_operation(&req, &state, path.id, TeamPermission::Admin).await
    {
        return Ok(resp);
    }

    let manager = get_team_manager(&state);

    match manager.remove_member(path.id, path.user_id).await {
        Ok(()) => {
            info!("Member {} removed from team {}", path.user_id, path.id);
            Ok(HttpResponse::NoContent().finish())
        }
        Err(e) => {
            error!(
                "Failed to remove member {} from team {}: {}",
                path.user_id, path.id, e
            );
            Ok(errors::gateway_error_to_response(e))
        }
    }
}

/// Update member role
///
/// PUT /v1/teams/{id}/members/{user_id}/role
pub async fn update_member_role(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<MemberPath>,
    body: web::Json<UpdateRoleBody>,
) -> ActixResult<HttpResponse> {
    info!(
        "Updating role for member {} in team {} to {:?}",
        path.user_id, path.id, body.role
    );

    if let Err(resp) = authorize_team_operation(&req, &state, path.id, TeamPermission::Admin).await
    {
        return Ok(resp);
    }

    let manager = get_team_manager(&state);

    let request = UpdateRoleRequest {
        role: body.role.clone(),
    };

    match manager
        .update_member_role(path.id, path.user_id, request)
        .await
    {
        Ok(member) => {
            info!(
                "Member {} role updated to {:?} in team {}",
                path.user_id, member.role, path.id
            );
            Ok(HttpResponse::Ok().json(ApiResponse::success(member)))
        }
        Err(e) => {
            error!(
                "Failed to update role for member {} in team {}: {}",
                path.user_id, path.id, e
            );
            Ok(errors::gateway_error_to_response(e))
        }
    }
}

/// Get team usage statistics
///
/// GET /v1/teams/{id}/usage
pub async fn get_team_usage(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
) -> ActixResult<HttpResponse> {
    if let Err(resp) = authorize_team_operation(&req, &state, path.id, TeamPermission::Member).await
    {
        return Ok(resp);
    }

    let manager = get_team_manager(&state);

    match manager.get_team_usage(path.id).await {
        Ok(usage) => Ok(HttpResponse::Ok().json(ApiResponse::success(usage))),
        Err(e) => {
            error!("Failed to get usage for team {}: {}", path.id, e);
            Ok(errors::gateway_error_to_response(e))
        }
    }
}

/// Configure team routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1/teams")
            .route("", web::post().to(create_team))
            .route("", web::get().to(list_teams))
            .route("/{id}", web::get().to(get_team))
            .route("/{id}", web::put().to(update_team))
            .route("/{id}", web::delete().to(delete_team))
            .route("/{id}/members", web::post().to(add_member))
            .route("/{id}/members", web::get().to(list_members))
            .route("/{id}/members/{user_id}", web::delete().to(remove_member))
            .route(
                "/{id}/members/{user_id}/role",
                web::put().to(update_member_role),
            )
            .route("/{id}/usage", web::get().to(get_team_usage)),
    );
}

#[cfg(test)]
mod tests {
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
        let same_team_member =
            has_team_access(&manager, &caller, team.id(), TeamPermission::Member)
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
}
