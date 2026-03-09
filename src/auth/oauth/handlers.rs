//! HTTP handlers for OAuth endpoints

use super::client::OAuthClient;
use super::config::OAuthGatewayConfig;
use super::session::{OAuthSession, SessionStore};
use super::types::{CallbackParams, LogoutRequest, RefreshRequest, UserInfo};
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// OAuth state shared across handlers
#[derive(Clone)]
pub struct OAuthState {
    /// OAuth gateway configuration
    pub config: Arc<OAuthGatewayConfig>,

    /// OAuth clients by provider name
    pub clients: Arc<HashMap<String, OAuthClient>>,

    /// Session store
    pub session_store: Arc<dyn SessionStore>,
}

impl std::fmt::Debug for OAuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthState")
            .field("providers", &self.clients.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl OAuthState {
    /// Create a new OAuth state
    pub fn new(
        config: OAuthGatewayConfig,
        session_store: Arc<dyn SessionStore>,
    ) -> Result<Self, String> {
        let mut clients = HashMap::new();

        for (name, provider_config) in &config.providers {
            if provider_config.enabled {
                let client = OAuthClient::new(provider_config.clone())
                    .map_err(|e| format!("Failed to create OAuth client for {}: {}", name, e))?;
                clients.insert(name.clone(), client);
            }
        }

        Ok(Self {
            config: Arc::new(config),
            clients: Arc::new(clients),
            session_store,
        })
    }

    /// Get a client by provider name
    pub fn get_client(&self, provider: &str) -> Option<&OAuthClient> {
        self.clients.get(provider)
    }
}

/// Response for OAuth login initiation (authorization URL + CSRF state)
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthLoginResponse {
    /// Authorization URL to redirect the user to
    pub authorization_url: String,

    /// State parameter for CSRF validation
    pub state: String,
}

/// Response for successful authentication
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    /// Session ID
    pub session_id: String,

    /// Access token for API calls
    pub access_token: String,

    /// Refresh token (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Token type
    pub token_type: String,

    /// Expiration time in seconds
    pub expires_in: u64,

    /// User information
    pub user: UserInfo,
}

/// OAuth error response (follows OAuth 2.0 spec format)
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthErrorResponse {
    /// Error code
    pub error: String,

    /// Error description
    pub error_description: String,
}

impl OAuthErrorResponse {
    pub fn new(error: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            error_description: description.into(),
        }
    }
}

/// Query parameters for login endpoint
#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    /// Post-login redirect URI
    #[serde(default)]
    pub redirect_uri: Option<String>,

    /// Login hint (email)
    #[serde(default)]
    pub login_hint: Option<String>,

    /// Prompt type
    #[serde(default)]
    pub prompt: Option<String>,
}

/// Returns `true` when `redirect_uri` is safe to follow.
///
/// If `allowed_origins` is empty the whitelist is disabled (permissive).
/// Otherwise the URI's origin (scheme + host + optional port) must exactly
/// match one of the entries in `allowed_origins`.
fn is_redirect_origin_allowed(redirect_uri: &str, allowed_origins: &[String]) -> bool {
    if allowed_origins.is_empty() {
        return true;
    }
    let Ok(parsed) = url::Url::parse(redirect_uri) else {
        return false;
    };
    let origin = match parsed.port() {
        Some(port) => format!(
            "{}://{}:{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or(""),
            port
        ),
        None => format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or("")),
    };
    allowed_origins.iter().any(|allowed| allowed == &origin)
}

/// Configure OAuth routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/oauth")
            .route("/{provider}/login", web::get().to(oauth_login))
            .route("/{provider}/callback", web::get().to(oauth_callback))
            .route("/refresh", web::post().to(oauth_refresh))
            .route("/logout", web::post().to(oauth_logout))
            .route("/userinfo", web::get().to(oauth_userinfo)),
    );
}

