//! OAuth middleware for protecting routes and extracting user information

use super::session::{OAuthSession, SessionStore};
use super::types::UserInfo;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::{Error, HttpMessage, HttpRequest, web};
use futures::future::{Ready, ok};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, warn};

/// OAuth middleware for protecting routes
///
/// This middleware validates OAuth sessions and extracts user information
/// from incoming requests. It can be configured to:
/// - Require authentication (reject unauthenticated requests)
/// - Allow anonymous access (pass through if no session)
/// - Apply role-based access control
#[derive(Clone)]
pub struct OAuthMiddleware {
    /// Session store for validating sessions
    session_store: Arc<dyn SessionStore>,

    /// Whether authentication is required
    require_auth: bool,

    /// Required roles (empty means any authenticated user)
    required_roles: Vec<String>,

    /// Routes to exclude from authentication
    exclude_paths: Vec<String>,
}

impl OAuthMiddleware {
    /// Create a new OAuth middleware that requires authentication
    pub fn new(session_store: Arc<dyn SessionStore>) -> Self {
        Self {
            session_store,
            require_auth: true,
            required_roles: Vec::new(),
            exclude_paths: Vec::new(),
        }
    }

    /// Create middleware that allows anonymous access
    pub fn optional(session_store: Arc<dyn SessionStore>) -> Self {
        Self {
            session_store,
            require_auth: false,
            required_roles: Vec::new(),
            exclude_paths: Vec::new(),
        }
    }

    /// Require specific roles
    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.required_roles = roles;
        self
    }

    /// Add a role requirement
    pub fn require_role(mut self, role: impl Into<String>) -> Self {
        self.required_roles.push(role.into());
        self
    }

    /// Exclude paths from authentication
    pub fn exclude_paths(mut self, paths: Vec<String>) -> Self {
        self.exclude_paths = paths;
        self
    }

    /// Add a path to exclude
    pub fn exclude_path(mut self, path: impl Into<String>) -> Self {
        self.exclude_paths.push(path.into());
        self
    }

    /// Check if a path should be excluded from authentication
    fn is_excluded(&self, path: &str) -> bool {
        self.exclude_paths.iter().any(|p| {
            if p.ends_with('*') {
                path.starts_with(p.trim_end_matches('*'))
            } else {
                path == p
            }
        })
    }
}

impl std::fmt::Debug for OAuthMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthMiddleware")
            .field("require_auth", &self.require_auth)
            .field("required_roles", &self.required_roles)
            .field("exclude_paths", &self.exclude_paths)
            .finish()
    }
}

impl<S, B> Transform<S, ServiceRequest> for OAuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = OAuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(OAuthMiddlewareService {
            service,
            session_store: self.session_store.clone(),
            require_auth: self.require_auth,
            required_roles: self.required_roles.clone(),
            exclude_paths: self.exclude_paths.clone(),
        })
    }
}

/// Service implementation for OAuth middleware
pub struct OAuthMiddlewareService<S> {
    service: S,
    session_store: Arc<dyn SessionStore>,
    require_auth: bool,
    required_roles: Vec<String>,
    exclude_paths: Vec<String>,
}

impl<S> OAuthMiddlewareService<S> {
    fn is_excluded(&self, path: &str) -> bool {
        self.exclude_paths.iter().any(|p| {
            if p.ends_with('*') {
                path.starts_with(p.trim_end_matches('*'))
            } else {
                path == p
            }
        })
    }
}

