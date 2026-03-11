//! OAuth client implementation

use super::config::OAuthConfig;
use super::types::{
    CallbackParams, OAuthError, OAuthState, PkceChallengeMethod, TokenResponse, UserInfo,
};
use crate::utils::error::gateway_error::{GatewayError, Result};
use crate::utils::net::http::create_custom_client;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, warn};

/// OAuth 2.0 client for handling authentication flows
#[derive(Clone)]
pub struct OAuthClient {
    /// HTTP client for making requests
    http_client: Client,

    /// OAuth configuration
    config: Arc<OAuthConfig>,
}

impl std::fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthClient")
            .field("provider", &self.config.provider)
            .field("client_id", &self.config.client_id)
            .finish()
    }
}

impl OAuthClient {
    /// Create a new OAuth client
    pub fn new(config: OAuthConfig) -> Result<Self> {
        config.validate().map_err(GatewayError::Config)?;

        let http_client = create_custom_client(Duration::from_millis(config.timeout_ms))
            .map_err(|e| GatewayError::Network(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            http_client,
            config: Arc::new(config),
        })
    }

    /// Get the OAuth configuration
    pub fn config(&self) -> &OAuthConfig {
        &self.config
    }

    /// Generate the authorization URL and state for initiating OAuth flow
    ///
    /// Returns a tuple of (authorization_url, oauth_state)
    pub fn get_authorization_url(&self) -> (String, OAuthState) {
        let state = if self.config.use_pkce {
            OAuthState::with_pkce(self.config.provider.to_string())
                .with_redirect_uri(&self.config.redirect_uri)
        } else {
            OAuthState::new(self.config.provider.to_string())
                .with_redirect_uri(&self.config.redirect_uri)
        };

        let url = self.build_authorization_url(&state);
        (url, state)
    }

    /// Generate authorization URL with custom state
    pub fn get_authorization_url_with_state(&self, mut state: OAuthState) -> String {
        state.provider = self.config.provider.to_string();
        if state.redirect_uri.is_none() {
            state.redirect_uri = Some(self.config.redirect_uri.clone());
        }
        self.build_authorization_url(&state)
    }

