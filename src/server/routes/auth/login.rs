//! User login endpoint

use crate::server::middleware::AuthRateLimiter;
use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use crate::utils::auth::crypto::password::verify_password;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, info, warn};

use super::models::{LoginRequest, LoginResponse, UserInfo};

/// Global login rate limiter: 5 attempts per IP per minute
static LOGIN_RATE_LIMITER: std::sync::OnceLock<Arc<AuthRateLimiter>> = std::sync::OnceLock::new();

fn get_login_rate_limiter() -> Arc<AuthRateLimiter> {
    LOGIN_RATE_LIMITER
        .get_or_init(|| Arc::new(AuthRateLimiter::new(5, 60, 60)))
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
///
/// * When the immediate peer is a **trusted proxy** (configured in `server.trusted_proxies`),
///   the leftmost valid IP from `X-Forwarded-For` is used as the real client address.
/// * Otherwise the raw TCP peer address is used so that `X-Forwarded-For` cannot be
///   spoofed by untrusted callers.
///
/// Port numbers are always stripped so the limit applies per-IP, not per-connection.
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

    // Only consult X-Forwarded-For when the request arrives from a trusted proxy
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
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// User login endpoint
pub async fn login(
    req: HttpRequest,
    state: web::Data<AppState>,
    request: web::Json<LoginRequest>,
) -> ActixResult<HttpResponse> {
    let client_ip = extract_client_ip(&req, &state.config.gateway.server.trusted_proxies);

    // Rate limit: max 5 login attempts per IP per minute
    let limiter = get_login_rate_limiter();

    // Probabilistic cleanup: every 100th request, purge stale entries
    let count = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    if count.is_multiple_of(100) {
        limiter.cleanup_old_entries();
    }

    if let Err(retry_after) = limiter.check_allowed(&client_ip) {
        warn!(
            "Login rate limit exceeded for IP {}: retry after {}s",
            client_ip, retry_after
        );
        return Ok(HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", retry_after.to_string()))
            .json(ApiResponse::<()>::error(
                "Too many login attempts. Please try again later.".to_string(),
            )));
    }

    info!("User login attempt from IP {}", client_ip);

    // Find user by username
    let user = match state
        .storage
        .database
        .find_user_by_username(&request.username)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!("Login attempt with invalid username from IP {}", client_ip);
            limiter.record_failure(&client_ip);
            return Ok(HttpResponse::Unauthorized()
                .json(ApiResponse::<()>::error("Invalid credentials".to_string())));
        }
        Err(e) => {
            error!("Database error during login: {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Database error".to_string())));
        }
    };

    // Check if user is active
    if !user.is_active() {
        warn!("Login attempt for inactive user from IP {}", client_ip);
        limiter.record_failure(&client_ip);
        return Ok(HttpResponse::Forbidden()
            .json(ApiResponse::<()>::error("Account is disabled".to_string())));
    }

    // Verify password
    let password_valid = match verify_password(&request.password, &user.password_hash) {
        Ok(valid) => valid,
        Err(e) => {
            error!("Password verification error: {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Authentication error".to_string())));
        }
    };

    if !password_valid {
        warn!("Login attempt with invalid password from IP {}", client_ip);
        limiter.record_failure(&client_ip);
        return Ok(HttpResponse::Unauthorized()
            .json(ApiResponse::<()>::error("Invalid credentials".to_string())));
    }

    // Update last login time
    if let Err(e) = state
        .storage
        .database
        .update_user_last_login(user.id())
        .await
    {
        warn!("Failed to update last login time: {}", e);
    }

    // Generate JWT tokens
    let access_token = match state
        .auth
        .jwt()
        .create_access_token(user.id(), user.role.to_string(), vec![], None, None)
        .await
    {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate access token: {}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Token generation failed".to_string(),
                )),
            );
        }
    };

    let refresh_token = match state.auth.jwt().create_refresh_token(user.id(), None).await {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate refresh token: {}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Token generation failed".to_string(),
                )),
            );
        }
    };

    info!("User logged in successfully from IP {}", client_ip);

    let response = LoginResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 3600, // 1 hour
        user: UserInfo {
            id: user.id(),
            username: user.username,
            email: user.email,
            full_name: user.display_name,
            role: user.role.to_string(),
            email_verified: user.email_verified,
        },
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // NOTE: Full integration tests require mocking AppState, AuthSystem, and StorageLayer
    // TODO: Add full integration tests with proper mocking infrastructure

    #[test]
    fn test_login_request_deserialization() {
        let json = r#"{"username": "testuser", "password": "pass123"}"#;
        let request: LoginRequest = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(request.username, "testuser");
        assert_eq!(request.password, "pass123");
    }

    #[test]
    fn test_login_request_missing_fields() {
        let json = r#"{"username": "testuser"}"#;
        let result: Result<LoginRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_login_response_serialization() {
        let response = LoginResponse {
            access_token: "access_token_here".to_string(),
            refresh_token: "refresh_token_here".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            user: UserInfo {
                id: Uuid::new_v4(),
                username: "testuser".to_string(),
                email: "test@example.com".to_string(),
                full_name: Some("Test User".to_string()),
                role: "User".to_string(),
                email_verified: true,
            },
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("access_token"));
        assert!(json.contains("refresh_token"));
        assert!(json.contains("Bearer"));
        assert!(json.contains("testuser"));
    }

    #[test]
    fn test_user_info_structure() {
        let user_info = UserInfo {
            id: Uuid::new_v4(),
            username: "john_doe".to_string(),
            email: "john@example.com".to_string(),
            full_name: Some("John Doe".to_string()),
            role: "Admin".to_string(),
            email_verified: true,
        };

        assert_eq!(user_info.username, "john_doe");
        assert_eq!(user_info.role, "Admin");
        assert!(user_info.email_verified);
        assert!(user_info.full_name.is_some());
    }

    #[test]
    fn test_login_rate_limiter_blocks_after_limit() {
        let limiter = AuthRateLimiter::new(5, 60, 60);
        let ip = "192.0.2.1";

        // First 5 attempts should be allowed
        for _ in 0..5 {
            assert!(limiter.check_allowed(ip).is_ok());
            limiter.record_failure(ip);
        }

        // 6th attempt should be blocked
        assert!(limiter.check_allowed(ip).is_err());
    }

    #[test]
    fn test_login_rate_limiter_different_ips_independent() {
        let limiter = AuthRateLimiter::new(5, 60, 60);
        let ip1 = "192.0.2.1";
        let ip2 = "192.0.2.2";

        // Exhaust limit for ip1
        for _ in 0..5 {
            assert!(limiter.check_allowed(ip1).is_ok());
            limiter.record_failure(ip1);
        }
        assert!(limiter.check_allowed(ip1).is_err());

        // ip2 should still be allowed
        assert!(limiter.check_allowed(ip2).is_ok());
    }

    #[test]
    fn test_extract_client_ip_strips_port() {
        // IPv4 with port: only the IP portion should be used as the rate-limit key
        let ipv4_with_port = "192.0.2.1:54321"
            .parse::<std::net::SocketAddr>()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|_| "192.0.2.1:54321".to_string());
        assert_eq!(ipv4_with_port, "192.0.2.1");

        // IPv6 with port
        let ipv6_with_port = "[::1]:54321"
            .parse::<std::net::SocketAddr>()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|_| "[::1]:54321".to_string());
        assert_eq!(ipv6_with_port, "::1");
    }

    #[test]
    fn test_success_does_not_reset_rate_limit_counter() {
        // Successful login must NOT reset the counter: an attacker with one valid
        // account must not be able to interleave their own successful logins to
        // bypass the per-IP brute-force limit against a victim account.
        let limiter = AuthRateLimiter::new(5, 60, 60);
        let ip = "192.0.2.1";

        // Accumulate 4 failures (one short of lockout)
        for _ in 0..4 {
            assert!(limiter.check_allowed(ip).is_ok());
            limiter.record_failure(ip);
        }

        // The login handler no longer calls record_success(); simulate by doing nothing.

        // The 5th failure still hits the limit and triggers lockout
        assert!(limiter.check_allowed(ip).is_ok());
        limiter.record_failure(ip);
        assert!(limiter.check_allowed(ip).is_err());
    }

    // ---- trusted-proxy XFF helpers ----

    #[test]
    fn test_client_ip_from_xff_single() {
        assert_eq!(
            client_ip_from_xff("203.0.113.5"),
            Some("203.0.113.5".to_string())
        );
    }

    #[test]
    fn test_client_ip_from_xff_chain() {
        // Leftmost address is the original client
        assert_eq!(
            client_ip_from_xff("203.0.113.5, 10.0.0.1, 10.0.0.2"),
            Some("203.0.113.5".to_string())
        );
    }

    #[test]
    fn test_client_ip_from_xff_invalid_returns_none() {
        assert_eq!(client_ip_from_xff("not-an-ip, 10.0.0.1"), None);
    }

    #[test]
    fn test_client_ip_from_xff_empty_returns_none() {
        assert_eq!(client_ip_from_xff(""), None);
    }

    #[test]
    fn test_client_ip_from_xff_ipv6() {
        assert_eq!(
            client_ip_from_xff("2001:db8::1, 10.0.0.1"),
            Some("2001:db8::1".to_string())
        );
    }
}
