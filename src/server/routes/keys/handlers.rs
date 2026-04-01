//! HTTP request handlers for API key management

use super::types::{
    CreateKeyRequest, CreateKeyResponse, KeyErrorResponse, KeyResponse, KeyUsageResponse,
    ListKeysQuery, ListKeysResponse, PaginationInfo, RevokeKeyResponse, RotateKeyResponse,
    UpdateKeyRequest, VerifyKeyRequest, VerifyKeyResponse,
};
use crate::auth::{AuthMethod, AuthResult};
use crate::core::keys::KeyManager;
use crate::core::keys::{CreateKeyConfig, KeyStatus, UpdateKeyConfig};
use crate::core::models::user::types::{User, UserRole};
use crate::core::types::context::RequestContext;
use crate::server::middleware::extract_auth_method_with_api_key_header;
use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Check whether `requesting_user` is allowed to access the key.
///
/// Admins and super-admins bypass the ownership check.
/// Managers can access team-scoped keys for teams they belong to.
fn check_ownership(
    requesting_user: &User,
    key_user_id: Option<Uuid>,
    key_team_id: Option<Uuid>,
) -> bool {
    // Admin (and SuperAdmin via has_role hierarchy) bypass all ownership checks.
    if requesting_user.has_role(&UserRole::Admin) {
        return true;
    }
    // User directly owns the key.
    if key_user_id == Some(requesting_user.id()) {
        return true;
    }
    // Managers can access team-scoped keys for teams they belong to.
    // has_role(Manager) is true for Manager, Admin, and SuperAdmin.
    if requesting_user.has_role(&UserRole::Manager)
        && let Some(team_id) = key_team_id
    {
        return requesting_user.team_ids.contains(&team_id);
    }
    false
}

/// Check whether an authenticated caller is allowed to access a key.
///
/// Handles both user-based callers (JWT / user-linked API key) and team-only
/// API keys (`user == None`, team context set on the request context).
fn check_auth_result_ownership(
    auth: &AuthResult,
    key_user_id: Option<Uuid>,
    key_team_id: Option<Uuid>,
) -> bool {
    if let Some(ref user) = auth.user {
        check_ownership(user, key_user_id, key_team_id)
    } else {
        // Team-only API key: allow access only to keys owned by the same team.
        let caller_team = auth.context.team_id();
        caller_team.is_some() && caller_team == key_team_id
    }
}

/// Returns `true` when at least one auth backend is enabled.
///
/// When both backends are disabled the gateway runs in no-auth mode and the
/// middleware already bypasses all credential checks, so handler-level checks
/// must be skipped too to preserve that behaviour.
fn is_auth_enabled(state: &web::Data<AppState>) -> bool {
    let cfg = state.config.load();
    cfg.auth().enable_jwt || cfg.auth().enable_api_key
}

/// Extract and authenticate the requesting caller from the request headers.
///
/// Supports the same credential schemes as the auth middleware:
/// `Authorization: Bearer`, `Authorization: ApiKey`, `Authorization: gw-...`,
/// and `X-API-Key`.
///
/// Returns `Ok(Some(result))` when valid credentials are present and accepted
/// (the caller may be a user OR a team-only API key with `result.user == None`),
/// `Ok(None)` when no credentials are present at all, and `Err(HttpResponse)`
/// when credentials are present but invalid.
async fn authenticate_request(
    req: &HttpRequest,
    state: &web::Data<AppState>,
) -> Result<Option<AuthResult>, HttpResponse> {
    let api_key_header = state.config.load().auth().api_key_header.clone();
    let auth_method =
        extract_auth_method_with_api_key_header(req.headers(), api_key_header.as_str());

    if matches!(auth_method, AuthMethod::None) {
        return Ok(None);
    }

    let context = RequestContext::new();
    match state.auth.authenticate(auth_method, context).await {
        Ok(result) if result.success => Ok(Some(result)),
        Ok(result) => {
            let msg = result
                .error
                .unwrap_or_else(|| "Authentication failed".to_string());
            let error_response = KeyErrorResponse::unauthorized(msg);
            Err(HttpResponse::Unauthorized().json(ApiResponse::<()>::error(error_response.error)))
        }
        Err(e) => {
            error!("Authentication error: {}", e);
            let error_response = KeyErrorResponse::unauthorized("Authentication error");
            Err(HttpResponse::Unauthorized().json(ApiResponse::<()>::error(error_response.error)))
        }
    }
}