    /// Build the authorization URL from state
    fn build_authorization_url(&self, state: &OAuthState) -> String {
        let mut params = vec![
            ("response_type", "code".to_string()),
            ("client_id", self.config.client_id.clone()),
            ("redirect_uri", self.config.redirect_uri.clone()),
            ("scope", self.config.scopes_string()),
            ("state", state.state.clone()),
        ];

        // Add PKCE parameters if enabled
        if self.config.use_pkce
            && let Some(challenge) = state.code_challenge()
        {
            params.push(("code_challenge", challenge));
            params.push((
                "code_challenge_method",
                PkceChallengeMethod::S256.to_string(),
            ));
        }

        // Add nonce for OIDC
        if let Some(nonce) = &state.nonce {
            params.push(("nonce", nonce.clone()));
        }

        // Add extra parameters from config
        for (key, value) in &self.config.extra_params {
            params.push((key.as_str(), value.clone()));
        }

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        format!("{}?{}", self.config.auth_url, query)
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code(&self, code: &str, state: &OAuthState) -> Result<TokenResponse> {
        debug!(
            "Exchanging authorization code for tokens with provider: {}",
            self.config.provider
        );

        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("code", code);
        params.insert("client_id", self.config.client_id.as_str());
        params.insert("redirect_uri", self.config.redirect_uri.as_str());

        // Add client secret if available
        let client_secret_value;
        if let Some(secret) = &self.config.client_secret {
            client_secret_value = secret.clone();
            params.insert("client_secret", client_secret_value.as_str());
        }

        // Add PKCE code verifier if present
        let code_verifier_value;
        if let Some(verifier) = &state.code_verifier {
            code_verifier_value = verifier.clone();
            params.insert("code_verifier", code_verifier_value.as_str());
        }

        let response = self
            .http_client
            .post(&self.config.token_url)
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await
            .map_err(|e| GatewayError::Network(format!("Token exchange request failed: {}", e)))?;

        let status = response.status();
        let body: String = response
            .text()
            .await
            .map_err(|e| GatewayError::Network(format!("Failed to read response body: {}", e)))?;

        if !status.is_success() {
            // Try to parse as OAuth error
            if let Ok(oauth_error) = serde_json::from_str::<OAuthError>(&body) {
                error!("OAuth token exchange error: {}", oauth_error);
                return Err(GatewayError::Auth(oauth_error.to_string()));
            }
            error!("Token exchange failed with status {}: {}", status, body);
            return Err(GatewayError::Auth(format!(
                "Token exchange failed: {}",
                status
            )));
        }

        // GitHub returns tokens in different formats
        let token_response = self.parse_token_response(&body)?;
        debug!("Successfully obtained tokens from {}", self.config.provider);

        Ok(token_response)
    }

    /// Parse token response handling different provider formats
    fn parse_token_response(&self, body: &str) -> Result<TokenResponse> {
        // Try standard JSON format first
        if let Ok(response) = serde_json::from_str::<TokenResponse>(body) {
            return Ok(response);
        }

        // GitHub might return URL-encoded format
        if body.contains("access_token=") {
            let params: HashMap<String, String> = url::form_urlencoded::parse(body.as_bytes())
                .into_owned()
                .collect();

            let access_token = params
                .get("access_token")
                .ok_or_else(|| GatewayError::Auth("Missing access_token in response".to_string()))?
                .clone();

            let token_type = params
                .get("token_type")
                .cloned()
                .unwrap_or_else(|| "Bearer".to_string());

            let scope = params.get("scope").cloned();

            return Ok(TokenResponse {
                access_token,
                refresh_token: params.get("refresh_token").cloned(),
                expires_in: params
                    .get("expires_in")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3600),
                token_type,
                scope,
                id_token: params.get("id_token").cloned(),
            });
        }

        Err(GatewayError::Parsing(format!(
            "Failed to parse token response: {}",
            body
        )))
    }

    /// Refresh an access token using a refresh token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
        debug!(
            "Refreshing access token with provider: {}",
            self.config.provider
        );

        let mut params = HashMap::new();
        params.insert("grant_type", "refresh_token");
        params.insert("refresh_token", refresh_token);
        params.insert("client_id", self.config.client_id.as_str());

        // Add client secret if available
        let client_secret_value;
        if let Some(secret) = &self.config.client_secret {
            client_secret_value = secret.clone();
            params.insert("client_secret", client_secret_value.as_str());
        }

        let response = self
            .http_client
            .post(&self.config.token_url)
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await
            .map_err(|e| GatewayError::Network(format!("Token refresh request failed: {}", e)))?;

        let status = response.status();
        let body: String = response
            .text()
            .await
            .map_err(|e| GatewayError::Network(format!("Failed to read response body: {}", e)))?;

        if !status.is_success() {
            if let Ok(oauth_error) = serde_json::from_str::<OAuthError>(&body) {
                error!("OAuth token refresh error: {}", oauth_error);
                return Err(GatewayError::Auth(oauth_error.to_string()));
            }
            error!("Token refresh failed with status {}: {}", status, body);
            return Err(GatewayError::Auth(format!(
                "Token refresh failed: {}",
                status
            )));
        }

        let token_response = self.parse_token_response(&body)?;
        debug!("Successfully refreshed tokens");

