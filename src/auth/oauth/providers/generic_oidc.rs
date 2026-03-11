//! Generic OIDC Provider with Discovery support
//!
//! This module provides a generic OpenID Connect provider implementation
//! that supports OIDC Discovery for automatic endpoint configuration.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::auth::oauth::config::{OAuthConfig, OAuthProvider};
use crate::auth::oauth::types::{OAuthState, TokenResponse, UserInfo};
use crate::utils::error::gateway_error::{GatewayError, Result};
use crate::utils::net::http::create_custom_client;

/// OIDC Discovery document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcDiscovery {
    /// Issuer identifier
    pub issuer: String,

    /// Authorization endpoint
    pub authorization_endpoint: String,

    /// Token endpoint
    pub token_endpoint: String,

    /// UserInfo endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userinfo_endpoint: Option<String>,

    /// JWKS URI for token validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwks_uri: Option<String>,

    /// End session endpoint (logout)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_session_endpoint: Option<String>,

    /// Revocation endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revocation_endpoint: Option<String>,

    /// Introspection endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introspection_endpoint: Option<String>,

    /// Supported scopes
    #[serde(default)]
    pub scopes_supported: Vec<String>,

    /// Supported response types
    #[serde(default)]
    pub response_types_supported: Vec<String>,

    /// Supported grant types
    #[serde(default)]
    pub grant_types_supported: Vec<String>,

    /// Supported subject types
    #[serde(default)]
    pub subject_types_supported: Vec<String>,

    /// Supported ID token signing algorithms
    #[serde(default)]
    pub id_token_signing_alg_values_supported: Vec<String>,

    /// Supported token endpoint auth methods
    #[serde(default)]
    pub token_endpoint_auth_methods_supported: Vec<String>,

    /// Supported claims
    #[serde(default)]
    pub claims_supported: Vec<String>,

    /// Whether PKCE is supported
    #[serde(default)]
    pub code_challenge_methods_supported: Vec<String>,
}

impl OidcDiscovery {
    /// Fetch OIDC discovery document from issuer
    pub async fn fetch(issuer_url: &str) -> Result<Self> {
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            issuer_url.trim_end_matches('/')
        );

        debug!("Fetching OIDC discovery from: {}", discovery_url);

        let client = create_custom_client(Duration::from_secs(30))
            .map_err(|e| GatewayError::Network(format!("Failed to create HTTP client: {}", e)))?;

        let response = client
            .get(&discovery_url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| GatewayError::Network(format!("Failed to fetch OIDC discovery: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::Network(format!(
                "OIDC discovery failed with status: {}",
                response.status()
            )));
        }

        let discovery: OidcDiscovery = response
            .json()
            .await
            .map_err(|e| GatewayError::Parsing(format!("Failed to parse OIDC discovery: {}", e)))?;

        info!(
            "Successfully fetched OIDC discovery for issuer: {}",
            discovery.issuer
        );
        Ok(discovery)
    }

    /// Check if PKCE is supported
    pub fn supports_pkce(&self) -> bool {
        self.code_challenge_methods_supported
            .contains(&"S256".to_string())
            || self
                .code_challenge_methods_supported
                .contains(&"plain".to_string())
    }

    /// Check if a specific scope is supported
    pub fn supports_scope(&self, scope: &str) -> bool {
        self.scopes_supported.is_empty() || self.scopes_supported.contains(&scope.to_string())
    }

    /// Get recommended scopes based on what's supported
    pub fn recommended_scopes(&self) -> Vec<String> {
        let default_scopes = vec!["openid", "email", "profile"];

        if self.scopes_supported.is_empty() {
            return default_scopes.into_iter().map(String::from).collect();
        }

        default_scopes
            .into_iter()
            .filter(|s| self.scopes_supported.contains(&s.to_string()))
            .map(String::from)
            .collect()
    }
}

/// OIDC Provider configuration
#[derive(Debug, Clone)]
pub struct OidcProviderConfig {
    /// Provider name/identifier
    pub name: String,

    /// Issuer URL (used for discovery)
    pub issuer_url: String,

    /// Client ID
    pub client_id: String,

    /// Client secret (optional for public clients)
    pub client_secret: Option<String>,

    /// Redirect URI
    pub redirect_uri: String,

    /// Additional scopes beyond openid
    pub additional_scopes: Vec<String>,

    /// Whether to use PKCE
    pub use_pkce: bool,

    /// Extra authorization parameters
    pub extra_params: HashMap<String, String>,