/// Resolve and authorize create-key ownership scope from caller auth context.
///
/// Returns the effective `(user_id, team_id)` pair to persist.
fn resolve_create_key_scope(
    auth: &AuthResult,
    request: &CreateKeyRequest,
) -> std::result::Result<(Option<Uuid>, Option<Uuid>), &'static str> {
    let requested_user_id = request.user_id;
    let requested_team_id = request.team_id;
    let requests_admin_key = request
        .permissions
        .as_ref()
        .map(|p| p.is_admin)
        .unwrap_or(false);

    if let Some(ref user) = auth.user {
        let is_admin = user.has_role(&UserRole::Admin);
        if is_admin {
            return Ok((requested_user_id, requested_team_id));
        }

        if requests_admin_key {
            return Err("Only admin can create admin API keys");
        }

        match (requested_user_id, requested_team_id) {
            (Some(user_id), None) if user_id == user.id() => Ok((Some(user_id), None)),
            (None, Some(team_id))
                if user.has_role(&UserRole::Manager) && user.team_ids.contains(&team_id) =>
            {
                Ok((None, Some(team_id)))
            }
            (None, None) => Ok((Some(user.id()), None)),
            _ => Err("Not authorized to create API key for this scope"),
        }
    } else {
        // Team-only API key caller.
        if requests_admin_key {
            return Err("Team-scoped API keys cannot create admin API keys");
        }

        let caller_team_id = auth.context.team_id();
        match (requested_user_id, requested_team_id, caller_team_id) {
            (None, Some(requested_team), Some(caller_team)) if requested_team == caller_team => {
                Ok((None, Some(caller_team)))
            }
            (None, None, Some(caller_team)) => Ok((None, Some(caller_team))),
            _ => Err("Not authorized to create API key for this scope"),
        }
    }
}

/// POST /v1/keys - Create a new API key
pub async fn create_key(
    req: HttpRequest,
    state: web::Data<AppState>,
    request: web::Json<CreateKeyRequest>,
) -> ActixResult<HttpResponse> {
    info!("Creating new API key: {}", request.name);

    let (resolved_user_id, resolved_team_id) = if is_auth_enabled(&state) {
        let auth = match authenticate_request(&req, &state).await {
            Err(resp) => return Ok(resp),
            Ok(None) => {
                let error_response =
                    KeyErrorResponse::unauthorized("Authentication required to create a key");
                return Ok(HttpResponse::Unauthorized()
                    .json(ApiResponse::<()>::error(error_response.error)));
            }
            Ok(Some(auth)) => auth,
        };

        match resolve_create_key_scope(&auth, &request) {
            Ok(scope) => scope,
            Err(message) => {
                let error_response = KeyErrorResponse::forbidden(message);
                return Ok(
                    HttpResponse::Forbidden().json(ApiResponse::<()>::error(error_response.error))
                );
            }
        }
    } else {
        (request.user_id, request.team_id)
    };

    // Get key manager from state
    let key_manager = get_key_manager(&state);

    // Build creation config
    let config = CreateKeyConfig {
        name: request.name.clone(),
        description: request.description.clone(),
        user_id: resolved_user_id,
        team_id: resolved_team_id,
        budget_id: request.budget_id,
        permissions: request.permissions.clone().unwrap_or_default(),
        rate_limits: request.rate_limits.clone().unwrap_or_default(),
        expires_at: request.expires_at,
        metadata: request.metadata.clone().unwrap_or(serde_json::Value::Null),
    };

    // Generate the key
    match key_manager.generate_key(config).await {
        Ok((key_id, raw_key)) => {
            // Get key info
            let key_info = key_manager
                .get_key(key_id)
                .await
                .map_err(|e| {
                    error!("Failed to get key info: {}", e);
                    actix_web::error::ErrorInternalServerError("Failed to get key info")
                })?
                .ok_or_else(|| {
                    actix_web::error::ErrorInternalServerError("Key not found after creation")
                })?;

            let response = CreateKeyResponse {
                id: key_id,
                key: raw_key,
                info: key_info,
                warning: "Store this key securely. It will not be shown again.".to_string(),
            };

            info!("API key created successfully: {}", key_id);
            Ok(HttpResponse::Created().json(ApiResponse::success(response)))
        }
        Err(e) => {
            warn!("Failed to create API key: {}", e);
            let error_response = KeyErrorResponse::validation(e.to_string());
            Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(error_response.error)))
        }
    }
}

