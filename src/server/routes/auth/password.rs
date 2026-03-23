//! Password management endpoints

use crate::server::middleware::AuthRateLimiter;
use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use crate::utils::data::validation::DataValidator;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{info, warn};

use super::models::{ChangePasswordRequest, ForgotPasswordRequest, ResetPasswordRequest};
use super::user::get_authenticated_user;

/// Global password reset rate limiter: 5 attempts per IP per 15 minutes
static PASSWORD_RESET_RATE_LIMITER: std::sync::OnceLock<Arc<AuthRateLimiter>> =
    std::sync::OnceLock::new();

fn get_password_reset_rate_limiter() -> Arc<AuthRateLimiter> {
    PASSWORD_RESET_RATE_LIMITER
        .get_or_init(|| Arc::new(AuthRateLimiter::new(5, 900, 900)))
        .clone()
}

/// Parse the leftmost valid IP from an X-Forwarded-For header value.
fn client_ip_from_xff(xff: &str) -> Option<String> {
    xff.split(',')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<std::net::IpAddr>().ok())
        .map(|ip| ip.to_string())
}

/// Extract the rate-limiting key (client IP) from the request.
fn extract_client_ip(req: &HttpRequest, trusted_proxies: &[String]) -> String {
    let peer = req
        .connection_info()
        .peer_addr()
        .unwrap_or("unknown")
        .to_string();

    let peer_ip = peer
        .parse::<std::net::SocketAddr>()
        .map(|addr| addr.ip().to_string())
        .unwrap_or(peer);

    if !trusted_proxies.is_empty()
        && trusted_proxies.contains(&peer_ip)
        && let Some(xff) = req.headers().get("x-forwarded-for")
        && let Ok(xff_str) = xff.to_str()
        && let Some(client_ip) = client_ip_from_xff(xff_str)
    {
        return client_ip;
    }

    peer_ip
}

/// Counter for probabilistic cleanup of rate limiter entries
static PASSWORD_RESET_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Forgot password endpoint
pub async fn forgot_password(
    req: HttpRequest,
    state: web::Data<AppState>,
    request: web::Json<ForgotPasswordRequest>,
) -> ActixResult<HttpResponse> {
    let cfg = state.config.load();
    let client_ip = extract_client_ip(&req, &cfg.gateway.server.trusted_proxies);

    // Rate limit: max 5 password reset requests per IP per 15 minutes
    let limiter = get_password_reset_rate_limiter();

    // Probabilistic cleanup: every 100th request, purge stale entries
    let count = PASSWORD_RESET_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    if count.is_multiple_of(100) {
        limiter.cleanup_old_entries();
    }

    if let Err(retry_after) = limiter.check_allowed(&client_ip) {
        warn!(
            "Password reset rate limit exceeded for IP {}: retry after {}s",
            client_ip, retry_after
        );
        return Ok(HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", retry_after.to_string()))
            .json(ApiResponse::<()>::error(
                "Too many password reset attempts. Please try again later.".to_string(),
            )));
    }

    info!("Password reset request received from IP {}", client_ip);

    // Generate reset token
    match state.auth.request_password_reset(&request.email).await {
        Ok(_reset_token) => {
            // NOTE: Email sending for password reset not yet implemented.
            info!("Password reset token generated");
            // Record as failure to count against the rate limit regardless of outcome,
            // preventing enumeration attacks
            limiter.record_failure(&client_ip);
            Ok(HttpResponse::Ok().json(ApiResponse::success(())))
        }
        Err(e) => {
            // Don't reveal if email exists or not
            warn!("Password reset request failed: {}", e);
            limiter.record_failure(&client_ip);
            Ok(HttpResponse::Ok().json(ApiResponse::success(())))
        }
    }
}

/// Reset password endpoint
pub async fn reset_password(
    req: HttpRequest,
    state: web::Data<AppState>,
    request: web::Json<ResetPasswordRequest>,
) -> ActixResult<HttpResponse> {
    let cfg = state.config.load();
    let client_ip = extract_client_ip(&req, &cfg.gateway.server.trusted_proxies);

    // Rate limit: max 5 reset attempts per IP per 15 minutes
    let limiter = get_password_reset_rate_limiter();

    // Probabilistic cleanup: every 100th request, purge stale entries
    let count = PASSWORD_RESET_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    if count.is_multiple_of(100) {
        limiter.cleanup_old_entries();
    }

    if let Err(retry_after) = limiter.check_allowed(&client_ip) {
        warn!(
            "Password reset token rate limit exceeded for IP {}: retry after {}s",
            client_ip, retry_after
        );
        return Ok(HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", retry_after.to_string()))
            .json(ApiResponse::<()>::error(
                "Too many password reset attempts. Please try again later.".to_string(),
            )));
    }

    info!("Password reset with token from IP {}", client_ip);

    // Validate new password
    if let Err(e) = DataValidator::validate_password(&request.new_password) {
        return Ok(HttpResponse::Ok().json(ApiResponse::<()>::error_for_type(e.to_string())));
    }

    // Reset password
    match state
        .auth
        .reset_password(&request.token, &request.new_password)
        .await
    {
        Ok(()) => {
            info!("Password reset successfully from IP {}", client_ip);
            Ok(HttpResponse::Ok().json(ApiResponse::success(())))
        }
        Err(e) => {
            warn!("Password reset failed from IP {}: {}", client_ip, e);
            limiter.record_failure(&client_ip);
            Ok(HttpResponse::Ok().json(ApiResponse::<()>::error_for_type(
                "Invalid or expired reset token".to_string(),
            )))
        }
    }
}

/// Change password endpoint
pub async fn change_password(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<ChangePasswordRequest>,
) -> ActixResult<HttpResponse> {
    info!("Password change request");

    // Get authenticated user
    let user = match get_authenticated_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .json(ApiResponse::<()>::error("Unauthorized".to_string())));
        }
    };

    // Validate new password
    if let Err(e) = DataValidator::validate_password(&request.new_password) {
        return Ok(HttpResponse::Ok().json(ApiResponse::<()>::error_for_type(e.to_string())));
    }

    // Change password
    match state
        .auth
        .change_password(user.id(), &request.current_password, &request.new_password)
        .await
    {
        Ok(()) => {
            info!("Password changed successfully for user: {}", user.username);
            Ok(HttpResponse::Ok().json(ApiResponse::success(())))
        }
        Err(e) => {
            warn!("Password change failed: {}", e);
            Ok(HttpResponse::Ok().json(ApiResponse::<()>::error_for_type(e.to_string())))
        }
    }
}
