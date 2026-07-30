//! Token refresh endpoint

use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use actix_web::{HttpResponse, Result as ActixResult, web};
use tracing::{debug, error, warn};

use super::models::{RefreshTokenRequest, RefreshWireRequest};

/// Refresh token endpoint
pub async fn refresh_token(
    state: web::Data<AppState>,
    request: web::Json<RefreshTokenRequest>,
) -> ActixResult<HttpResponse> {
    refresh_token_internal(state, request.into_inner(), None).await
}

pub(super) async fn refresh_token_with_wire(
    state: web::Data<AppState>,
    request: web::Json<RefreshWireRequest>,
) -> ActixResult<HttpResponse> {
    let request = request.into_inner();
    refresh_token_internal(state, request.public, request.team_id).await
}

async fn refresh_token_internal(
    state: web::Data<AppState>,
    request: RefreshTokenRequest,
    team_id: Option<uuid::Uuid>,
) -> ActixResult<HttpResponse> {
    debug!("Token refresh request");

    // Verify refresh token
    match state
        .auth
        .jwt()
        .verify_refresh_token(&request.refresh_token)
        .await
    {
        Ok(user_id) => {
            // Find user to get current role
            let user = match state.storage.database.find_user_by_id(user_id).await {
                Ok(Some(user)) => user,
                Ok(None) => {
                    warn!("Refresh token for non-existent user: {}", user_id);
                    return Ok(HttpResponse::Unauthorized()
                        .json(ApiResponse::<()>::error("Invalid token".to_string())));
                }
                Err(e) => {
                    error!("Database error during token refresh: {}", e);
                    return Ok(HttpResponse::InternalServerError()
                        .json(ApiResponse::<()>::error("Database error".to_string())));
                }
            };

            if !user.is_active() {
                return Ok(HttpResponse::Unauthorized()
                    .json(ApiResponse::<()>::error("Invalid token".to_string())));
            }

            // Explicit selection is validated only after refresh-token and active-user
            // verification, preserving the primary authentication result.
            let verified_team = if let Some(team_id) = team_id {
                match state.auth.validate_active_team(user.id(), team_id).await {
                    Ok(Some(verified)) => Some(verified),
                    Ok(None) => {
                        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                            "Invalid team selection".to_string(),
                        )));
                    }
                    Err(error) => {
                        error!("Team validation failed during token refresh: {}", error);
                        return Ok(HttpResponse::InternalServerError().json(
                            ApiResponse::<()>::error("Internal server error".to_string()),
                        ));
                    }
                }
            } else {
                None
            };

            // Generate new token pair
            let user_permissions =
                match state.auth.rbac().get_user_permissions(&user).await {
                    Ok(permissions) => permissions,
                    Err(error) => {
                        error!("Failed to load permissions during token refresh: {}", error);
                        return Ok(HttpResponse::InternalServerError().json(
                            ApiResponse::<()>::error("Internal server error".to_string()),
                        ));
                    }
                };

            let token_result = match verified_team.as_ref() {
                Some(verified_team) => {
                    state
                        .auth
                        .jwt()
                        .create_token_pair_for_verified_team(
                            user.id(),
                            format!("{:?}", user.role),
                            user_permissions,
                            verified_team,
                            None,
                        )
                        .await
                }
                None => {
                    state
                        .auth
                        .jwt()
                        .create_token_pair(
                            user.id(),
                            format!("{:?}", user.role),
                            user_permissions,
                            None,
                            None,
                        )
                        .await
                }
            };

            match token_result {
                Ok(tokens) => {
                    debug!("Token refreshed successfully for user: {}", user.username);
                    Ok(HttpResponse::Ok().json(ApiResponse::success(tokens)))
                }
                Err(e) => {
                    error!("Failed to generate new tokens: {}", e);
                    Ok(
                        HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                            "Internal server error".to_string(),
                        )),
                    )
                }
            }
        }
        Err(e) => {
            warn!("Invalid refresh token: {}", e);
            Ok(HttpResponse::Unauthorized().json(ApiResponse::<()>::error(
                "Invalid refresh token".to_string(),
            )))
        }
    }
}