/// OAuth login endpoint - redirects to provider's authorization page
///
/// GET /auth/oauth/{provider}/login
pub async fn oauth_login(
    oauth: web::Data<OAuthState>,
    path: web::Path<String>,
    query: web::Query<LoginQuery>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let provider = path.into_inner();
    info!("OAuth login request for provider: {}", provider);

    // Get the OAuth client for this provider
    let client = match oauth.get_client(&provider) {
        Some(c) => c,
        None => {
            warn!("Unknown OAuth provider: {}", provider);
            return Ok(HttpResponse::NotFound().json(OAuthErrorResponse::new(
                "provider_not_found",
                format!("OAuth provider '{}' is not configured", provider),
            )));
        }
    };

    // Generate authorization URL and state
    let (mut auth_url, mut state) = client.get_authorization_url();

    // Store redirect URI in state if provided
    if let Some(redirect) = &query.redirect_uri {
        state = state.with_data("client_redirect", redirect.clone());
    }

    // Add login hint if provided
    if let Some(hint) = &query.login_hint {
        if !auth_url.contains("login_hint=") {
            auth_url = format!(
                "{}&login_hint={}",
                auth_url,
                url::form_urlencoded::byte_serialize(hint.as_bytes()).collect::<String>()
            );
        }
    }

    // Add prompt if provided
    if let Some(prompt) = &query.prompt {
        if !auth_url.contains("prompt=") {
            auth_url = format!(
                "{}&prompt={}",
                auth_url,
                url::form_urlencoded::byte_serialize(prompt.as_bytes()).collect::<String>()
            );
        }
    }

    // Store client info in state
    if let Some(ip) = req.connection_info().peer_addr() {
        state = state.with_data("client_ip", ip.to_string());
    }
    if let Some(ua) = req
        .headers()
        .get("User-Agent")
        .and_then(|h| h.to_str().ok())
    {
        state = state.with_data("user_agent", ua.to_string());
    }

    // Store state for CSRF validation
    if let Err(e) = oauth.session_store.set_state(state.clone()).await {
        error!("Failed to store OAuth state: {:?}", e);
        return Ok(
            HttpResponse::InternalServerError().json(OAuthErrorResponse::new(
                "state_storage_error",
                "Failed to initialize OAuth flow",
            )),
        );
    }

    debug!("Redirecting to OAuth provider: {}", auth_url);

    // Return redirect response
    Ok(HttpResponse::Found()
        .insert_header(("Location", auth_url))
        .finish())
}