/// GET /v1/keys - List API keys
pub async fn list_keys(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<ListKeysQuery>,
) -> ActixResult<HttpResponse> {
    info!("Listing API keys");

    let key_manager = get_key_manager(&state);

    if is_auth_enabled(&state) {
        // All listing operations require authentication.
        let auth = match authenticate_request(&req, &state).await {
            Err(resp) => return Ok(resp),
            Ok(None) => {
                let error_response =
                    KeyErrorResponse::unauthorized("Authentication required to list keys");
                return Ok(HttpResponse::Unauthorized()
                    .json(ApiResponse::<()>::error(error_response.error)));
            }
            Ok(Some(a)) => a,
        };

        if let Some(requested_user_id) = query.user_id {
            // Users can list their own keys; admins can list any user's keys.
            // Team-only API keys (user == None) cannot list by user_id.
            let allowed = auth
                .user
                .as_ref()
                .map(|u| check_ownership(u, Some(requested_user_id), None))
                .unwrap_or(false);
            if !allowed {
                warn!(
                    "Caller attempted to list keys for user {} without permission",
                    requested_user_id
                );
                let error_response =
                    KeyErrorResponse::forbidden("Not authorized to list keys for this user");
                return Ok(
                    HttpResponse::Forbidden().json(ApiResponse::<()>::error(error_response.error))
                );
            }
        } else if let Some(team_id) = query.team_id {
            // Listing keys for a specific team.
            // Managers belonging to that team and team-only API keys for that team are allowed.
            let allowed = match &auth.user {
                Some(user) => {
                    user.has_role(&UserRole::Admin)
                        || (user.has_role(&UserRole::Manager) && user.team_ids.contains(&team_id))
                }
                None => auth.context.team_id() == Some(team_id),
            };
            if !allowed {
                warn!(
                    "Caller attempted to list keys for team {} without permission",
                    team_id
                );
                let error_response =
                    KeyErrorResponse::forbidden("Not authorized to list keys for this team");
                return Ok(
                    HttpResponse::Forbidden().json(ApiResponse::<()>::error(error_response.error))
                );
            }
        } else {
            // Listing all keys (no filter) requires admin privileges.
            let is_admin = auth
                .user
                .as_ref()
                .map(|u| u.has_role(&UserRole::Admin))
                .unwrap_or(false);
            if !is_admin {
                warn!("Non-admin caller attempted to list all keys");
                let error_response =
                    KeyErrorResponse::forbidden("Admin privileges required to list all keys");
                return Ok(
                    HttpResponse::Forbidden().json(ApiResponse::<()>::error(error_response.error))
                );
            }
        }
    }

    // Calculate offset from page
    let offset = ((query.page.saturating_sub(1)) * query.limit) as usize;
    let limit = query.limit as usize;

    // Get keys based on filters
    let keys = if let Some(user_id) = query.user_id {
        key_manager.list_user_keys(user_id).await
    } else if let Some(team_id) = query.team_id {
        key_manager.list_team_keys(team_id).await
    } else {
        key_manager
            .list_keys(query.status, Some(limit), Some(offset))
            .await
    };

    match keys {
        Ok(keys) => {
            // Get total count for pagination
            let total = key_manager.count_keys(query.status).await.unwrap_or(0);

            let response = ListKeysResponse {
                keys,
                pagination: PaginationInfo::new(total, query.page, query.limit),
            };

            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to list API keys: {}", e);
            let error_response = KeyErrorResponse::internal("Failed to list keys");
            Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(error_response.error)))
        }
    }
}

