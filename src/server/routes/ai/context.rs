//! Request context and authentication helpers

use crate::core::models::ApiKey;
use crate::core::models::user::types::User;
use crate::core::types::context::RequestContext;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, ResponseError, Result as ActixResult};
use serde::Serialize;
use std::future::Future;
use tracing::{debug, error};

/// Get request context from headers and middleware extensions
pub fn get_request_context(req: &HttpRequest) -> ActixResult<RequestContext> {
    if let Some(context) = req.extensions().get::<RequestContext>() {
        return Ok(context.clone());
    }

    let mut context = RequestContext::new();

    // Extract request ID
    if let Some(request_id) = req.headers().get("x-request-id")
        && let Ok(id) = request_id.to_str()
    {
        context.request_id = id.to_string();
    }

    // Extract user agent
    if let Some(user_agent) = req.headers().get("user-agent")
        && let Ok(agent) = user_agent.to_str()
    {
        context.user_agent = Some(agent.to_string());
    }

    context.client_ip = req.connection_info().peer_addr().map(|ip| ip.to_string());

    Ok(context)
}

/// Extract user from request extensions
pub fn get_authenticated_user(req: &HttpRequest) -> Option<User> {
    req.extensions().get::<User>().cloned()
}

/// Extract API key from request extensions
pub fn get_authenticated_api_key(req: &HttpRequest) -> Option<ApiKey> {
    req.extensions().get::<ApiKey>().cloned()
}

/// Check if user has permission for the requested operation
pub fn check_permission(user: Option<&User>, api_key: Option<&ApiKey>, _operation: &str) -> bool {
    // In a real implementation, this would check permissions through the RBAC system
    // For now, assume all authenticated requests are allowed
    user.is_some() || api_key.is_some()
}

/// Log API usage for billing and analytics
pub async fn log_api_usage(context: &RequestContext, model: &str, tokens_used: u32, cost: f64) {
    // In a real implementation, this would log usage to the database
    debug!(
        "API usage: user_id={:?}, model={}, tokens={}, cost={}",
        context.user_id, model, tokens_used, cost
    );
}

/// Common handler for JSON AI requests: extract context → call handler → json or error response.
///
/// Eliminates the repeated pattern of:
/// ```ignore
/// let context = get_request_context(&req)?;
/// match handler(request.into_inner(), context).await {
///     Ok(r) => Ok(HttpResponse::Ok().json(r)),
///     Err(e) => { error!("..."); Ok(e.error_response()) }
/// }
/// ```
pub async fn handle_ai_request<Req, Resp, F, Fut>(
    req: &HttpRequest,
    request: Req,
    error_label: &str,
    handler: F,
) -> ActixResult<HttpResponse>
where
    Resp: Serialize,
    F: FnOnce(Req, RequestContext) -> Fut,
    Fut: Future<Output = Result<Resp, GatewayError>>,
{
    let context = get_request_context(req)?;
    match handler(request, context).await {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(e) => {
            error!("{} error: {}", error_label, e);
            Ok(e.error_response())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::user::types::User;
    use crate::core::models::{Metadata, UsageStats};

    fn create_test_user() -> User {
        User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "hash".to_string(),
        )
    }

    fn create_test_api_key() -> ApiKey {
        ApiKey {
            metadata: Metadata::new(),
            name: "test-key".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "sk-test".to_string(),
            user_id: None,
            team_id: None,
            permissions: vec![],
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        }
    }

    // ==================== check_permission Tests ====================

    #[test]
    fn test_check_permission_no_auth() {
        assert!(!check_permission(None, None, "chat"));
    }

    #[test]
    fn test_check_permission_with_user() {
        let user = create_test_user();
        assert!(check_permission(Some(&user), None, "chat"));
    }

    #[test]
    fn test_check_permission_with_api_key() {
        let api_key = create_test_api_key();
        assert!(check_permission(None, Some(&api_key), "chat"));
    }

    #[test]
    fn test_check_permission_with_both() {
        let user = create_test_user();
        let api_key = create_test_api_key();
        assert!(check_permission(Some(&user), Some(&api_key), "chat"));
    }

    #[test]
    fn test_check_permission_various_operations() {
        let user = create_test_user();
        assert!(check_permission(Some(&user), None, "chat"));
        assert!(check_permission(Some(&user), None, "completions"));
        assert!(check_permission(Some(&user), None, "embeddings"));
        assert!(check_permission(Some(&user), None, "images"));
    }

    // ==================== get_authenticated_user Tests ====================

    #[test]
    fn test_get_authenticated_user_returns_none() {
        let req = actix_web::test::TestRequest::default().to_http_request();
        assert!(get_authenticated_user(&req).is_none());
    }

    // ==================== get_authenticated_api_key Tests ====================

    #[test]
    fn test_get_authenticated_api_key_returns_none() {
        let req = actix_web::test::TestRequest::default().to_http_request();
        assert!(get_authenticated_api_key(&req).is_none());
    }

    // ==================== log_api_usage Tests ====================

    #[tokio::test]
    async fn test_log_api_usage() {
        let context = RequestContext::new();
        log_api_usage(&context, "gpt-4", 100, 0.002).await;
        // Function should not panic
    }

    #[tokio::test]
    async fn test_log_api_usage_various_models() {
        let context = RequestContext::new();
        log_api_usage(&context, "gpt-3.5-turbo", 50, 0.001).await;
        log_api_usage(&context, "claude-3-opus", 200, 0.005).await;
        log_api_usage(&context, "gemini-pro", 75, 0.0015).await;
    }

    #[tokio::test]
    async fn test_log_api_usage_zero_tokens() {
        let context = RequestContext::new();
        log_api_usage(&context, "gpt-4", 0, 0.0).await;
    }

    #[tokio::test]
    async fn test_log_api_usage_large_values() {
        let context = RequestContext::new();
        log_api_usage(&context, "gpt-4", 100000, 100.0).await;
    }

    #[tokio::test]
    async fn test_log_api_usage_with_user_id() {
        let mut context = RequestContext::new();
        context.user_id = Some(uuid::Uuid::new_v4().to_string());
        log_api_usage(&context, "gpt-4", 100, 0.002).await;
    }

    // ==================== RequestContext Tests ====================

    #[test]
    fn test_request_context_new() {
        let context = RequestContext::new();
        assert!(context.user_id.is_none());
    }
}
