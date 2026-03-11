//! Helper functions for middleware

use crate::auth::AuthMethod;
use actix_web::http::header::HeaderMap;

/// Extract authentication method from headers
pub fn extract_auth_method(headers: &HeaderMap) -> AuthMethod {
    // Check Authorization header
    if let Some(auth_header) = headers.get("authorization")
        && let Ok(auth_str) = auth_header.to_str()
    {
        if let Some(stripped) = auth_str.strip_prefix("Bearer ") {
            let token = stripped.to_string();
            return AuthMethod::Jwt(token);
        } else if let Some(stripped) = auth_str.strip_prefix("ApiKey ") {
            let key = stripped.to_string();
            return AuthMethod::ApiKey(key);
        } else if auth_str.starts_with("gw-") {
            return AuthMethod::ApiKey(auth_str.to_string());
        }
    }

    // Check X-API-Key header
    if let Some(api_key_header) = headers.get("x-api-key")
        && let Ok(key) = api_key_header.to_str()
    {
        return AuthMethod::ApiKey(key.to_string());
    }

    // Check session cookie
    if let Some(cookie_header) = headers.get("cookie")
        && let Ok(cookie_str) = cookie_header.to_str()
    {
        for cookie in cookie_str.split(';') {
            let cookie = cookie.trim();
            if let Some(stripped) = cookie.strip_prefix("session=") {
                let session_id = stripped.to_string();
                return AuthMethod::Session(session_id);
            }
        }
    }

    AuthMethod::None
}

/// Check if a route is public (doesn't require authentication)
pub fn is_public_route(path: &str) -> bool {
    const PUBLIC_ROUTES: &[&str] = &[
        "/health",
        "/auth/login",
        "/auth/register",
        "/auth/forgot-password",
        "/auth/reset-password",
        "/auth/verify-email",
        "/docs",
        "/openapi.json",
    ];

    PUBLIC_ROUTES
        .iter()
        .any(|&route| path == route || path.starts_with(&format!("{route}/")))
}

/// Check if a route requires admin privileges
pub fn is_admin_route(path: &str) -> bool {
    const ADMIN_ROUTES: &[&str] = &["/admin", "/api/admin"];

    ADMIN_ROUTES.iter().any(|&route| path.starts_with(route))
}

/// Check if a route is for API access
pub fn is_api_route(path: &str) -> bool {
    const API_ROUTES: &[&str] = &[
        "/v1/chat/completions",
        "/v1/embeddings",
        "/v1/images",
        "/v1/audio",
        "/v1/models",
    ];

    API_ROUTES.iter().any(|&route| path.starts_with(route))
}