/// GET /v1/keys/{id} - Get a specific API key
pub async fn get_key(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    let key_id = path.into_inner();
    info!("Getting API key: {}", key_id);

    let key_manager = get_key_manager(&state);

    // Authenticate before fetching the key to avoid leaking key existence to
    // unauthenticated callers (they should not be able to distinguish 404 from
    // 401 via this endpoint).
    let auth_opt = if is_auth_enabled(&state) {
        match authenticate_request(&req, &state).await {
            Err(resp) => return Ok(resp),
            Ok(None) => {
                let error_response =
                    KeyErrorResponse::unauthorized("Authentication required to access a key");
                return Ok(HttpResponse::Unauthorized()
                    .json(ApiResponse::<()>::error(error_response.error)));
            }
            Ok(auth) => auth,
        }
    } else {
        None
    };

    match key_manager.get_key(key_id).await {
        Ok(Some(key)) => {
            // Ownership check (skipped when auth is disabled).
            if let Some(ref auth) = auth_opt
                && !check_auth_result_ownership(auth, key.user_id, key.team_id)
            {
                warn!(
                    "Caller attempted to access key {} owned by user={:?} team={:?}",
                    key_id, key.user_id, key.team_id
                );
                let error_response =
                    KeyErrorResponse::forbidden("Not authorized to access this key");
                return Ok(
                    HttpResponse::Forbidden().json(ApiResponse::<()>::error(error_response.error))
                );
            }
            let response = KeyResponse { key };
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Ok(None) => {
            warn!("API key not found: {}", key_id);
            let error_response = KeyErrorResponse::not_found("API key");
            Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error(error_response.error)))
        }
        Err(e) => {
            error!("Failed to get API key: {}", e);
            let error_response = KeyErrorResponse::internal("Failed to get key");
            Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(error_response.error)))
        }
    }
}

/// PUT /v1/keys/{id} - Update an API key
pub async fn update_key(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    request: web::Json<UpdateKeyRequest>,
) -> ActixResult<HttpResponse> {
    let key_id = path.into_inner();
    info!("Updating API key: {}", key_id);

    let key_manager = get_key_manager(&state);

    // Authenticate BEFORE fetching the key so unauthenticated callers cannot
    // probe key existence by observing the difference between 404 and 401/403.
    let auth_opt = if is_auth_enabled(&state) {
        match authenticate_request(&req, &state).await {
            Err(resp) => return Ok(resp),
            Ok(None) => {
                let error_response =
                    KeyErrorResponse::unauthorized("Authentication required to update a key");
                return Ok(HttpResponse::Unauthorized()
                    .json(ApiResponse::<()>::error(error_response.error)));
            }
            Ok(auth) => auth,
        }
    } else {
        None
    };

    // Retrieve key to verify ownership.
    let existing_key = match key_manager.get_key(key_id).await {
        Ok(Some(k)) => k,
        Ok(None) => {
            warn!("API key not found: {}", key_id);
            let error_response = KeyErrorResponse::not_found("API key");
            return Ok(
                HttpResponse::NotFound().json(ApiResponse::<()>::error(error_response.error))
            );
        }
        Err(e) => {
            error!("Failed to get API key: {}", e);
            let error_response = KeyErrorResponse::internal("Failed to get key");
            return Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(error_response.error)));
        }
    };

    if let Some(ref auth) = auth_opt
        && !check_auth_result_ownership(auth, existing_key.user_id, existing_key.team_id)
    {
        warn!(
            "Caller attempted to update key {} owned by user={:?} team={:?}",
            key_id, existing_key.user_id, existing_key.team_id
        );
        let error_response = KeyErrorResponse::forbidden("Not authorized to access this key");
        return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error(error_response.error)));
    }

    let config = UpdateKeyConfig {
        name: request.name.clone(),
        description: request.description.clone(),
        permissions: request.permissions.clone(),
        rate_limits: request.rate_limits.clone(),
        budget_id: request.budget_id,
        expires_at: request.expires_at,
        metadata: request.metadata.clone(),
    };

    match key_manager.update_key(key_id, config).await {
        Ok(key) => {
            info!("API key updated successfully: {}", key_id);
            let response = KeyResponse { key };
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Err(e) => {
            warn!("Failed to update API key: {}", e);
            let error_str = e.to_string();
            if error_str.contains("not found") {
                let error_response = KeyErrorResponse::not_found("API key");
                Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error(error_response.error)))
            } else if error_str.contains("revoked") {
                let error_response = KeyErrorResponse::conflict("Cannot update a revoked key");
                Ok(HttpResponse::Conflict().json(ApiResponse::<()>::error(error_response.error)))
            } else {
                let error_response = KeyErrorResponse::validation(error_str);
                Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(error_response.error)))
            }
        }
    }
}