/// OAuth callback endpoint - handles provider callback
///
/// GET /auth/oauth/{provider}/callback
pub async fn oauth_callback(
    oauth: web::Data<OAuthState>,
    path: web::Path<String>,
    query: web::Query<CallbackParams>,
) -> ActixResult<HttpResponse> {
    let provider = path.into_inner();
    info!("OAuth callback for provider: {}", provider);

    // Check for OAuth error from provider
    if let Some(error) = &query.error {
        warn!(
            "OAuth error from provider: {} - {}",
            error,
            query
                .error_description
                .as_deref()
                .unwrap_or("No description")
        );
        return Ok(HttpResponse::BadRequest().json(OAuthErrorResponse::new(
            error.clone(),
            query.error_description.clone().unwrap_or_default(),
        )));
    }

    // Get state from query
    let state_id = match &query.state {
        Some(s) => s,
        None => {
            return Ok(HttpResponse::BadRequest().json(OAuthErrorResponse::new(
                "missing_state",
                "State parameter is required",
            )));
        }
    };

    // Retrieve and validate state
    let stored_state = match oauth.session_store.get_and_delete_state(state_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            warn!("OAuth state not found or expired: {}", state_id);
            return Ok(HttpResponse::BadRequest().json(OAuthErrorResponse::new(
                "invalid_state",
                "OAuth state is invalid or has expired",
            )));
        }
        Err(e) => {
            error!("Failed to retrieve OAuth state: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(OAuthErrorResponse::new(
                    "state_retrieval_error",
                    "Failed to validate OAuth state",
                )),
            );
        }
    };

    // Get OAuth client
    let client = match oauth.get_client(&provider) {
        Some(c) => c,
        None => {
            return Ok(HttpResponse::NotFound().json(OAuthErrorResponse::new(
                "provider_not_found",
                format!("OAuth provider '{}' is not configured", provider),
            )));
        }
    };

    // Validate callback parameters
    if let Err(e) = client.validate_callback(&query, &stored_state) {
        warn!("OAuth callback validation failed: {}", e);
        return Ok(HttpResponse::BadRequest()
            .json(OAuthErrorResponse::new("validation_error", e.to_string())));
    }

    // Exchange code for tokens
    let code = match query.code.as_ref() {
        Some(code) => code,
        None => {
            return Ok(HttpResponse::BadRequest().json(OAuthErrorResponse::new(
                "missing_code",
                "Authorization code is required",
            )));
        }
    };
    let token_response = match client.exchange_code(code, &stored_state).await {
        Ok(t) => t,
        Err(e) => {
            error!("Token exchange failed: {}", e);
            return Ok(
                HttpResponse::InternalServerError().json(OAuthErrorResponse::new(
                    "token_exchange_error",
                    "Failed to exchange authorization code for tokens",
                )),
            );
        }
    };

    // Get user info
    let user_info = match client.get_user_info(&token_response.access_token).await {
        Ok(u) => u,
        Err(e) => {
            error!("Failed to get user info: {}", e);
            return Ok(
                HttpResponse::InternalServerError().json(OAuthErrorResponse::new(
                    "userinfo_error",
                    "Failed to retrieve user information",
                )),
            );
        }
    };

    info!(
        "OAuth authentication successful for user: {}",
        user_info.email
    );

    // Create session
    let mut session = OAuthSession::new(
        user_info.clone(),
        token_response.access_token.clone(),
        token_response.expires_in,
        oauth.config.session_ttl_seconds,
    );

    if let Some(rt) = &token_response.refresh_token {
        session = session.with_refresh_token(rt.clone());
    }

    if let Some(it) = &token_response.id_token {
        session = session.with_id_token(it.clone());
    }

    // Add client info from stored state
    session = session.with_client_info(
        stored_state.extra_data.get("client_ip").cloned(),
        stored_state.extra_data.get("user_agent").cloned(),
    );

    // Apply role mapping
    if !oauth.config.default_role.is_empty() {
        session = session.with_role(&oauth.config.default_role);
    }

    let session_id = session.session_id.clone();

    // Store session
    if let Err(e) = oauth.session_store.set(session).await {
        error!("Failed to store session: {:?}", e);
        return Ok(
            HttpResponse::InternalServerError().json(OAuthErrorResponse::new(
                "session_storage_error",
                "Failed to create session",
            )),
        );
    }

    // Check if we need to redirect to a client URL
    if let Some(redirect) = stored_state.extra_data.get("client_redirect") {
        let allowed = &oauth.config.allowed_redirect_origins;
        if is_redirect_origin_allowed(redirect, allowed) {
            let redirect_url = if redirect.contains('?') {
                format!("{}&session_id={}", redirect, session_id)
            } else {
                format!("{}?session_id={}", redirect, session_id)
            };
            return Ok(HttpResponse::Found()
                .insert_header(("Location", redirect_url))
                .finish());
        } else {
            warn!(
                "OAuth callback: client_redirect '{}' not in allowed_redirect_origins; falling back to /",
                redirect
            );
            return Ok(HttpResponse::Found()
                .insert_header(("Location", "/"))
                .finish());
        }
    }

    // Return auth response
    Ok(HttpResponse::Ok().json(AuthResponse {
        session_id,
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token,
        token_type: token_response.token_type,
        expires_in: token_response.expires_in,
        user: user_info,
    }))
}

/// Refresh token endpoint
///
/// POST /auth/oauth/refresh
pub async fn oauth_refresh(
    oauth: web::Data<OAuthState>,
    body: web::Json<RefreshRequest>,
) -> ActixResult<HttpResponse> {
    debug!("OAuth token refresh request");

    // Find session with this refresh token
    // Note: In production, you'd want to index sessions by refresh token
    // For now, we'll require the session_id in the request

    // Try to determine which provider to use based on stored session
    // This is a simplified implementation - production would be more robust

    // For now, try each provider until one works
    for (provider_name, client) in oauth.clients.iter() {
        match client.refresh_token(&body.refresh_token).await {
            Ok(token_response) => {
                info!("Token refresh successful with provider: {}", provider_name);
                return Ok(HttpResponse::Ok().json(AuthResponse {
                    session_id: String::new(), // Would need session lookup
                    access_token: token_response.access_token,
                    refresh_token: token_response.refresh_token,
                    token_type: token_response.token_type,
                    expires_in: token_response.expires_in,
                    user: UserInfo::new("", "", provider_name), // Would need session lookup
                }));
            }
            Err(e) => {
                debug!("Refresh failed with provider {}: {}", provider_name, e);
            }
        }
    }

    Ok(HttpResponse::Unauthorized().json(OAuthErrorResponse::new(
        "refresh_failed",
        "Failed to refresh token with any provider",
    )))
}