        Ok(token_response)
    }

    /// Get user information using an access token
    pub async fn get_user_info(&self, access_token: &str) -> Result<UserInfo> {
        let userinfo_url = self
            .config
            .userinfo_url
            .as_ref()
            .ok_or_else(|| GatewayError::Config("UserInfo URL not configured".to_string()))?;

        debug!("Fetching user info from {}", userinfo_url);

        let response = self
            .http_client
            .get(userinfo_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| GatewayError::Network(format!("UserInfo request failed: {}", e)))?;

        let status = response.status();
        let body: String = response
            .text()
            .await
            .map_err(|e| GatewayError::Network(format!("Failed to read response body: {}", e)))?;

        if !status.is_success() {
            error!("UserInfo request failed with status {}: {}", status, body);
            return Err(GatewayError::Auth(format!(
                "UserInfo request failed: {}",
                status
            )));
        }

        let user_info = self.parse_user_info(&body)?;
        debug!("Successfully fetched user info for: {}", user_info.email);

        Ok(user_info)
    }

    /// Parse user info response handling different provider formats
    fn parse_user_info(&self, body: &str) -> Result<UserInfo> {
        let json: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| GatewayError::Parsing(format!("Failed to parse user info: {}", e)))?;

        let provider = self.config.provider.to_string();

        // Extract user ID based on provider
        let id = self.extract_user_id(&json)?;

        // Extract email - try multiple fields
        let email = json
            .get("email")
            .and_then(|v| v.as_str())
            .or_else(|| json.get("mail").and_then(|v| v.as_str()))
            .ok_or_else(|| GatewayError::Auth("Email not found in user info".to_string()))?
            .to_string();

        let mut user_info = UserInfo::new(id, email, provider);

        // Extract name
        if let Some(name) = json
            .get("name")
            .or_else(|| json.get("displayName"))
            .or_else(|| json.get("login")) // GitHub uses 'login'
            .and_then(|v| v.as_str())
        {
            user_info.name = Some(name.to_string());
        }

        // Extract picture/avatar
        if let Some(picture) = json
            .get("picture")
            .or_else(|| json.get("avatar_url"))
            .or_else(|| json.get("photo"))
            .and_then(|v| v.as_str())
        {
            user_info.picture = Some(picture.to_string());
        }

        // Extract email verified status
        if let Some(verified) = json.get("email_verified").and_then(|v| v.as_bool()) {
            user_info.email_verified = verified;
        }

        // Store additional claims
        let known_fields = [
            "sub",
            "id",
            "email",
            "name",
            "picture",
            "avatar_url",
            "email_verified",
            "mail",
            "displayName",
            "login",
            "photo",
        ];
        for (key, value) in json.as_object().into_iter().flatten() {
            if !known_fields.contains(&key.as_str()) {
                user_info.extra_claims.insert(key.clone(), value.clone());
            }
        }

        Ok(user_info)
    }

    /// Extract user ID from response based on provider format
    fn extract_user_id(&self, json: &serde_json::Value) -> Result<String> {
        // Try 'sub' first (OIDC standard)
        if let Some(sub) = json.get("sub").and_then(|v| v.as_str()) {
            return Ok(sub.to_string());
        }

        // Try 'id' (GitHub, etc.)
        if let Some(id) = json.get("id") {
            return Ok(match id {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => id.to_string(),
            });
        }

        // Try 'oid' (Microsoft Azure AD)
        if let Some(oid) = json.get("oid").and_then(|v| v.as_str()) {
            return Ok(oid.to_string());
        }

        Err(GatewayError::Auth(
            "User ID not found in response".to_string(),
        ))
    }

    /// Validate callback parameters
    pub fn validate_callback(
        &self,
        params: &CallbackParams,
        expected_state: &OAuthState,
    ) -> Result<()> {
        // Check for OAuth error
        if let Some(error) = &params.error {
            let description = params.error_description.clone().unwrap_or_default();
            return Err(GatewayError::Auth(format!(
                "OAuth error: {} - {}",
                error, description
            )));
        }

        // Validate state parameter
        let state = params
            .state
            .as_ref()
            .ok_or_else(|| GatewayError::Auth("Missing state parameter".to_string()))?;

        if state != &expected_state.state {
            warn!(
                "State mismatch: expected {}, got {}",
                expected_state.state, state
            );
            return Err(GatewayError::Auth("Invalid state parameter".to_string()));
        }

        // Check if state has expired
        if expected_state.is_expired() {
            return Err(GatewayError::Auth("OAuth state has expired".to_string()));
        }

        // Validate code is present
        if params.code.is_none() {
            return Err(GatewayError::Auth("Missing authorization code".to_string()));
        }

        Ok(())
    }

    /// Revoke a token (if supported by the provider)
    pub async fn revoke_token(&self, token: &str, token_type_hint: Option<&str>) -> Result<()> {
        // Build revocation URL - not all providers support this
        let revocation_url = match self.config.provider {
            super::config::OAuthProvider::Google => {
                Some("https://oauth2.googleapis.com/revoke".to_string())
            }
            super::config::OAuthProvider::Microsoft => {
                Some("https://login.microsoftonline.com/common/oauth2/v2.0/logout".to_string())
            }
            _ => None,
        };

        let Some(url) = revocation_url else {
            warn!(
                "Token revocation not supported for provider: {}",
                self.config.provider
            );
            return Ok(());
        };

        let mut params = HashMap::new();
        params.insert("token", token);
        if let Some(hint) = token_type_hint {
            params.insert("token_type_hint", hint);
        }

        let response = self
            .http_client
            .post(&url)
            .form(&params)
            .send()
            .await
            .map_err(|e| GatewayError::Network(format!("Token revocation failed: {}", e)))?;

        if !response.status().is_success() {
            warn!(
                "Token revocation returned non-success status: {}",
                response.status()
            );
        }

        Ok(())
    }

    /// Get the logout URL for the provider
    pub fn get_logout_url(
        &self,
        id_token: Option<&str>,
        post_logout_redirect_uri: Option<&str>,
    ) -> Option<String> {
        let logout_url = self.config.logout_url.as_ref()?;

        let mut params = Vec::new();

        if let Some(token) = id_token {
            params.push(format!("id_token_hint={}", urlencoding::encode(token)));
        }

        if let Some(uri) = post_logout_redirect_uri {
            params.push(format!(
                "post_logout_redirect_uri={}",
                urlencoding::encode(uri)
            ));
        }

        if params.is_empty() {
            Some(logout_url.clone())
        } else {
            Some(format!("{}?{}", logout_url, params.join("&")))
        }
    }
}

