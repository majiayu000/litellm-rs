//! Session management endpoints

use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use actix_web::http::header::HeaderMap;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use tracing::{info, warn};

/// User logout endpoint
pub async fn logout(state: web::Data<AppState>, req: HttpRequest) -> ActixResult<HttpResponse> {
    info!("User logout");

    // Extract session token from headers or cookies
    if let Some(session_token) = extract_session_token(req.headers())
        && let Err(e) = state.auth.logout(&session_token).await
    {
        warn!("Failed to logout user: {}", e);
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(())))
}

/// Extract session token from headers
pub fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    // Check Authorization header
    if let Some(auth_header) = headers.get("authorization")
        && let Ok(auth_str) = auth_header.to_str()
        && let Some(stripped) = auth_str.strip_prefix("Session ")
    {
        return Some(stripped.to_string());
    }

    // Check session cookie
    if let Some(cookie_header) = headers.get("cookie")
        && let Ok(cookie_str) = cookie_header.to_str()
    {
        for cookie in cookie_str.split(';') {
            let cookie = cookie.trim();
            if let Some(stripped) = cookie.strip_prefix("session=") {
                return Some(stripped.to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header::{HeaderName, HeaderValue};

    // ==================== extract_session_token from Cookie Tests ====================

    #[test]
    fn test_extract_session_token_from_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static("session=abc123; other=value"),
        );

        let token = extract_session_token(&headers);
        assert_eq!(token, Some("abc123".to_string()));
    }

    #[test]
    fn test_extract_session_token_from_cookie_only() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static("session=token123"),
        );

        let token = extract_session_token(&headers);
        assert_eq!(token, Some("token123".to_string()));
    }

    #[test]
    fn test_extract_session_token_from_cookie_first() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static("session=first; session=second"),
        );

        let token = extract_session_token(&headers);
        assert_eq!(token, Some("first".to_string()));
    }

    #[test]
    fn test_extract_session_token_from_cookie_with_spaces() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static("  session=spaced  ; other=value"),
        );

        let token = extract_session_token(&headers);
        assert_eq!(token, Some("spaced".to_string()));
    }

    #[test]
    fn test_extract_session_token_no_session_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static("other=value; another=thing"),
        );

        let token = extract_session_token(&headers);
        assert!(token.is_none());
    }

    // ==================== extract_session_token from Authorization Tests ====================

    #[test]
    fn test_extract_session_token_from_authorization() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_static("Session xyz789"),
        );

        let token = extract_session_token(&headers);
        assert_eq!(token, Some("xyz789".to_string()));
    }

    #[test]
    fn test_extract_session_token_from_authorization_with_long_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_static("Session eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"),
        );

        let token = extract_session_token(&headers);
        assert!(token.is_some());
        assert!(token.unwrap().starts_with("eyJhbGci"));
    }

    #[test]
    fn test_extract_session_token_wrong_authorization_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_static("Bearer xyz789"),
        );

        let token = extract_session_token(&headers);
        assert!(token.is_none());
    }

    #[test]
    fn test_extract_session_token_basic_auth() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_static("Basic dXNlcjpwYXNz"),
        );

        let token = extract_session_token(&headers);
        assert!(token.is_none());
    }

    // ==================== Priority Tests ====================

    #[test]
    fn test_extract_session_token_prefers_authorization_over_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_static("Session auth_token"),
        );
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static("session=cookie_token"),
        );

        let token = extract_session_token(&headers);
        // Authorization header is checked first
        assert_eq!(token, Some("auth_token".to_string()));
    }

    // ==================== Empty/Missing Tests ====================

    #[test]
    fn test_extract_session_token_empty_headers() {
        let headers = HeaderMap::new();
        let token = extract_session_token(&headers);
        assert!(token.is_none());
    }

    #[test]
    fn test_extract_session_token_empty_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static(""),
        );

        let token = extract_session_token(&headers);
        assert!(token.is_none());
    }

    #[test]
    fn test_extract_session_token_empty_authorization() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_static(""),
        );

        let token = extract_session_token(&headers);
        assert!(token.is_none());
    }

    #[test]
    fn test_extract_session_token_session_prefix_only() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_static("Session "),
        );

        let token = extract_session_token(&headers);
        // Returns empty string after "Session "
        assert_eq!(token, Some("".to_string()));
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_extract_session_token_special_characters() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_static("Session abc-123_xyz.456"),
        );

        let token = extract_session_token(&headers);
        assert_eq!(token, Some("abc-123_xyz.456".to_string()));
    }

    #[test]
    fn test_extract_session_token_case_sensitive() {
        let mut headers = HeaderMap::new();
        // "session" in cookie is lowercase as expected
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static("Session=abc123"),
        );

        let token = extract_session_token(&headers);
        // Cookie key is case-sensitive, "Session" != "session"
        assert!(token.is_none());
    }

    #[test]
    fn test_extract_session_token_multiple_cookies() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static("a=1; b=2; session=target; c=3"),
        );

        let token = extract_session_token(&headers);
        assert_eq!(token, Some("target".to_string()));
    }
}
