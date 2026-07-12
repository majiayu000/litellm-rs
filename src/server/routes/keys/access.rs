use super::types::{CreateKeyRequest, KeyErrorResponse, UpdateKeyRequest};
use crate::auth::{AUTHENTICATION_SERVICE_UNAVAILABLE_MESSAGE, AuthMethod, AuthResult};
use crate::core::keys::{KeyInfo, KeyPermissions, KeyRateLimits, KeyStatus};
use crate::core::models::user::types::{User, UserRole};
use crate::core::types::context::RequestContext;
use crate::server::middleware::extract_auth_method_with_api_key_header;
use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use actix_web::{HttpRequest, HttpResponse, web};
use tracing::error;
use uuid::Uuid;

const MANAGEMENT_PERMISSIONS: &[&str] = &[
    "*",
    "system.admin",
    "keys.list_all",
    "users.manage",
    "config.manage",
    "teams.manage",
    "analytics.admin",
];

fn permissions_grant_management_access(permissions: &KeyPermissions) -> bool {
    permissions.is_admin
        || permissions
            .custom_permissions
            .iter()
            .any(|permission| MANAGEMENT_PERMISSIONS.contains(&permission.as_str()))
}

fn auth_can_grant_management_access(auth: &AuthResult) -> bool {
    auth.user
        .as_ref()
        .map(|user| user.has_role(&UserRole::Admin))
        .unwrap_or(false)
}

pub(super) fn check_ownership(
    requesting_user: &User,
    key_user_id: Option<Uuid>,
    key_team_id: Option<Uuid>,
) -> bool {
    if requesting_user.has_role(&UserRole::Admin) {
        return true;
    }
    if key_user_id == Some(requesting_user.id()) {
        return true;
    }
    if requesting_user.has_role(&UserRole::Manager)
        && let Some(team_id) = key_team_id
    {
        return requesting_user.team_ids.contains(&team_id);
    }
    false
}

pub(super) fn check_auth_result_ownership(
    auth: &AuthResult,
    key_user_id: Option<Uuid>,
    key_team_id: Option<Uuid>,
) -> bool {
    if let Some(ref user) = auth.user {
        check_ownership(user, key_user_id, key_team_id)
    } else {
        let caller_team = auth.context.team_id();
        caller_team.is_some() && caller_team == key_team_id
    }
}

pub(super) fn is_auth_enabled(state: &web::Data<AppState>) -> bool {
    let cfg = state.config.load();
    cfg.auth().enable_jwt || cfg.auth().enable_api_key
}

pub(super) async fn invalidate_api_key_auth_cache(state: &web::Data<AppState>, key_id: Uuid) {
    state
        .auth
        .api_key()
        .invalidate_cache_for_key_id(key_id)
        .await;
}

pub(super) async fn authenticate_request(
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
        Err(error) => {
            error!(error = %error, "Authentication infrastructure failure");
            Err(authentication_unavailable_response())
        }
    }
}

fn authentication_unavailable_response() -> HttpResponse {
    let error_response = KeyErrorResponse::internal(AUTHENTICATION_SERVICE_UNAVAILABLE_MESSAGE);
    HttpResponse::InternalServerError().json(ApiResponse::<()>::error(error_response.error))
}

pub(super) fn resolve_create_key_scope(
    auth: &AuthResult,
    request: &CreateKeyRequest,
) -> std::result::Result<(Option<Uuid>, Option<Uuid>), &'static str> {
    let requested_user_id = request.user_id;
    let requested_team_id = request.team_id;
    let requests_management_key = request
        .permissions
        .as_ref()
        .map(permissions_grant_management_access)
        .unwrap_or(false);

    if let Some(ref user) = auth.user {
        let is_admin = user.has_role(&UserRole::Admin);
        if is_admin {
            return Ok((requested_user_id, requested_team_id));
        }

        if requests_management_key {
            return Err("Only admin can create API keys with management permissions");
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
        if requests_management_key {
            return Err("Team-scoped API keys cannot create API keys with management permissions");
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

pub(super) fn validate_update_key_permissions(
    auth: Option<&AuthResult>,
    request: &UpdateKeyRequest,
) -> std::result::Result<(), &'static str> {
    let Some(permissions) = request.permissions.as_ref() else {
        return Ok(());
    };

    if !permissions_grant_management_access(permissions) {
        return Ok(());
    }

    if auth.map(auth_can_grant_management_access).unwrap_or(true) {
        return Ok(());
    }

    Err("Only admin can update API keys with management permissions")
}

pub(super) fn validate_create_key_rate_limits(
    request: &CreateKeyRequest,
) -> std::result::Result<(), &'static str> {
    validate_supported_key_rate_limits(request.rate_limits.as_ref())
}

pub(super) fn validate_update_key_rate_limits(
    request: &UpdateKeyRequest,
) -> std::result::Result<(), &'static str> {
    validate_supported_key_rate_limits(request.rate_limits.as_ref())
}

