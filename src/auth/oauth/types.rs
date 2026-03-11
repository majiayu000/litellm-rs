//! OAuth types and data structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// OAuth 2.0 token response from the authorization server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    /// The access token issued by the authorization server
    pub access_token: String,

    /// The refresh token for obtaining new access tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// The lifetime in seconds of the access token
    pub expires_in: u64,

    /// The type of token (typically "Bearer")
    pub token_type: String,

    /// OAuth 2.0 scopes granted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// ID token (for OIDC flows)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
}

impl Default for TokenResponse {
    fn default() -> Self {
        Self {
            access_token: String::new(),
            refresh_token: None,
            expires_in: 3600,
            token_type: "Bearer".to_string(),
            scope: None,
            id_token: None,
        }
    }
}

/// User information retrieved from the OAuth provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Unique identifier from the OAuth provider
    pub id: String,

    /// User's email address
    pub email: String,

    /// User's display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// URL to the user's profile picture
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,

    /// The OAuth provider that authenticated this user
    pub provider: String,

    /// Whether the email has been verified by the provider
    #[serde(default)]
    pub email_verified: bool,

    /// Additional claims from the provider
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_claims: HashMap<String, serde_json::Value>,
}

impl UserInfo {
    /// Create a new UserInfo with required fields
    pub fn new(
        id: impl Into<String>,
        email: impl Into<String>,
        provider: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            email: email.into(),
            name: None,
            picture: None,
            provider: provider.into(),
            email_verified: false,
            extra_claims: HashMap::new(),
        }
    }

    /// Set the display name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the profile picture URL
    pub fn with_picture(mut self, picture: impl Into<String>) -> Self {
        self.picture = Some(picture.into());
        self
    }

    /// Set the email verified status
    pub fn with_email_verified(mut self, verified: bool) -> Self {
        self.email_verified = verified;
        self
    }

    /// Add an extra claim
    pub fn with_claim(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.extra_claims.insert(key.into(), value);
        self
    }
}

/// OAuth state for CSRF protection during the authorization flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthState {
    /// Unique state parameter for CSRF protection
    pub state: String,

    /// The OAuth provider being used
    pub provider: String,

    /// PKCE code verifier (for public clients)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_verifier: Option<String>,

    /// Original redirect URI from the client
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,

    /// Nonce for OIDC flows
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,

    /// Timestamp when this state was created
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Time-to-live in seconds
    pub ttl_seconds: u64,

    /// Additional data to preserve across the OAuth flow
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_data: HashMap<String, String>,
}

impl OAuthState {
    /// Create a new OAuth state with CSRF protection
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            state: Uuid::new_v4().to_string(),
            provider: provider.into(),
            code_verifier: None,
            redirect_uri: None,
            nonce: None,
            created_at: chrono::Utc::now(),
            ttl_seconds: 600, // 10 minutes default
            extra_data: HashMap::new(),
        }
    }

    /// Create a new OAuth state with PKCE support
    pub fn with_pkce(provider: impl Into<String>) -> Self {
        let code_verifier = generate_pkce_verifier();
        Self {
            state: Uuid::new_v4().to_string(),
            provider: provider.into(),
            code_verifier: Some(code_verifier),
            redirect_uri: None,
            nonce: Some(Uuid::new_v4().to_string()),
            created_at: chrono::Utc::now(),
            ttl_seconds: 600,
            extra_data: HashMap::new(),
        }
    }

    /// Set the redirect URI
    pub fn with_redirect_uri(mut self, uri: impl Into<String>) -> Self {
        self.redirect_uri = Some(uri.into());
        self
    }

    /// Set the TTL
    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.ttl_seconds = ttl_seconds;
        self
    }

    /// Add extra data
    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_data.insert(key.into(), value.into());
        self
    }

    /// Check if the state has expired
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now();
        let expires_at = self.created_at + chrono::Duration::seconds(self.ttl_seconds as i64);
        now > expires_at
    }

    /// Get the PKCE code challenge
    pub fn code_challenge(&self) -> Option<String> {
        self.code_verifier
            .as_ref()
            .map(|v| generate_pkce_challenge(v))
    }
}