    /// Connection timeout in milliseconds
    pub timeout_ms: u64,
}

impl OidcProviderConfig {
    /// Create a new OIDC provider configuration
    pub fn new(
        name: impl Into<String>,
        issuer_url: impl Into<String>,
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            issuer_url: issuer_url.into(),
            client_id: client_id.into(),
            client_secret: None,
            redirect_uri: redirect_uri.into(),
            additional_scopes: vec!["email".to_string(), "profile".to_string()],
            use_pkce: true,
            extra_params: HashMap::new(),
            timeout_ms: 30000,
        }
    }

    /// Set the client secret
    pub fn with_client_secret(mut self, secret: impl Into<String>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    /// Add additional scopes
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.additional_scopes = scopes;
        self
    }

    /// Enable or disable PKCE
    pub fn with_pkce(mut self, use_pkce: bool) -> Self {
        self.use_pkce = use_pkce;
        self
    }

    /// Add extra authorization parameter
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_params.insert(key.into(), value.into());
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// Generic OIDC Provider with discovery support
pub struct OidcProvider {
    /// Provider configuration
    config: OidcProviderConfig,

    /// Discovered OIDC metadata
    discovery: OidcDiscovery,

    /// HTTP client
    http_client: Client,
}

impl std::fmt::Debug for OidcProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcProvider")
            .field("name", &self.config.name)
            .field("issuer", &self.discovery.issuer)
            .finish()
    }
}

impl OidcProvider {
    /// Create a new OIDC provider with automatic discovery
    pub async fn new(config: OidcProviderConfig) -> Result<Self> {
        let discovery = OidcDiscovery::fetch(&config.issuer_url).await?;

        let http_client = create_custom_client(Duration::from_millis(config.timeout_ms))
            .map_err(|e| GatewayError::Network(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            discovery,
            http_client,
        })
    }

    /// Create from an existing discovery document
    pub fn from_discovery(config: OidcProviderConfig, discovery: OidcDiscovery) -> Result<Self> {
        let http_client = create_custom_client(Duration::from_millis(config.timeout_ms))
            .map_err(|e| GatewayError::Network(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            discovery,
            http_client,
        })
    }

    /// Get the provider name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get the discovery document
    pub fn discovery(&self) -> &OidcDiscovery {
        &self.discovery
    }

    /// Convert to OAuthConfig for use with OAuthClient
    pub fn to_oauth_config(&self) -> OAuthConfig {
        let mut scopes = vec!["openid".to_string()];
        scopes.extend(self.config.additional_scopes.clone());

        let use_pkce = self.config.use_pkce && self.discovery.supports_pkce();

        OAuthConfig {
            provider: OAuthProvider::Custom,
            client_id: self.config.client_id.clone(),
            client_secret: self.config.client_secret.clone(),
            auth_url: self.discovery.authorization_endpoint.clone(),
            token_url: self.discovery.token_endpoint.clone(),
            userinfo_url: self.discovery.userinfo_endpoint.clone(),
            scopes,
            redirect_uri: self.config.redirect_uri.clone(),
            use_pkce,
            logout_url: self.discovery.end_session_endpoint.clone(),
            jwks_uri: self.discovery.jwks_uri.clone(),
            issuer: Some(self.discovery.issuer.clone()),
            extra_params: self.config.extra_params.clone(),
            role_mapping: HashMap::new(),
            enabled: true,
            timeout_ms: self.config.timeout_ms,
        }
    }

    /// Generate authorization URL
    pub fn get_authorization_url(&self) -> (String, OAuthState) {
        let state = if self.config.use_pkce && self.discovery.supports_pkce() {
            OAuthState::with_pkce(self.config.name.clone())
                .with_redirect_uri(&self.config.redirect_uri)
        } else {
            OAuthState::new(self.config.name.clone()).with_redirect_uri(&self.config.redirect_uri)
        };

        let url = self.build_authorization_url(&state);
        (url, state)
    }

