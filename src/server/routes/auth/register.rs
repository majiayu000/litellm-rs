//! User registration endpoint

use crate::server::middleware::{AuthRateLimiter, trusted_network_client_key};
use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use crate::utils::auth::crypto::password::hash_password;
use crate::utils::data::validation::DataValidator;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, info, warn};

use super::models::{RegisterRequest, RegisterResponse};

/// Global registration rate limiter: 10 attempts per IP per hour
static REGISTER_RATE_LIMITER: std::sync::OnceLock<Arc<AuthRateLimiter>> =
    std::sync::OnceLock::new();

fn get_register_rate_limiter() -> Arc<AuthRateLimiter> {
    REGISTER_RATE_LIMITER
        .get_or_init(|| Arc::new(AuthRateLimiter::new(10, 3600, 3600)))
        .clone()
}

/// Counter for probabilistic cleanup of rate limiter entries
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generic error message for duplicate credentials to prevent enumeration
const DUPLICATE_CREDENTIALS_ERROR: &str =
    "Registration failed. An account with these credentials may already exist.";

/// User registration endpoint
pub async fn register(
    req: HttpRequest,
    state: web::Data<AppState>,
    request: web::Json<RegisterRequest>,
) -> ActixResult<HttpResponse> {
    let trusted_proxies = state.config.load().server().trusted_proxies.clone();
    let client_id = trusted_network_client_key(&req, &trusted_proxies);
    let client_ip = client_id.as_deref().unwrap_or("unknown");

    // Rate limit: max 10 registration attempts per IP per hour
    let limiter = get_register_rate_limiter();

    // Probabilistic cleanup: every 100th request, purge stale entries
    let count = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    if count.is_multiple_of(100) {
        limiter.cleanup_old_entries();
    }

    let auth_attempt = match limiter.reserve_network_attempt(client_id.as_deref()).await {
        Ok(attempt) => attempt,
        Err(retry_after) => {
            warn!(
                "Registration rate limit exceeded for IP {}: retry after {}s",
                client_ip, retry_after
            );
            return Ok(HttpResponse::TooManyRequests()
                .insert_header(("Retry-After", retry_after.to_string()))
                .json(ApiResponse::<()>::error(
                    "Too many registration attempts. Please try again later.".to_string(),
                )));
        }
    };

    // Count every admitted registration request before any backend await.
    auth_attempt.record_failure();

    info!("User registration attempt: {}", request.username);

    // Validate input
    if let Err(e) = DataValidator::validate_username(&request.username) {
        return Ok(
            HttpResponse::BadRequest().json(ApiResponse::<()>::error_for_type(e.to_string()))
        );
    }

    if let Err(e) = crate::utils::config::helpers::ConfigValidator::validate_email(&request.email) {
        return Ok(
            HttpResponse::BadRequest().json(ApiResponse::<()>::error_for_type(e.to_string()))
        );
    }

    if let Err(e) = DataValidator::validate_password(&request.password) {
        return Ok(
            HttpResponse::BadRequest().json(ApiResponse::<()>::error_for_type(e.to_string()))
        );
    }

    // Check if user already exists
    match state
        .storage
        .database
        .find_user_by_username(&request.username)
        .await
    {
        Ok(Some(_)) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                DUPLICATE_CREDENTIALS_ERROR.to_string(),
            )));
        }
        Ok(None) => {} // Continue with registration
        Err(e) => {
            error!("Failed to check existing user: {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Database error".to_string())));
        }
    }

    // Check if email already exists
    match state
        .storage
        .database
        .find_user_by_email(&request.email)
        .await
    {
        Ok(Some(_)) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                DUPLICATE_CREDENTIALS_ERROR.to_string(),
            )));
        }
        Ok(None) => {} // Continue with registration
        Err(e) => {
            error!("Failed to check existing email: {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Database error".to_string())));
        }
    }

    // Hash password
    let password_hash = match hash_password(&request.password) {
        Ok(hash) => hash,
        Err(e) => {
            error!("Failed to hash password: {}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Password hashing failed".to_string(),
                )),
            );
        }
    };

    // Create user
    let user = crate::core::models::user::types::User::new(
        request.username.clone(),
        request.email.clone(),
        password_hash,
    );

    // Store user in database
    match state.storage.database.create_user(&user).await {
        Ok(created_user) => {
            info!("User registered successfully: {}", created_user.username);

            let response = RegisterResponse {
                user_id: created_user.id(),
                username: created_user.username,
                email: created_user.email,
                message: "Registration successful. Please verify your email.".to_string(),
            };

            Ok(HttpResponse::Created().json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to create user: {}", e);
            Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("User creation failed".to_string())))
        }
    }
}