/// DELETE /v1/keys/{id} - Revoke an API key
pub async fn revoke_key(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    let key_id = path.into_inner();
    info!("Revoking API key: {}", key_id);

    let key_manager = get_key_manager(&state);

    // Authenticate BEFORE fetching the key so unauthenticated callers cannot
    // probe key existence by observing the difference between 404 and 401/403.
    let auth_opt = if is_auth_enabled(&state) {
        match authenticate_request(&req, &state).await {
            Err(resp) => return Ok(resp),
            Ok(None) => {
                let error_response =
                    KeyErrorResponse::unauthorized("Authentication required to revoke a key");
                return Ok(HttpResponse::Unauthorized()
                    .json(ApiResponse::<()>::error(error_response.error)));
            }
            Ok(auth) => auth,
        }
    } else {
        None
    };

    // Retrieve key to verify ownership.
    let existing_key = match key_manager.get_key(key_id).await {
        Ok(Some(k)) => k,
        Ok(None) => {
            warn!("API key not found: {}", key_id);
            let error_response = KeyErrorResponse::not_found("API key");
            return Ok(
                HttpResponse::NotFound().json(ApiResponse::<()>::error(error_response.error))
            );
        }
        Err(e) => {
            error!("Failed to get API key: {}", e);
            let error_response = KeyErrorResponse::internal("Failed to get key");
            return Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(error_response.error)));
        }
    };

    if let Some(ref auth) = auth_opt
        && !check_auth_result_ownership(auth, existing_key.user_id, existing_key.team_id)
    {
        warn!(
            "Caller attempted to revoke key {} owned by user={:?} team={:?}",
            key_id, existing_key.user_id, existing_key.team_id
        );
        let error_response = KeyErrorResponse::forbidden("Not authorized to access this key");
        return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error(error_response.error)));
    }

    match key_manager.revoke_key(key_id).await {
        Ok(()) => {
            info!("API key revoked successfully: {}", key_id);
            let response = RevokeKeyResponse {
                key_id,
                status: KeyStatus::Revoked,
                message: "API key has been revoked successfully".to_string(),
            };
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Err(e) => {
            warn!("Failed to revoke API key: {}", e);
            let error_str = e.to_string();
            if error_str.contains("not found") {
                let error_response = KeyErrorResponse::not_found("API key");
                Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error(error_response.error)))
            } else if error_str.contains("already revoked") {
                let error_response = KeyErrorResponse::conflict("API key is already revoked");
                Ok(HttpResponse::Conflict().json(ApiResponse::<()>::error(error_response.error)))
            } else {
                let error_response = KeyErrorResponse::internal("Failed to revoke key");
                Ok(HttpResponse::InternalServerError()
                    .json(ApiResponse::<()>::error(error_response.error)))
            }
        }
    }
}

