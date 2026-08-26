//! Password management endpoints

use crate::server::middleware::{AuthRateLimiter, trusted_network_client_key};
use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use crate::utils::data::validation::DataValidator;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
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

/// Counter for probabilistic cleanup of rate limiter entries
static PASSWORD_RESET_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);
const PASSWORD_RESET_MIN_RESPONSE_TIME: Duration = Duration::from_millis(250);

fn password_reset_padding(elapsed: Duration) -> Option<Duration> {
    if elapsed < PASSWORD_RESET_MIN_RESPONSE_TIME {
        Some(PASSWORD_RESET_MIN_RESPONSE_TIME - elapsed)
    } else {
        None
    }
}

async fn enforce_password_reset_min_response_time(started_at: Instant) {
    if let Some(delay) = password_reset_padding(started_at.elapsed()) {
        tokio::time::sleep(delay).await;
    }
}

/// Forgot password endpoint
pub async fn forgot_password(
    req: HttpRequest,
    state: web::Data<AppState>,
    request: web::Json<ForgotPasswordRequest>,
) -> ActixResult<HttpResponse> {
    let started_at = Instant::now();
    let cfg = state.config.load();
    let client_id = trusted_network_client_key(&req, &cfg.gateway.server.trusted_proxies);
    let client_ip = client_id.as_deref().unwrap_or("unknown");

    // Rate limit: max 5 password reset requests per IP per 15 minutes
    let limiter = get_password_reset_rate_limiter();

    // Probabilistic cleanup: every 100th request, purge stale entries
    let count = PASSWORD_RESET_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    if count.is_multiple_of(100) {
        limiter.cleanup_old_entries();
    }

    let auth_attempt = match limiter.reserve_network_attempt(client_id.as_deref()).await {
        Ok(attempt) => attempt,
        Err(retry_after) => {
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
    };
    // This endpoint counts every admitted request, regardless of whether the
    // address exists. Commit before the backend await so cancellation cannot
    // bypass the enumeration/request-volume limit.
    auth_attempt.record_failure();

    info!("Password reset request received from IP {}", client_ip);

    // Generate reset token
    match state.auth.request_password_reset(&request.email).await {
        Ok(_reset_token) => {
            // NOTE: Email sending for password reset not yet implemented.
            info!("Password reset token generated");
            enforce_password_reset_min_response_time(started_at).await;
            Ok(HttpResponse::Ok().json(ApiResponse::success(())))
        }
        Err(e) => {
            // Don't reveal if email exists or not
            warn!("Password reset request failed: {}", e);
            enforce_password_reset_min_response_time(started_at).await;
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
    let client_id = trusted_network_client_key(&req, &cfg.gateway.server.trusted_proxies);
    let client_ip = client_id.as_deref().unwrap_or("unknown");

    // Rate limit: max 5 reset attempts per IP per 15 minutes
    let limiter = get_password_reset_rate_limiter();

    // Probabilistic cleanup: every 100th request, purge stale entries
    let count = PASSWORD_RESET_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    if count.is_multiple_of(100) {
        limiter.cleanup_old_entries();
    }

    let auth_attempt = match limiter.reserve_network_attempt(client_id.as_deref()).await {
        Ok(attempt) => attempt,
        Err(retry_after) => {
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
    };

    info!("Password reset with token from IP {}", client_ip);

    // Validate new password
    if let Err(e) = DataValidator::validate_password(&request.new_password) {
        auth_attempt.release();
        return Ok(HttpResponse::Ok().json(ApiResponse::<()>::error_for_type(e.to_string())));
    }

    // Reset password
    match state
        .auth
        .reset_password(&request.token, &request.new_password)
        .await
    {
        Ok(()) => {
            auth_attempt.release();
            info!("Password reset successfully from IP {}", client_ip);
            Ok(HttpResponse::Ok().json(ApiResponse::success(())))
        }
        Err(e) => {
            auth_attempt.record_failure();
            warn!("Password reset failed from IP {}: {}", client_ip, e);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_reset_padding_equalizes_fast_paths() {
        assert_eq!(
            password_reset_padding(Duration::from_millis(10)),
            Some(Duration::from_millis(240))
        );
        assert_eq!(password_reset_padding(Duration::from_millis(250)), None);
        assert_eq!(password_reset_padding(Duration::from_millis(500)), None);
    }
}