/// PKCE code challenge method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PkceChallengeMethod {
    /// Plain text code challenge (not recommended)
    Plain,
    /// SHA-256 hashed code challenge (recommended)
    #[default]
    S256,
}

impl std::fmt::Display for PkceChallengeMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain => write!(f, "plain"),
            Self::S256 => write!(f, "S256"),
        }
    }
}

/// OAuth error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthError {
    /// Error code from the OAuth specification
    pub error: String,

    /// Human-readable error description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,

    /// URI to a web page with more information about the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_uri: Option<String>,
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)?;
        if let Some(desc) = &self.error_description {
            write!(f, ": {}", desc)?;
        }
        Ok(())
    }
}

impl std::error::Error for OAuthError {}

/// Authorization request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationRequest {
    /// OAuth response type (e.g., "code")
    pub response_type: String,

    /// Client identifier
    pub client_id: String,

    /// Redirect URI for the callback
    pub redirect_uri: String,

    /// Requested scopes
    pub scope: String,

    /// State parameter for CSRF protection
    pub state: String,

    /// PKCE code challenge
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_challenge: Option<String>,

    /// PKCE code challenge method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_challenge_method: Option<PkceChallengeMethod>,

    /// Nonce for OIDC
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,

    /// Prompt parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,

    /// Login hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_hint: Option<String>,
}

/// Token exchange request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenExchangeRequest {
    /// Grant type (e.g., "authorization_code", "refresh_token")
    pub grant_type: String,

    /// Authorization code (for authorization_code grant)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// Redirect URI (must match the authorization request)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,

    /// Client ID
    pub client_id: String,

    /// Client secret (for confidential clients)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// PKCE code verifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_verifier: Option<String>,

    /// Refresh token (for refresh_token grant)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Requested scopes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Callback query parameters from OAuth provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallbackParams {
    /// Authorization code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// State parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,

    /// Error code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Error description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
}

/// Logout request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoutRequest {
    /// The refresh token to revoke
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// The access token to revoke
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,

    /// URL to redirect after logout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_logout_redirect_uri: Option<String>,
}

/// Refresh token request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshRequest {
    /// The refresh token
    pub refresh_token: String,

    /// Optional scopes to request (subset of original)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

// PKCE utility functions