impl<S, B> Service<ServiceRequest> for OAuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_string();

        // Check if path is excluded
        if self.is_excluded(&path) {
            return Box::pin(self.service.call(req));
        }

        // Extract session ID from request
        let session_id = extract_session_id(&req);

        let session_store = self.session_store.clone();
        let require_auth = self.require_auth;
        let required_roles = self.required_roles.clone();
        let fut = self.service.call(req);

        Box::pin(async move {
            match session_id {
                Some(sid) => {
                    // Validate session
                    match session_store.get(&sid).await {
                        Ok(Some(session)) => {
                            // Check role requirements
                            if !required_roles.is_empty() {
                                let has_role = session
                                    .role
                                    .as_ref()
                                    .map(|r| required_roles.contains(r))
                                    .unwrap_or(false);

                                if !has_role {
                                    warn!(
                                        "User {} lacks required role for path {}",
                                        session.user_info.email, path
                                    );
                                    return Err(actix_web::error::ErrorForbidden(
                                        "Insufficient permissions",
                                    ));
                                }
                            }

                            debug!(
                                "OAuth session validated for user: {}",
                                session.user_info.email
                            );

                            // Session is valid, proceed with request
                            fut.await
                        }
                        Ok(None) => {
                            if require_auth {
                                warn!("Invalid or expired session: {}", sid);
                                Err(actix_web::error::ErrorUnauthorized("Invalid session"))
                            } else {
                                fut.await
                            }
                        }
                        Err(e) => {
                            warn!("Session lookup error: {:?}", e);
                            if require_auth {
                                Err(actix_web::error::ErrorInternalServerError(
                                    "Session validation failed",
                                ))
                            } else {
                                fut.await
                            }
                        }
                    }
                }
                None => {
                    if require_auth {
                        debug!("No session provided for protected route: {}", path);
                        Err(actix_web::error::ErrorUnauthorized(
                            "Authentication required",
                        ))
                    } else {
                        fut.await
                    }
                }
            }
        })
    }
}

/// Extract session ID from request headers
fn extract_session_id(req: &ServiceRequest) -> Option<String> {
    // Try X-Session-ID header first
    if let Some(sid) = req
        .headers()
        .get("X-Session-ID")
        .and_then(|h| h.to_str().ok())
    {
        return Some(sid.to_string());
    }

    // Try Authorization header with Bearer token
    if let Some(auth) = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        && let Some(token) = auth.strip_prefix("Bearer ")
    {
        return Some(token.to_string());
    }

    // Try cookie
    if let Some(cookie) = req.cookie("session_id") {
        return Some(cookie.value().to_string());
    }

    None
}

/// Extension trait for extracting OAuth user from request
pub trait OAuthUserExt {
    /// Get the authenticated OAuth user, if any
    fn oauth_user(&self) -> Option<UserInfo>;

    /// Get the OAuth session, if any
    fn oauth_session(&self) -> Option<OAuthSession>;
}

impl OAuthUserExt for HttpRequest {
    fn oauth_user(&self) -> Option<UserInfo> {
        self.extensions().get::<UserInfo>().cloned()
    }

    fn oauth_session(&self) -> Option<OAuthSession> {
        self.extensions().get::<OAuthSession>().cloned()
    }
}

/// Request guard for requiring OAuth authentication
pub struct OAuthUser(pub UserInfo);

impl OAuthUser {
    /// Get the user info
    pub fn user(&self) -> &UserInfo {
        &self.0
    }

    /// Get the user ID
    pub fn id(&self) -> &str {
        &self.0.id
    }

    /// Get the user email
    pub fn email(&self) -> &str {
        &self.0.email
    }

    /// Get the provider
    pub fn provider(&self) -> &str {
        &self.0.provider
    }
}

impl std::ops::Deref for OAuthUser {
    type Target = UserInfo;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Extractor for OAuth user from request
impl actix_web::FromRequest for OAuthUser {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let session_id = req
            .headers()
            .get("X-Session-ID")
            .and_then(|h| h.to_str().ok())
            .map(String::from)
            .or_else(|| {
                req.headers()
                    .get("Authorization")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|h| h.strip_prefix("Bearer "))
                    .map(String::from)
            })
            .or_else(|| req.cookie("session_id").map(|c| c.value().to_string()));

        let oauth_state = req
            .app_data::<web::Data<super::handlers::OAuthState>>()
            .cloned();

        Box::pin(async move {
            let session_id =
                session_id.ok_or_else(|| actix_web::error::ErrorUnauthorized("Missing session"))?;

            let oauth = oauth_state.ok_or_else(|| {
                actix_web::error::ErrorInternalServerError("OAuth not configured")
            })?;

            let session = oauth
                .session_store
                .get(&session_id)
                .await
                .map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Session lookup failed: {:?}",
                        e
                    ))
                })?
                .ok_or_else(|| actix_web::error::ErrorUnauthorized("Invalid session"))?;

            Ok(OAuthUser(session.user_info))
        })
    }
}

