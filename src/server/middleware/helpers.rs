//! Helper functions for middleware

use crate::auth::AuthMethod;
use actix_web::http::header::HeaderMap;
use actix_web::http::header::HeaderName;

/// Extract authentication method from headers
pub fn extract_auth_method(headers: &HeaderMap) -> AuthMethod {
    extract_auth_method_with_api_key_header(headers, "x-api-key")
}

/// Extract authentication method from headers using a configured API key header.
///
/// Priority:
/// 1. `Authorization` header (`Bearer <jwt>`, `ApiKey <key>`, or raw `gw-...`)
/// 2. Configured API key header
/// 3. `X-API-Key` (backward-compatible fallback)
/// 4. `session=` cookie
pub fn extract_auth_method_with_api_key_header(
    headers: &HeaderMap,
    api_key_header: &str,
) -> AuthMethod {
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

    // Check configured API key header (unless it overlaps with Authorization/Cookie).
    if let Some(key) = get_header_value(headers, api_key_header) {
        return AuthMethod::ApiKey(key);
    }

    // Backward-compatible fallback.
    if !api_key_header.eq_ignore_ascii_case("x-api-key")
        && let Some(key) = get_header_value(headers, "x-api-key")
    {
        return AuthMethod::ApiKey(key);
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

fn get_header_value(headers: &HeaderMap, header_name: &str) -> Option<String> {
    let trimmed = header_name.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("authorization")
        || trimmed.eq_ignore_ascii_case("cookie")
    {
        return None;
    }

    let header_name = HeaderName::try_from(trimmed).ok()?;
    headers
        .get(&header_name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

/// Check if a route is public (doesn't require authentication)
pub fn is_public_route(path: &str) -> bool {
    const PUBLIC_ROUTES: &[&str] = &[
        "/health",
        "/auth/login",
        "/auth/login/callback",
        "/auth/register",
        "/auth/forgot-password",
        "/auth/reset-password",
        "/auth/verify-email",
        "/docs",
        "/openapi.json",
    ];

    PUBLIC_ROUTES.contains(&path)
}

/// Check if a route requires admin privileges
pub fn is_admin_route(path: &str) -> bool {
    const ADMIN_ROUTES: &[&str] = &["/admin", "/api/admin"];

    ADMIN_ROUTES
        .iter()
        .any(|&route| path == route || path.starts_with(&format!("{route}/")))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_admin_route_exact_match() {
        assert!(is_admin_route("/admin"));
        assert!(is_admin_route("/api/admin"));
    }

    #[test]
    fn test_is_admin_route_sub_paths() {
        assert!(is_admin_route("/admin/users"));
        assert!(is_admin_route("/admin/settings"));
        assert!(is_admin_route("/api/admin/users"));
    }

    #[test]
    fn test_is_admin_route_no_prefix_confusion() {
        // Regression: bare starts_with() without separator allowed these
        assert!(!is_admin_route("/adminevil"));
        assert!(!is_admin_route("/api/adminevil"));
        assert!(!is_admin_route("/administrator"));
        assert!(!is_admin_route("/api/adminsecret"));
    }

    #[test]
    fn test_is_admin_route_unrelated_paths() {
        assert!(!is_admin_route("/health"));
        assert!(!is_admin_route("/v1/chat/completions"));
        assert!(!is_admin_route("/auth/login"));
    }
}
