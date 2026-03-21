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

use crate::core::teams::{
    AddMemberRequest, CreateTeamRequest, Team, TeamManager, TeamRole, UpdateRoleRequest,
    UpdateTeamRequest,
};
use crate::server::routes::{ApiResponse, PaginatedResponse, PaginationQuery, errors};
use crate::server::state::AppState;
use actix_web::{HttpResponse, Result as ActixResult, web};
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

/// Create a new team
///
/// POST /v1/teams
pub async fn create_team(
    state: web::Data<AppState>,
    body: web::Json<CreateTeamBody>,
) -> ActixResult<HttpResponse> {
    info!("Creating team: {}", body.name);

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
    state: web::Data<AppState>,
    query: web::Query<PaginationQuery>,
) -> ActixResult<HttpResponse> {
    if let Err(e) = query.validate() {
        return Ok(errors::validation_error(&e));
    }

    let manager = get_team_manager(&state);

    match manager.list_teams(query.offset(), query.limit).await {
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
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
) -> ActixResult<HttpResponse> {
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
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
    body: web::Json<UpdateTeamBody>,
) -> ActixResult<HttpResponse> {
    info!("Updating team: {}", path.id);

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
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
) -> ActixResult<HttpResponse> {
    info!("Deleting team: {}", path.id);

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
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
    body: web::Json<AddMemberBody>,
) -> ActixResult<HttpResponse> {
    info!(
        "Adding member {} to team {} with role {:?}",
        body.user_id, path.id, body.role
    );

    let manager = get_team_manager(&state);

    let request = AddMemberRequest {
        user_id: body.user_id,
        role: body.role.clone(),
    };

    // Auth context not yet wired; invited_by is left unset.
    let invited_by = None;

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
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
) -> ActixResult<HttpResponse> {
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
    state: web::Data<AppState>,
    path: web::Path<MemberPath>,
) -> ActixResult<HttpResponse> {
    info!("Removing member {} from team {}", path.user_id, path.id);

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
    state: web::Data<AppState>,
    path: web::Path<MemberPath>,
    body: web::Json<UpdateRoleBody>,
) -> ActixResult<HttpResponse> {
    info!(
        "Updating role for member {} in team {} to {:?}",
        path.user_id, path.id, body.role
    );

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
    state: web::Data<AppState>,
    path: web::Path<TeamPath>,
) -> ActixResult<HttpResponse> {
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
}