/// POST /v1/keys/{id}/rotate - Rotate an API key
pub async fn rotate_key(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    let key_id = path.into_inner();
    info!("Rotating API key: {}", key_id);

    let key_manager = get_key_manager(&state);

    // Authenticate BEFORE fetching the key so unauthenticated callers cannot
    // probe key existence by observing the difference between 404 and 401/403.
    let auth_opt = if is_auth_enabled(&state) {
        match authenticate_request(&req, &state).await {
            Err(resp) => return Ok(resp),
            Ok(None) => {
                let error_response =
                    KeyErrorResponse::unauthorized("Authentication required to rotate a key");
                return Ok(HttpResponse::Unauthorized()
                    .json(ApiResponse::<()>::error(error_response.error)));
            }
            Ok(auth) => auth,
        }
    } else {
        None
    };

    // Retrieve key to verify ownership.
    let existing_key = match key_manager.get_key(key_id).await {
        Ok(Some(k)) => k,
        Ok(None) => {
            warn!("API key not found: {}", key_id);
            let error_response = KeyErrorResponse::not_found("API key");
            return Ok(
                HttpResponse::NotFound().json(ApiResponse::<()>::error(error_response.error))
            );
        }
        Err(e) => {
            error!("Failed to get API key: {}", e);
            let error_response = KeyErrorResponse::internal("Failed to get key");
            return Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(error_response.error)));
        }
    };

    if let Some(ref auth) = auth_opt
        && !check_auth_result_ownership(auth, existing_key.user_id, existing_key.team_id)
    {
        warn!(
            "Caller attempted to rotate key {} owned by user={:?} team={:?}",
            key_id, existing_key.user_id, existing_key.team_id
        );
        let error_response = KeyErrorResponse::forbidden("Not authorized to access this key");
        return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error(error_response.error)));
    }

    match key_manager.rotate_key(key_id).await {
        Ok((new_key_id, new_raw_key)) => {
            // Get new key info
            let key_info = key_manager
                .get_key(new_key_id)
                .await
                .map_err(|e| {
                    error!("Failed to get new key info: {}", e);
                    actix_web::error::ErrorInternalServerError("Failed to get new key info")
                })?
                .ok_or_else(|| {
                    actix_web::error::ErrorInternalServerError("New key not found after rotation")
                })?;

            info!("API key rotated successfully: {} -> {}", key_id, new_key_id);

            let response = RotateKeyResponse {
                old_key_id: key_id,
                new_key_id,
                new_key: new_raw_key,
                info: key_info,
                warning: "Store this new key securely. It will not be shown again. The old key has been revoked.".to_string(),
            };

            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Err(e) => {
            warn!("Failed to rotate API key: {}", e);
            let error_str = e.to_string();
            if error_str.contains("not found") {
                let error_response = KeyErrorResponse::not_found("API key");
                Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error(error_response.error)))
            } else if error_str.contains("revoked") {
                let error_response = KeyErrorResponse::conflict("Cannot rotate a revoked key");
                Ok(HttpResponse::Conflict().json(ApiResponse::<()>::error(error_response.error)))
            } else {
                let error_response = KeyErrorResponse::internal("Failed to rotate key");
                Ok(HttpResponse::InternalServerError()
                    .json(ApiResponse::<()>::error(error_response.error)))
            }
        }
    }
}