/// Extractor for optional OAuth user
pub struct OptionalOAuthUser(pub Option<UserInfo>);

impl OptionalOAuthUser {
    /// Get the user info if present
    pub fn user(&self) -> Option<&UserInfo> {
        self.0.as_ref()
    }

    /// Check if user is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.0.is_some()
    }
}

impl std::ops::Deref for OptionalOAuthUser {
    type Target = Option<UserInfo>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl actix_web::FromRequest for OptionalOAuthUser {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let session_id = req
            .headers()
            .get("X-Session-ID")
            .and_then(|h| h.to_str().ok())
            .map(String::from)
            .or_else(|| {
                req.headers()
                    .get("Authorization")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|h| h.strip_prefix("Bearer "))
                    .map(String::from)
            })
            .or_else(|| req.cookie("session_id").map(|c| c.value().to_string()));

        let oauth_state = req
            .app_data::<web::Data<super::handlers::OAuthState>>()
            .cloned();

        Box::pin(async move {
            let Some(session_id) = session_id else {
                return Ok(OptionalOAuthUser(None));
            };

            let Some(oauth) = oauth_state else {
                return Ok(OptionalOAuthUser(None));
            };

            match oauth.session_store.get(&session_id).await {
                Ok(Some(session)) => Ok(OptionalOAuthUser(Some(session.user_info))),
                Ok(None) => Ok(OptionalOAuthUser(None)),
                Err(_) => Ok(OptionalOAuthUser(None)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::oauth::session::InMemorySessionStore;

    #[test]
    fn test_middleware_creation() {
        let store = Arc::new(InMemorySessionStore::new());
        let middleware = OAuthMiddleware::new(store.clone());

        assert!(middleware.require_auth);
        assert!(middleware.required_roles.is_empty());
        assert!(middleware.exclude_paths.is_empty());
    }

    #[test]
    fn test_middleware_optional() {
        let store = Arc::new(InMemorySessionStore::new());
        let middleware = OAuthMiddleware::optional(store);

        assert!(!middleware.require_auth);
    }

    #[test]
    fn test_middleware_with_roles() {
        let store = Arc::new(InMemorySessionStore::new());
        let middleware = OAuthMiddleware::new(store)
            .require_role("admin")
            .require_role("superuser");

        assert_eq!(middleware.required_roles.len(), 2);
        assert!(middleware.required_roles.contains(&"admin".to_string()));
    }

    #[test]
    fn test_middleware_exclude_paths() {
        let store = Arc::new(InMemorySessionStore::new());
        let middleware = OAuthMiddleware::new(store)
            .exclude_path("/health")
            .exclude_path("/public/*");

        assert!(middleware.is_excluded("/health"));
        assert!(middleware.is_excluded("/public/page"));
        assert!(middleware.is_excluded("/public/nested/page"));
        assert!(!middleware.is_excluded("/api/private"));
    }

    #[test]
    fn test_middleware_debug() {
        let store = Arc::new(InMemorySessionStore::new());
        let middleware = OAuthMiddleware::new(store)
            .require_role("admin")
            .exclude_path("/health");

        let debug_str = format!("{:?}", middleware);
        assert!(debug_str.contains("OAuthMiddleware"));
        assert!(debug_str.contains("require_auth"));
        assert!(debug_str.contains("admin"));
    }

    #[test]
    fn test_oauth_user_extractor() {
        let user = OAuthUser(UserInfo::new("123", "test@example.com", "google"));

        assert_eq!(user.id(), "123");
        assert_eq!(user.email(), "test@example.com");
        assert_eq!(user.provider(), "google");
    }

    #[test]
    fn test_optional_oauth_user() {
        let with_user = OptionalOAuthUser(Some(UserInfo::new("123", "test@example.com", "google")));
        let without_user = OptionalOAuthUser(None);

        assert!(with_user.is_authenticated());
        assert!(with_user.user().is_some());

        assert!(!without_user.is_authenticated());
        assert!(without_user.user().is_none());
    }

    #[test]
    fn test_oauth_user_deref() {
        let user = OAuthUser(UserInfo::new("123", "test@example.com", "google"));

        // Should be able to access UserInfo fields through deref
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.provider, "google");
    }
}