/// Logout endpoint
///
/// POST /auth/oauth/logout
pub async fn oauth_logout(
    oauth: web::Data<OAuthState>,
    body: web::Json<LogoutRequest>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    info!("OAuth logout request");

    // Get session ID from header or body
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
        });

    // Delete session if we have a session ID
    if let Some(sid) = session_id {
        if let Err(e) = oauth.session_store.delete(&sid).await {
            warn!("Failed to delete session during logout: {:?}", e);
        } else {
            debug!("Session deleted: {}", sid);
        }
    }

    // Revoke tokens if provided
    // Note: Not all providers support token revocation

    // Build logout redirect URL if available
    let logout_redirect = body.post_logout_redirect_uri.clone();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Logged out successfully",
        "redirect_url": logout_redirect
    })))
}

/// Get current user info
///
/// GET /auth/oauth/userinfo
pub async fn oauth_userinfo(
    oauth: web::Data<OAuthState>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    debug!("OAuth userinfo request");

    // Get session ID from header
    let session_id = req
        .headers()
        .get("X-Session-ID")
        .and_then(|h| h.to_str().ok())
        .or_else(|| {
            req.headers()
                .get("Authorization")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
        });

    let session_id = match session_id {
        Some(sid) => sid,
        None => {
            return Ok(HttpResponse::Unauthorized().json(OAuthErrorResponse::new(
                "missing_session",
                "Session ID is required",
            )));
        }
    };

    // Get session
    let session = match oauth.session_store.get(session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Ok(HttpResponse::Unauthorized().json(OAuthErrorResponse::new(
                "invalid_session",
                "Session not found or expired",
            )));
        }
        Err(e) => {
            error!("Failed to retrieve session: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(OAuthErrorResponse::new(
                    "session_error",
                    "Failed to retrieve session",
                )),
            );
        }
    };

    Ok(HttpResponse::Ok().json(session.user_info))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::oauth::config::OAuthConfig;
    use crate::auth::oauth::session::InMemorySessionStore;

    fn create_test_config() -> OAuthGatewayConfig {
        let mut config = OAuthGatewayConfig::default();
        config.add_provider(
            "google",
            OAuthConfig::google("test_client_id", "https://app.example.com/callback")
                .with_client_secret("test_secret"),
        );
        config
    }

    #[test]
    fn test_oauth_state_creation() {
        let config = create_test_config();
        let session_store = Arc::new(InMemorySessionStore::new());
        let state = OAuthState::new(config, session_store);
        assert!(state.is_ok());

        let state = state.unwrap();
        assert!(state.get_client("google").is_some());
        assert!(state.get_client("unknown").is_none());
    }

    #[test]
    fn test_login_response_serialization() {
        let response = OAuthLoginResponse {
            authorization_url: "https://auth.example.com".to_string(),
            state: "state123".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("authorization_url"));
        assert!(json.contains("state123"));
    }

    #[test]
    fn test_auth_response_serialization() {
        let response = AuthResponse {
            session_id: "session123".to_string(),
            access_token: "access_token".to_string(),
            refresh_token: Some("refresh_token".to_string()),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            user: UserInfo::new("123", "test@example.com", "google"),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("session123"));
        assert!(json.contains("access_token"));
        assert!(json.contains("test@example.com"));
    }

    #[test]
    fn test_error_response() {
        let error = OAuthErrorResponse::new("invalid_grant", "The authorization code has expired");
        assert_eq!(error.error, "invalid_grant");
        assert!(error.error_description.contains("expired"));
    }

    #[test]
    fn test_login_query_deserialization() {
        let json =
            r#"{"redirect_uri": "https://app.example.com", "login_hint": "user@example.com"}"#;
        let query: LoginQuery = serde_json::from_str(json).unwrap();
        assert_eq!(
            query.redirect_uri,
            Some("https://app.example.com".to_string())
        );
        assert_eq!(query.login_hint, Some("user@example.com".to_string()));
    }

    #[test]
    fn test_oauth_state_debug() {
        let config = create_test_config();
        let session_store = Arc::new(InMemorySessionStore::new());
        let state = OAuthState::new(config, session_store).unwrap();
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("OAuthState"));
        assert!(debug_str.contains("google"));
    }
}