/// GET /v1/keys/{id}/usage - Get usage statistics for an API key
pub async fn get_key_usage(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    let key_id = path.into_inner();
    info!("Getting usage stats for API key: {}", key_id);

    let key_manager = get_key_manager(&state);

    // Authenticate before fetching the key to avoid leaking key existence to
    // unauthenticated callers.
    let auth_opt = if is_auth_enabled(&state) {
        match authenticate_request(&req, &state).await {
            Err(resp) => return Ok(resp),
            Ok(None) => {
                let error_response =
                    KeyErrorResponse::unauthorized("Authentication required to access key usage");
                return Ok(HttpResponse::Unauthorized()
                    .json(ApiResponse::<()>::error(error_response.error)));
            }
            Ok(auth) => auth,
        }
    } else {
        None
    };

    // Fetch the key to perform the ownership check before returning usage data.
    let key = match key_manager.get_key(key_id).await {
        Ok(Some(k)) => k,
        Ok(None) => {
            warn!("API key not found: {}", key_id);
            let error_response = KeyErrorResponse::not_found("API key");
            return Ok(
                HttpResponse::NotFound().json(ApiResponse::<()>::error(error_response.error))
            );
        }
        Err(e) => {
            error!("Failed to get API key: {}", e);
            let error_response = KeyErrorResponse::internal("Failed to get key");
            return Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(error_response.error)));
        }
    };

    if let Some(ref auth) = auth_opt
        && !check_auth_result_ownership(auth, key.user_id, key.team_id)
    {
        warn!(
            "Caller attempted to access usage for key {} owned by user={:?} team={:?}",
            key_id, key.user_id, key.team_id
        );
        let error_response = KeyErrorResponse::forbidden("Not authorized to access this key");
        return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error(error_response.error)));
    }

    match key_manager.get_usage_stats(key_id).await {
        Ok(usage) => {
            let response = KeyUsageResponse { key_id, usage };
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Err(e) => {
            warn!("Failed to get usage stats: {}", e);
            let error_str = e.to_string();
            if error_str.contains("not found") {
                let error_response = KeyErrorResponse::not_found("API key");
                Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error(error_response.error)))
            } else {
                let error_response = KeyErrorResponse::internal("Failed to get usage stats");
                Ok(HttpResponse::InternalServerError()
                    .json(ApiResponse::<()>::error(error_response.error)))
            }
        }
    }
}

/// POST /v1/keys/verify - Verify an API key
pub async fn verify_key(
    state: web::Data<AppState>,
    request: web::Json<VerifyKeyRequest>,
) -> ActixResult<HttpResponse> {
    info!("Verifying API key");

    let key_manager = get_key_manager(&state);

    match key_manager.validate_key(&request.key).await {
        Ok(result) => {
            let response = VerifyKeyResponse { result };
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to verify API key: {}", e);
            let error_response = KeyErrorResponse::internal("Failed to verify key");
            Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(error_response.error)))
        }
    }
}

/// Helper function to get the shared KeyManager from AppState.
fn get_key_manager(state: &web::Data<AppState>) -> KeyManager {
    state.key_manager.clone()
}

#[cfg(test)]
mod handler_tests {
    use super::*;

    #[test]
    fn test_create_key_config_from_request() {
        let request = CreateKeyRequest {
            name: "Test Key".to_string(),
            description: Some("A test".to_string()),
            user_id: None,
            team_id: None,
            budget_id: None,
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
        // Key without owner — regular user cannot access
        assert!(!check_ownership(&user, None, None));
    }

    #[test]
    fn test_check_ownership_manager_team_access() {
        let team_id = Uuid::new_v4();
        let other_team = Uuid::new_v4();
        let manager = make_user(UserRole::Manager, vec![team_id]);

        // Manager can access team-scoped keys for their own team
        assert!(check_ownership(&manager, None, Some(team_id)));
        // Manager cannot access keys for a team they don't belong to
        assert!(!check_ownership(&manager, None, Some(other_team)));
        // Manager cannot access user-owned keys for other users
        assert!(!check_ownership(&manager, Some(Uuid::new_v4()), None));
    }

    #[test]
    fn test_check_ownership_regular_user_cannot_access_team_key() {
        let team_id = Uuid::new_v4();
        let user = make_user(UserRole::User, vec![team_id]);
        // Regular users do NOT get team-level management access even if team members
        assert!(!check_ownership(&user, None, Some(team_id)));
    }

    #[test]
    fn test_is_auth_enabled_logic() {
        let regular_user = make_user(UserRole::User, vec![]);
        // Regular users must NOT pass the list-all-keys gate
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
            permissions: None,
            rate_limits: None,
            expires_at: None,
            metadata: None,
        };

        assert!(resolve_create_key_scope(&auth, &request).is_err());
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
            permissions: None,
            rate_limits: None,
            expires_at: None,
            metadata: None,
        };

        let resolved = resolve_create_key_scope(&auth, &request).unwrap();
        assert_eq!(resolved, (None, Some(team_id)));
    }
}