fn validate_supported_key_rate_limits(
    rate_limits: Option<&KeyRateLimits>,
) -> std::result::Result<(), &'static str> {
    let Some(rate_limits) = rate_limits else {
        return Ok(());
    };

    if rate_limits.tokens_per_minute.is_some()
        || rate_limits.requests_per_day.is_some()
        || rate_limits.tokens_per_day.is_some()
        || rate_limits.max_concurrent_requests.is_some()
    {
        return Err(
            "Only requests_per_minute API key rate limits are currently enforced; token, daily, and concurrency limits are not supported",
        );
    }

    Ok(())
}

pub(super) fn filter_and_paginate_keys(
    keys: Vec<KeyInfo>,
    status: Option<KeyStatus>,
    limit: usize,
    offset: usize,
) -> (Vec<KeyInfo>, u64) {
    let filtered: Vec<KeyInfo> = keys
        .into_iter()
        .filter(|key| status.map(|s| key.status == s).unwrap_or(true))
        .collect();
    let total = filtered.len() as u64;
    let page = filtered.into_iter().skip(offset).take(limit).collect();
    (page, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn direct_auth_distinguishes_invalid_credentials_from_storage_failure() {
        let mut config = crate::config::Config::default();
        config.gateway.auth.enable_jwt = true;
        config.gateway.auth.enable_api_key = true;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        let server = crate::server::http::HttpServer::new(&config)
            .await
            .expect("key-route test server should initialize");
        let state = web::Data::new(server.state().clone());

        let invalid_request = actix_web::test::TestRequest::default()
            .insert_header(("x-api-key", "gw-invalid-key-route-credential"))
            .to_http_request();
        let invalid_response = authenticate_request(&invalid_request, &state)
            .await
            .expect_err("invalid credentials should return an HTTP response");
        assert_eq!(
            invalid_response.status(),
            actix_web::http::StatusCode::UNAUTHORIZED
        );

        state
            .storage
            .db()
            .connection()
            .close_by_ref()
            .await
            .expect("test should close the authentication database pool");
        let outage_request = actix_web::test::TestRequest::default()
            .insert_header(("x-api-key", "gw-key-route-infrastructure-failure"))
            .to_http_request();
        let outage_response = authenticate_request(&outage_request, &state)
            .await
            .expect_err("storage failure should return a generic HTTP response");
        assert_eq!(
            outage_response.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = actix_web::body::to_bytes(outage_response.into_body())
            .await
            .expect("generic key-route authentication error should render");
        let body: serde_json::Value = serde_json::from_slice(&body)
            .expect("generic key-route authentication error should be valid JSON");
        assert_eq!(body["error"], AUTHENTICATION_SERVICE_UNAVAILABLE_MESSAGE);
        let body = body.to_string();
        for internal_detail in [
            "Storage error",
            "Database error",
            "Redis error",
            "Connection closed",
        ] {
            assert!(!body.contains(internal_detail));
        }
    }

    #[test]
    fn accepts_unset_or_rpm_only_key_rate_limits() {
        assert!(validate_supported_key_rate_limits(None).is_ok());
        assert!(
            validate_supported_key_rate_limits(Some(&KeyRateLimits {
                requests_per_minute: Some(60),
                ..Default::default()
            }))
            .is_ok()
        );
    }

    #[test]
    fn rejects_key_rate_limits_that_are_not_enforced() {
        let unsupported_limits = KeyRateLimits {
            requests_per_minute: Some(60),
            tokens_per_minute: Some(1000),
            ..Default::default()
        };

        assert!(validate_supported_key_rate_limits(Some(&unsupported_limits)).is_err());
    }
}