/// Generate a cryptographically random PKCE code verifier
fn generate_pkce_verifier() -> String {
    use rand::Rng;

    const VERIFIER_LENGTH: usize = 64;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

    let mut rng = rand::rng();
    (0..VERIFIER_LENGTH)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Generate a PKCE code challenge from a verifier using SHA-256
fn generate_pkce_challenge(verifier: &str) -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    URL_SAFE_NO_PAD.encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_response_default() {
        let response = TokenResponse::default();
        assert!(response.access_token.is_empty());
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in, 3600);
        assert!(response.refresh_token.is_none());
    }

    #[test]
    fn test_token_response_serialization() {
        let response = TokenResponse {
            access_token: "access123".to_string(),
            refresh_token: Some("refresh456".to_string()),
            expires_in: 7200,
            token_type: "Bearer".to_string(),
            scope: Some("openid email".to_string()),
            id_token: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("access123"));
        assert!(json.contains("refresh456"));
        assert!(json.contains("7200"));

        let parsed: TokenResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.access_token, "access123");
        assert_eq!(parsed.refresh_token, Some("refresh456".to_string()));
    }

    #[test]
    fn test_user_info_builder() {
        let user = UserInfo::new("123", "test@example.com", "google")
            .with_name("Test User")
            .with_picture("https://example.com/pic.jpg")
            .with_email_verified(true)
            .with_claim("locale", serde_json::json!("en-US"));

        assert_eq!(user.id, "123");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.provider, "google");
        assert_eq!(user.name, Some("Test User".to_string()));
        assert_eq!(
            user.picture,
            Some("https://example.com/pic.jpg".to_string())
        );
        assert!(user.email_verified);
        assert!(user.extra_claims.contains_key("locale"));
    }

    #[test]
    fn test_user_info_serialization() {
        let user = UserInfo::new("456", "user@example.com", "microsoft");
        let json = serde_json::to_string(&user).unwrap();

        let parsed: UserInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "456");
        assert_eq!(parsed.email, "user@example.com");
        assert_eq!(parsed.provider, "microsoft");
    }

    #[test]
    fn test_oauth_state_creation() {
        let state = OAuthState::new("google");

        assert!(!state.state.is_empty());
        assert_eq!(state.provider, "google");
        assert!(state.code_verifier.is_none());
        assert_eq!(state.ttl_seconds, 600);
        assert!(!state.is_expired());
    }

    #[test]
    fn test_oauth_state_with_pkce() {
        let state = OAuthState::with_pkce("github");

        assert!(state.code_verifier.is_some());
        assert!(state.nonce.is_some());
        assert!(state.code_challenge().is_some());
    }

    #[test]
    fn test_oauth_state_builder() {
        let state = OAuthState::new("okta")
            .with_redirect_uri("https://app.example.com/callback")
            .with_ttl(300)
            .with_data("flow", "login");

        assert_eq!(
            state.redirect_uri,
            Some("https://app.example.com/callback".to_string())
        );
        assert_eq!(state.ttl_seconds, 300);
        assert_eq!(state.extra_data.get("flow"), Some(&"login".to_string()));
    }

    #[test]
    fn test_oauth_state_expiration() {
        let mut state = OAuthState::new("test");
        state.created_at = chrono::Utc::now() - chrono::Duration::seconds(700);
        assert!(state.is_expired());
    }

    #[test]
    fn test_pkce_verifier_generation() {
        let verifier1 = generate_pkce_verifier();
        let verifier2 = generate_pkce_verifier();

        assert_eq!(verifier1.len(), 64);
        assert_eq!(verifier2.len(), 64);
        assert_ne!(verifier1, verifier2);
    }

    #[test]
    fn test_pkce_challenge_generation() {
        let verifier = "test_verifier_string";
        let challenge = generate_pkce_challenge(verifier);

        assert!(!challenge.is_empty());
        // Verify it's base64url encoded
        assert!(!challenge.contains('+'));
        assert!(!challenge.contains('/'));
        assert!(!challenge.contains('='));
    }

    #[test]
    fn test_pkce_challenge_method_display() {
        assert_eq!(PkceChallengeMethod::Plain.to_string(), "plain");
        assert_eq!(PkceChallengeMethod::S256.to_string(), "S256");
    }

    #[test]
    fn test_oauth_error_display() {
        let error = OAuthError {
            error: "invalid_grant".to_string(),
            error_description: Some("The authorization code has expired".to_string()),
            error_uri: None,
        };

        let display = format!("{}", error);
        assert!(display.contains("invalid_grant"));
        assert!(display.contains("authorization code has expired"));
    }

    #[test]
    fn test_callback_params_serialization() {
        let params = CallbackParams {
            code: Some("auth_code_123".to_string()),
            state: Some("state_456".to_string()),
            error: None,
            error_description: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        let parsed: CallbackParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.code, Some("auth_code_123".to_string()));
        assert_eq!(parsed.state, Some("state_456".to_string()));
    }

    #[test]
    fn test_callback_params_with_error() {
        let params = CallbackParams {
            code: None,
            state: Some("state_789".to_string()),
            error: Some("access_denied".to_string()),
            error_description: Some("User denied access".to_string()),
        };

        assert!(params.code.is_none());
        assert!(params.error.is_some());
    }

    #[test]
    fn test_refresh_request() {
        let request = RefreshRequest {
            refresh_token: "refresh_token_abc".to_string(),
            scope: Some("openid email".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: RefreshRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.refresh_token, "refresh_token_abc");
    }

    #[test]
    fn test_logout_request() {
        let request = LogoutRequest {
            refresh_token: Some("refresh_token".to_string()),
            access_token: Some("access_token".to_string()),
            post_logout_redirect_uri: Some("https://app.example.com".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("refresh_token"));
        assert!(json.contains("access_token"));
    }
}