/// URL encoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::oauth::config::OAuthProvider;

    fn create_test_config() -> OAuthConfig {
        OAuthConfig::google("test_client_id", "https://app.example.com/callback")
            .with_client_secret("test_client_secret")
    }

    #[test]
    fn test_oauth_client_creation() {
        let config = create_test_config();
        let client = OAuthClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_oauth_client_invalid_config() {
        let mut config = create_test_config();
        config.client_id = String::new();
        let client = OAuthClient::new(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_authorization_url_generation() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();
        let (url, state) = client.get_authorization_url();

        assert!(url.contains("accounts.google.com"));
        assert!(url.contains("client_id=test_client_id"));
        assert!(url.contains("redirect_uri="));
        assert!(url.contains(&format!("state={}", state.state)));
        assert!(url.contains("response_type=code"));
    }

    #[test]
    fn test_authorization_url_with_pkce() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();
        let (url, state) = client.get_authorization_url();

        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(state.code_verifier.is_some());
    }

    #[test]
    fn test_authorization_url_without_pkce() {
        let config = OAuthConfig::github("test_client_id", "https://app.example.com/callback")
            .with_client_secret("test_secret");
        let client = OAuthClient::new(config).unwrap();
        let (url, state) = client.get_authorization_url();

        assert!(!url.contains("code_challenge="));
        assert!(state.code_verifier.is_none());
    }

    #[test]
    fn test_authorization_url_with_extra_params() {
        let config = create_test_config()
            .with_param("prompt", "consent")
            .with_param("access_type", "offline");
        let client = OAuthClient::new(config).unwrap();
        let (url, _) = client.get_authorization_url();

        assert!(url.contains("prompt=consent"));
        assert!(url.contains("access_type=offline"));
    }

    #[test]
    fn test_callback_validation_success() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();
        let (_, state) = client.get_authorization_url();

        let params = CallbackParams {
            code: Some("auth_code".to_string()),
            state: Some(state.state.clone()),
            error: None,
            error_description: None,
        };

        assert!(client.validate_callback(&params, &state).is_ok());
    }

    #[test]
    fn test_callback_validation_error() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();
        let (_, state) = client.get_authorization_url();

        let params = CallbackParams {
            code: None,
            state: Some(state.state.clone()),
            error: Some("access_denied".to_string()),
            error_description: Some("User denied access".to_string()),
        };

        let result = client.validate_callback(&params, &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_callback_validation_state_mismatch() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();
        let (_, state) = client.get_authorization_url();

        let params = CallbackParams {
            code: Some("auth_code".to_string()),
            state: Some("wrong_state".to_string()),
            error: None,
            error_description: None,
        };

        let result = client.validate_callback(&params, &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_callback_validation_expired_state() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();
        let (_, mut state) = client.get_authorization_url();

        // Make state expired
        state.created_at = chrono::Utc::now() - chrono::Duration::seconds(700);

        let params = CallbackParams {
            code: Some("auth_code".to_string()),
            state: Some(state.state.clone()),
            error: None,
            error_description: None,
        };

        let result = client.validate_callback(&params, &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_user_info_google() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();

        let json = r#"{
            "sub": "123456789",
            "email": "user@example.com",
            "email_verified": true,
            "name": "Test User",
            "picture": "https://example.com/photo.jpg"
        }"#;

        let user_info = client.parse_user_info(json).unwrap();
        assert_eq!(user_info.id, "123456789");
        assert_eq!(user_info.email, "user@example.com");
        assert_eq!(user_info.name, Some("Test User".to_string()));
        assert!(user_info.email_verified);
    }

    #[test]
    fn test_parse_user_info_github() {
        let config = OAuthConfig::github("test_id", "https://app.example.com/callback");
        let client = OAuthClient::new(config).unwrap();

        let json = r#"{
            "id": 12345,
            "email": "user@example.com",
            "login": "testuser",
            "avatar_url": "https://github.com/avatar.jpg"
        }"#;

        let user_info = client.parse_user_info(json).unwrap();
        assert_eq!(user_info.id, "12345");
        assert_eq!(user_info.email, "user@example.com");
        assert_eq!(user_info.name, Some("testuser".to_string()));
        assert_eq!(
            user_info.picture,
            Some("https://github.com/avatar.jpg".to_string())
        );
    }

    #[test]
    fn test_parse_token_response_json() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();

        let json = r#"{
            "access_token": "access123",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "refresh456",
            "scope": "openid email"
        }"#;

        let response = client.parse_token_response(json).unwrap();
        assert_eq!(response.access_token, "access123");
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in, 3600);
        assert_eq!(response.refresh_token, Some("refresh456".to_string()));
    }

    #[test]
    fn test_parse_token_response_urlencoded() {
        let config = OAuthConfig::github("test_id", "https://app.example.com/callback");
        let client = OAuthClient::new(config).unwrap();

        let body = "access_token=access123&token_type=bearer&scope=read%3Auser";

        let response = client.parse_token_response(body).unwrap();
        assert_eq!(response.access_token, "access123");
    }

    #[test]
    fn test_logout_url_generation() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();

        let url = client.get_logout_url(None, None);
        assert!(url.is_some());

        let url_with_params =
            client.get_logout_url(Some("id_token_123"), Some("https://app.example.com"));
        assert!(url_with_params.is_some());
        let url = url_with_params.unwrap();
        assert!(url.contains("id_token_hint="));
        assert!(url.contains("post_logout_redirect_uri="));
    }

    #[test]
    fn test_oauth_client_debug() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();
        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("OAuthClient"));
        assert!(debug_str.contains("Google"));
    }

    #[test]
    fn test_extract_user_id_various_formats() {
        let config = create_test_config();
        let client = OAuthClient::new(config).unwrap();

        // OIDC standard 'sub'
        let json1 = serde_json::json!({"sub": "user123"});
        assert_eq!(client.extract_user_id(&json1).unwrap(), "user123");

        // GitHub style numeric 'id'
        let json2 = serde_json::json!({"id": 12345});
        assert_eq!(client.extract_user_id(&json2).unwrap(), "12345");

        // Microsoft 'oid'
        let json3 = serde_json::json!({"oid": "guid-123-456"});
        assert_eq!(client.extract_user_id(&json3).unwrap(), "guid-123-456");
    }
}