    fn build_authorization_url(&self, state: &OAuthState) -> String {
        let mut scopes = vec!["openid".to_string()];
        scopes.extend(self.config.additional_scopes.clone());

        let mut params = vec![
            ("response_type", "code".to_string()),
            ("client_id", self.config.client_id.clone()),
            ("redirect_uri", self.config.redirect_uri.clone()),
            ("scope", scopes.join(" ")),
            ("state", state.state.clone()),
        ];

        // Add PKCE if supported
        if self.config.use_pkce
            && self.discovery.supports_pkce()
            && let Some(challenge) = state.code_challenge()
        {
            params.push(("code_challenge", challenge));
            params.push(("code_challenge_method", "S256".to_string()));
        }

        // Add nonce
        if let Some(nonce) = &state.nonce {
            params.push(("nonce", nonce.clone()));
        }

        // Add extra params
        for (key, value) in &self.config.extra_params {
            params.push((key.as_str(), value.clone()));
        }

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        format!("{}?{}", self.discovery.authorization_endpoint, query)
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code(&self, code: &str, state: &OAuthState) -> Result<TokenResponse> {
        debug!("Exchanging code with OIDC provider: {}", self.config.name);

        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("code", code);
        params.insert("client_id", self.config.client_id.as_str());
        params.insert("redirect_uri", self.config.redirect_uri.as_str());

        let client_secret_value;
        if let Some(secret) = &self.config.client_secret {
            client_secret_value = secret.clone();
            params.insert("client_secret", client_secret_value.as_str());
        }

        let code_verifier_value;
        if let Some(verifier) = &state.code_verifier {
            code_verifier_value = verifier.clone();
            params.insert("code_verifier", code_verifier_value.as_str());
        }

        let response = self
            .http_client
            .post(&self.discovery.token_endpoint)
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await
            .map_err(|e| GatewayError::Network(format!("Token exchange failed: {}", e)))?;

        let status = response.status();
        let body: String = response
            .text()
            .await
            .map_err(|e| GatewayError::Network(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            error!("Token exchange failed: {} - {}", status, body);
            return Err(GatewayError::Auth(format!(
                "Token exchange failed: {}",
                status
            )));
        }

        let token_response: TokenResponse = serde_json::from_str(&body)
            .map_err(|e| GatewayError::Parsing(format!("Failed to parse token response: {}", e)))?;

        debug!("Successfully obtained tokens from {}", self.config.name);
        Ok(token_response)
    }

    /// Get user info
    pub async fn get_user_info(&self, access_token: &str) -> Result<UserInfo> {
        let userinfo_url =
            self.discovery.userinfo_endpoint.as_ref().ok_or_else(|| {
                GatewayError::Config("UserInfo endpoint not available".to_string())
            })?;

        debug!("Fetching user info from: {}", userinfo_url);

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
            .map_err(|e| GatewayError::Network(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            error!("UserInfo request failed: {} - {}", status, body);
            return Err(GatewayError::Auth(format!(
                "UserInfo request failed: {}",
                status
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| GatewayError::Parsing(format!("Failed to parse user info: {}", e)))?;

        // Extract standard OIDC claims
        let id = json
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::Auth("Missing 'sub' claim".to_string()))?
            .to_string();

        let email = json
            .get("email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::Auth("Missing 'email' claim".to_string()))?
            .to_string();

        let mut user_info = UserInfo::new(id, email, self.config.name.clone());

        if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
            user_info.name = Some(name.to_string());
        }

        if let Some(picture) = json.get("picture").and_then(|v| v.as_str()) {
            user_info.picture = Some(picture.to_string());
        }

        if let Some(verified) = json.get("email_verified").and_then(|v| v.as_bool()) {
            user_info.email_verified = verified;
        }

        // Store additional claims
        let known_fields = ["sub", "email", "name", "picture", "email_verified"];
        for (key, value) in json.as_object().into_iter().flatten() {
            if !known_fields.contains(&key.as_str()) {
                user_info.extra_claims.insert(key.clone(), value.clone());
            }
        }

        debug!("Successfully fetched user info for: {}", user_info.email);
        Ok(user_info)
    }

    /// Get logout URL
    pub fn get_logout_url(
        &self,
        id_token: Option<&str>,
        post_logout_redirect_uri: Option<&str>,
    ) -> Option<String> {
        let logout_url = self.discovery.end_session_endpoint.as_ref()?;

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

    /// Refresh access token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
        debug!("Refreshing token with OIDC provider: {}", self.config.name);

        let mut params = HashMap::new();
        params.insert("grant_type", "refresh_token");
        params.insert("refresh_token", refresh_token);
        params.insert("client_id", self.config.client_id.as_str());

        let client_secret_value;
        if let Some(secret) = &self.config.client_secret {
            client_secret_value = secret.clone();
            params.insert("client_secret", client_secret_value.as_str());
        }

        let response = self
            .http_client
            .post(&self.discovery.token_endpoint)
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await
            .map_err(|e| GatewayError::Network(format!("Token refresh failed: {}", e)))?;

        let status = response.status();
        let body: String = response
            .text()
            .await
            .map_err(|e| GatewayError::Network(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            error!("Token refresh failed: {} - {}", status, body);
            return Err(GatewayError::Auth(format!(
                "Token refresh failed: {}",
                status
            )));
        }

        let token_response: TokenResponse = serde_json::from_str(&body)
            .map_err(|e| GatewayError::Parsing(format!("Failed to parse token response: {}", e)))?;

        debug!("Successfully refreshed tokens");
        Ok(token_response)
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

    #[test]
    fn test_oidc_provider_config_builder() {
        let config = OidcProviderConfig::new(
            "test-provider",
            "https://auth.example.com",
            "client123",
            "https://app.example.com/callback",
        )
        .with_client_secret("secret456")
        .with_scopes(vec!["email".to_string(), "profile".to_string()])
        .with_pkce(true)
        .with_param("prompt", "consent");

        assert_eq!(config.name, "test-provider");
        assert_eq!(config.issuer_url, "https://auth.example.com");
        assert_eq!(config.client_id, "client123");
        assert_eq!(config.client_secret, Some("secret456".to_string()));
        assert!(config.use_pkce);
        assert_eq!(
            config.extra_params.get("prompt"),
            Some(&"consent".to_string())
        );
    }

    #[test]
    fn test_oidc_discovery_supports_pkce() {
        let mut discovery = OidcDiscovery {
            issuer: "https://auth.example.com".to_string(),
            authorization_endpoint: "https://auth.example.com/authorize".to_string(),
            token_endpoint: "https://auth.example.com/token".to_string(),
            userinfo_endpoint: None,
            jwks_uri: None,
            end_session_endpoint: None,
            revocation_endpoint: None,
            introspection_endpoint: None,
            scopes_supported: vec![],
            response_types_supported: vec![],
            grant_types_supported: vec![],
            subject_types_supported: vec![],
            id_token_signing_alg_values_supported: vec![],
            token_endpoint_auth_methods_supported: vec![],
            claims_supported: vec![],
            code_challenge_methods_supported: vec![],
        };

        assert!(!discovery.supports_pkce());

        discovery.code_challenge_methods_supported = vec!["S256".to_string()];
        assert!(discovery.supports_pkce());
    }

    #[test]
    fn test_oidc_discovery_recommended_scopes() {
        let discovery = OidcDiscovery {
            issuer: "https://auth.example.com".to_string(),
            authorization_endpoint: "https://auth.example.com/authorize".to_string(),
            token_endpoint: "https://auth.example.com/token".to_string(),
            userinfo_endpoint: None,
            jwks_uri: None,
            end_session_endpoint: None,
            revocation_endpoint: None,
            introspection_endpoint: None,
            scopes_supported: vec!["openid".to_string(), "email".to_string()],
            response_types_supported: vec![],
            grant_types_supported: vec![],
            subject_types_supported: vec![],
            id_token_signing_alg_values_supported: vec![],
            token_endpoint_auth_methods_supported: vec![],
            claims_supported: vec![],
            code_challenge_methods_supported: vec![],
        };

        let scopes = discovery.recommended_scopes();
        assert!(scopes.contains(&"openid".to_string()));
        assert!(scopes.contains(&"email".to_string()));
        assert!(!scopes.contains(&"profile".to_string())); // Not in scopes_supported
    }

    #[test]
    fn test_oidc_discovery_supports_scope() {
        let discovery = OidcDiscovery {
            issuer: "https://auth.example.com".to_string(),
            authorization_endpoint: "https://auth.example.com/authorize".to_string(),
            token_endpoint: "https://auth.example.com/token".to_string(),
            userinfo_endpoint: None,
            jwks_uri: None,
            end_session_endpoint: None,
            revocation_endpoint: None,
            introspection_endpoint: None,
            scopes_supported: vec!["openid".to_string(), "email".to_string()],
            response_types_supported: vec![],
            grant_types_supported: vec![],
            subject_types_supported: vec![],
            id_token_signing_alg_values_supported: vec![],
            token_endpoint_auth_methods_supported: vec![],
            claims_supported: vec![],
            code_challenge_methods_supported: vec![],
        };

        assert!(discovery.supports_scope("openid"));
        assert!(discovery.supports_scope("email"));
        assert!(!discovery.supports_scope("profile"));
    }
}
