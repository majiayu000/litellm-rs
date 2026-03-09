//! OAuth configuration types and provider definitions

use crate::core::types::config::defaults::default_true;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OAuth provider enumeration with pre-configured settings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    /// Google OAuth 2.0
    #[default]
    Google,
    /// Microsoft Azure AD / Entra ID
    Microsoft,
    /// GitHub OAuth
    GitHub,
    /// Okta Identity Platform
    Okta,
    /// Auth0 Identity Platform
    Auth0,
    /// Generic/Custom OAuth provider
    Custom,
}

impl OAuthProvider {
    /// Get the default authorization URL for the provider
    pub fn default_auth_url(&self) -> Option<&'static str> {
        match self {
            Self::Google => Some("https://accounts.google.com/o/oauth2/v2/auth"),
            Self::Microsoft => {
                Some("https://login.microsoftonline.com/common/oauth2/v2.0/authorize")
            }
            Self::GitHub => Some("https://github.com/login/oauth/authorize"),
            Self::Okta => None,  // Requires tenant-specific URL
            Self::Auth0 => None, // Requires tenant-specific URL
            Self::Custom => None,
        }
    }

    /// Get the default token URL for the provider
    pub fn default_token_url(&self) -> Option<&'static str> {
        match self {
            Self::Google => Some("https://oauth2.googleapis.com/token"),
            Self::Microsoft => Some("https://login.microsoftonline.com/common/oauth2/v2.0/token"),
            Self::GitHub => Some("https://github.com/login/oauth/access_token"),
            Self::Okta => None,
            Self::Auth0 => None,
            Self::Custom => None,
        }
    }

    /// Get the default userinfo URL for the provider
    pub fn default_userinfo_url(&self) -> Option<&'static str> {
        match self {
            Self::Google => Some("https://www.googleapis.com/oauth2/v3/userinfo"),
            Self::Microsoft => Some("https://graph.microsoft.com/oidc/userinfo"),
            Self::GitHub => Some("https://api.github.com/user"),
            Self::Okta => None,
            Self::Auth0 => None,
            Self::Custom => None,
        }
    }

    /// Get the default scopes for the provider
    pub fn default_scopes(&self) -> Vec<String> {
        match self {
            Self::Google => vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
            Self::Microsoft => vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
                "User.Read".to_string(),
            ],
            Self::GitHub => vec!["read:user".to_string(), "user:email".to_string()],
            Self::Okta | Self::Auth0 => vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
            Self::Custom => vec!["openid".to_string()],
        }
    }

    /// Check if the provider supports PKCE
    pub fn supports_pkce(&self) -> bool {
        match self {
            Self::Google => true,
            Self::Microsoft => true,
            Self::GitHub => false, // GitHub doesn't support PKCE
            Self::Okta => true,
            Self::Auth0 => true,
            Self::Custom => true, // Assume custom providers support PKCE
        }
    }

    /// Get the logout URL for the provider (if supported)
    pub fn default_logout_url(&self) -> Option<&'static str> {
        match self {
            Self::Google => Some("https://accounts.google.com/logout"),
            Self::Microsoft => Some("https://login.microsoftonline.com/common/oauth2/v2.0/logout"),
            Self::GitHub => None, // GitHub doesn't have an OAuth logout endpoint
            Self::Okta => None,   // Requires tenant-specific URL
            Self::Auth0 => None,  // Requires tenant-specific URL
            Self::Custom => None,
        }
    }
}

impl std::fmt::Display for OAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Google => write!(f, "google"),
            Self::Microsoft => write!(f, "microsoft"),
            Self::GitHub => write!(f, "github"),
            Self::Okta => write!(f, "okta"),
            Self::Auth0 => write!(f, "auth0"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for OAuthProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "google" => Ok(Self::Google),
            "microsoft" | "azure" | "entra" => Ok(Self::Microsoft),
            "github" => Ok(Self::GitHub),
            "okta" => Ok(Self::Okta),
            "auth0" => Ok(Self::Auth0),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("Unknown OAuth provider: {}", s)),
        }
    }
}

/// OAuth 2.0 configuration for a single provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// OAuth provider type
    pub provider: OAuthProvider,

    /// OAuth client ID
    pub client_id: String,

    /// OAuth client secret (required for confidential clients)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// Authorization endpoint URL
    pub auth_url: String,

    /// Token endpoint URL
    pub token_url: String,

    /// UserInfo endpoint URL (for OIDC providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userinfo_url: Option<String>,

    /// OAuth scopes to request
    #[serde(default)]
    pub scopes: Vec<String>,

    /// Redirect URI for OAuth callback
    pub redirect_uri: String,

    /// Whether to use PKCE (recommended for public clients)
    #[serde(default)]
    pub use_pkce: bool,

    /// Logout URL (for OIDC logout)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logout_url: Option<String>,

    /// JWKS URI for token validation (OIDC)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwks_uri: Option<String>,

    /// Issuer URL for token validation (OIDC)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,

    /// Additional authorization parameters
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_params: HashMap<String, String>,

    /// Role mapping from OAuth claims to internal roles
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub role_mapping: HashMap<String, String>,

    /// Whether this provider is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Connection timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    30000
}

impl OAuthConfig {
    /// Create a new OAuth configuration for the specified provider
    pub fn new(
        provider: OAuthProvider,
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> Self {
        let client_id = client_id.into();
        let redirect_uri = redirect_uri.into();

        Self {
            provider,
            client_id,
            client_secret: None,
            auth_url: provider.default_auth_url().unwrap_or("").to_string(),
            token_url: provider.default_token_url().unwrap_or("").to_string(),
            userinfo_url: provider.default_userinfo_url().map(String::from),
            scopes: provider.default_scopes(),
            redirect_uri,
            use_pkce: provider.supports_pkce(),
            logout_url: provider.default_logout_url().map(String::from),
            jwks_uri: None,
            issuer: None,
            extra_params: HashMap::new(),
            role_mapping: HashMap::new(),
            enabled: true,
            timeout_ms: 30000,
        }
    }

    /// Create a Google OAuth configuration
    pub fn google(client_id: impl Into<String>, redirect_uri: impl Into<String>) -> Self {
        Self::new(OAuthProvider::Google, client_id, redirect_uri)
    }

    /// Create a Microsoft OAuth configuration
    pub fn microsoft(client_id: impl Into<String>, redirect_uri: impl Into<String>) -> Self {
        Self::new(OAuthProvider::Microsoft, client_id, redirect_uri)
    }

    /// Create a GitHub OAuth configuration
    pub fn github(client_id: impl Into<String>, redirect_uri: impl Into<String>) -> Self {
        Self::new(OAuthProvider::GitHub, client_id, redirect_uri)
    }

    /// Create an Okta OAuth configuration
    pub fn okta(
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
        domain: impl Into<String>,
    ) -> Self {
        let domain = domain.into();
        let mut config = Self::new(OAuthProvider::Okta, client_id, redirect_uri);
        config.auth_url = format!("https://{}/oauth2/v1/authorize", domain);
        config.token_url = format!("https://{}/oauth2/v1/token", domain);
        config.userinfo_url = Some(format!("https://{}/oauth2/v1/userinfo", domain));
        config.logout_url = Some(format!("https://{}/oauth2/v1/logout", domain));
        config.jwks_uri = Some(format!("https://{}/oauth2/v1/keys", domain));
        config.issuer = Some(format!("https://{}", domain));
        config
    }

    /// Create an Auth0 OAuth configuration
    pub fn auth0(
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
        domain: impl Into<String>,
    ) -> Self {
        let domain = domain.into();
        let mut config = Self::new(OAuthProvider::Auth0, client_id, redirect_uri);
        config.auth_url = format!("https://{}/authorize", domain);
        config.token_url = format!("https://{}/oauth/token", domain);
        config.userinfo_url = Some(format!("https://{}/userinfo", domain));
        config.logout_url = Some(format!("https://{}/v2/logout", domain));
        config.jwks_uri = Some(format!("https://{}/.well-known/jwks.json", domain));
        config.issuer = Some(format!("https://{}/", domain));
        config
    }

    /// Create a custom OAuth configuration
    pub fn custom(
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
        auth_url: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        let mut config = Self::new(OAuthProvider::Custom, client_id, redirect_uri);
        config.auth_url = auth_url.into();
        config.token_url = token_url.into();
        config
    }

    /// Set the client secret
    pub fn with_client_secret(mut self, secret: impl Into<String>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    /// Set the scopes
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    /// Add a scope
    pub fn add_scope(mut self, scope: impl Into<String>) -> Self {
        self.scopes.push(scope.into());
        self
    }

    /// Set the userinfo URL
    pub fn with_userinfo_url(mut self, url: impl Into<String>) -> Self {
        self.userinfo_url = Some(url.into());
        self
    }

    /// Set the logout URL
    pub fn with_logout_url(mut self, url: impl Into<String>) -> Self {
        self.logout_url = Some(url.into());
        self
    }

    /// Enable or disable PKCE
    pub fn with_pkce(mut self, use_pkce: bool) -> Self {
        self.use_pkce = use_pkce;
        self
    }

    /// Add an extra authorization parameter
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_params.insert(key.into(), value.into());
        self
    }

    /// Add a role mapping
    pub fn with_role_mapping(
        mut self,
        oauth_role: impl Into<String>,
        internal_role: impl Into<String>,
    ) -> Self {
        self.role_mapping
            .insert(oauth_role.into(), internal_role.into());
        self
    }

    /// Set the timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.client_id.is_empty() {
            return Err("OAuth client_id cannot be empty".to_string());
        }
        if self.auth_url.is_empty() {
            return Err("OAuth auth_url cannot be empty".to_string());
        }
        if self.token_url.is_empty() {
            return Err("OAuth token_url cannot be empty".to_string());
        }
        if self.redirect_uri.is_empty() {
            return Err("OAuth redirect_uri cannot be empty".to_string());
        }
        if !self.redirect_uri.starts_with("http://") && !self.redirect_uri.starts_with("https://") {
            return Err("OAuth redirect_uri must be an HTTP(S) URL".to_string());
        }
        Ok(())
    }

    /// Get the full scopes string
    pub fn scopes_string(&self) -> String {
        self.scopes.join(" ")
    }
}

/// Multi-provider OAuth configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthGatewayConfig {
    /// Registered OAuth providers
    #[serde(default)]
    pub providers: HashMap<String, OAuthConfig>,

    /// Default provider to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,

    /// Session TTL in seconds
    #[serde(default = "default_session_ttl")]
    pub session_ttl_seconds: u64,

    /// Whether to allow multiple providers for the same user
    #[serde(default)]
    pub allow_multiple_providers: bool,

    /// Default role for new OAuth users
    #[serde(default = "default_role")]
    pub default_role: String,

    /// Whether to auto-create users on first login
    #[serde(default = "default_true")]
    pub auto_create_users: bool,

    /// Allowed origins for post-login client redirects (scheme + host, e.g. "https://app.example.com").
    /// If non-empty, `client_redirect` URIs must match one of these origins; otherwise the redirect
    /// is silently replaced with `/`. An empty list disables the whitelist check (permissive).
    #[serde(default)]
    pub allowed_redirect_origins: Vec<String>,
}

fn default_session_ttl() -> u64 {
    3600
}

fn default_role() -> String {
    "user".to_string()
}

impl Default for OAuthGatewayConfig {
    fn default() -> Self {
        Self {
            providers: HashMap::new(),
            default_provider: None,
            session_ttl_seconds: default_session_ttl(),
            allow_multiple_providers: false,
            default_role: default_role(),
            auto_create_users: true,
            allowed_redirect_origins: Vec::new(),
        }
    }
}

impl OAuthGatewayConfig {
    /// Add a provider to the configuration
    pub fn add_provider(&mut self, name: impl Into<String>, config: OAuthConfig) {
        self.providers.insert(name.into(), config);
    }

    /// Get a provider by name
    pub fn get_provider(&self, name: &str) -> Option<&OAuthConfig> {
        self.providers.get(name)
    }

    /// Get the default provider configuration
    pub fn get_default_provider(&self) -> Option<&OAuthConfig> {
        self.default_provider
            .as_ref()
            .and_then(|name| self.providers.get(name))
    }

    /// Validate all provider configurations
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let errors: Vec<String> = self
            .providers
            .iter()
            .filter_map(|(name, config)| {
                config.validate().err().map(|e| format!("{}: {}", name, e))
            })
            .collect();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_provider_default_urls() {
        assert!(OAuthProvider::Google.default_auth_url().is_some());
        assert!(OAuthProvider::Google.default_token_url().is_some());
        assert!(OAuthProvider::Google.default_userinfo_url().is_some());

        assert!(OAuthProvider::Microsoft.default_auth_url().is_some());
        assert!(OAuthProvider::GitHub.default_auth_url().is_some());

        assert!(OAuthProvider::Custom.default_auth_url().is_none());
    }

    #[test]
    fn test_oauth_provider_scopes() {
        let google_scopes = OAuthProvider::Google.default_scopes();
        assert!(google_scopes.contains(&"openid".to_string()));
        assert!(google_scopes.contains(&"email".to_string()));

        let github_scopes = OAuthProvider::GitHub.default_scopes();
        assert!(github_scopes.contains(&"read:user".to_string()));
    }

    #[test]
    fn test_oauth_provider_pkce_support() {
        assert!(OAuthProvider::Google.supports_pkce());
        assert!(OAuthProvider::Microsoft.supports_pkce());
        assert!(!OAuthProvider::GitHub.supports_pkce());
    }

    #[test]
    fn test_oauth_provider_from_str() {
        assert_eq!(
            "google".parse::<OAuthProvider>().unwrap(),
            OAuthProvider::Google
        );
        assert_eq!(
            "microsoft".parse::<OAuthProvider>().unwrap(),
            OAuthProvider::Microsoft
        );
        assert_eq!(
            "azure".parse::<OAuthProvider>().unwrap(),
            OAuthProvider::Microsoft
        );
        assert_eq!(
            "github".parse::<OAuthProvider>().unwrap(),
            OAuthProvider::GitHub
        );
        assert!("unknown".parse::<OAuthProvider>().is_err());
    }

    #[test]
    fn test_oauth_provider_display() {
        assert_eq!(OAuthProvider::Google.to_string(), "google");
        assert_eq!(OAuthProvider::Microsoft.to_string(), "microsoft");
        assert_eq!(OAuthProvider::Custom.to_string(), "custom");
    }

    #[test]
    fn test_oauth_config_google() {
        let config = OAuthConfig::google("client123", "https://app.example.com/callback");

        assert_eq!(config.provider, OAuthProvider::Google);
        assert_eq!(config.client_id, "client123");
        assert_eq!(config.redirect_uri, "https://app.example.com/callback");
        assert!(config.auth_url.contains("accounts.google.com"));
        assert!(config.use_pkce);
        assert!(config.enabled);
    }

    #[test]
    fn test_oauth_config_github() {
        let config = OAuthConfig::github("client456", "https://app.example.com/callback");

        assert_eq!(config.provider, OAuthProvider::GitHub);
        assert!(!config.use_pkce); // GitHub doesn't support PKCE
    }

    #[test]
    fn test_oauth_config_okta() {
        let config = OAuthConfig::okta(
            "client789",
            "https://app.example.com/callback",
            "dev-123456.okta.com",
        );

        assert_eq!(config.provider, OAuthProvider::Okta);
        assert!(config.auth_url.contains("dev-123456.okta.com"));
        assert!(config.token_url.contains("dev-123456.okta.com"));
    }

    #[test]
    fn test_oauth_config_auth0() {
        let config = OAuthConfig::auth0(
            "client101",
            "https://app.example.com/callback",
            "dev-abc123.auth0.com",
        );

        assert_eq!(config.provider, OAuthProvider::Auth0);
        assert!(config.auth_url.contains("dev-abc123.auth0.com"));
        assert!(config.logout_url.unwrap().contains("v2/logout"));
    }

    #[test]
    fn test_oauth_config_custom() {
        let config = OAuthConfig::custom(
            "client202",
            "https://app.example.com/callback",
            "https://auth.custom.com/authorize",
            "https://auth.custom.com/token",
        );

        assert_eq!(config.provider, OAuthProvider::Custom);
        assert_eq!(config.auth_url, "https://auth.custom.com/authorize");
        assert_eq!(config.token_url, "https://auth.custom.com/token");
    }

    #[test]
    fn test_oauth_config_builder() {
        let config = OAuthConfig::google("client123", "https://app.example.com/callback")
            .with_client_secret("secret456")
            .add_scope("calendar.readonly")
            .with_param("prompt", "consent")
            .with_role_mapping("admin", "super_admin")
            .with_timeout(60000);

        assert_eq!(config.client_secret, Some("secret456".to_string()));
        assert!(config.scopes.contains(&"calendar.readonly".to_string()));
        assert_eq!(
            config.extra_params.get("prompt"),
            Some(&"consent".to_string())
        );
        assert_eq!(
            config.role_mapping.get("admin"),
            Some(&"super_admin".to_string())
        );
        assert_eq!(config.timeout_ms, 60000);
    }

    #[test]
    fn test_oauth_config_validation() {
        let valid_config = OAuthConfig::google("client123", "https://app.example.com/callback");
        assert!(valid_config.validate().is_ok());

        let mut invalid_config = OAuthConfig::google("", "https://app.example.com/callback");
        assert!(invalid_config.validate().is_err());

        invalid_config = OAuthConfig::google("client123", "not-a-url");
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_oauth_config_scopes_string() {
        let config = OAuthConfig::google("client123", "https://app.example.com/callback");
        let scopes = config.scopes_string();
        assert!(scopes.contains("openid"));
        assert!(scopes.contains("email"));
    }

    #[test]
    fn test_oauth_config_serialization() {
        let config = OAuthConfig::google("client123", "https://app.example.com/callback")
            .with_client_secret("secret456");

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("client123"));
        assert!(json.contains("google"));

        let parsed: OAuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.client_id, "client123");
        assert_eq!(parsed.provider, OAuthProvider::Google);
    }

    #[test]
    fn test_oauth_gateway_config() {
        let mut gateway = OAuthGatewayConfig::default();
        gateway.add_provider(
            "google",
            OAuthConfig::google("client1", "https://app.example.com/callback"),
        );
        gateway.add_provider(
            "github",
            OAuthConfig::github("client2", "https://app.example.com/callback"),
        );
        gateway.default_provider = Some("google".to_string());

        assert!(gateway.get_provider("google").is_some());
        assert!(gateway.get_provider("github").is_some());
        assert!(gateway.get_provider("unknown").is_none());
        assert!(gateway.get_default_provider().is_some());
    }

    #[test]
    fn test_oauth_gateway_config_validation() {
        let mut gateway = OAuthGatewayConfig::default();
        gateway.add_provider(
            "valid",
            OAuthConfig::google("client1", "https://app.example.com/callback"),
        );

        assert!(gateway.validate().is_ok());

        let mut invalid_config = OAuthConfig::google("", "https://app.example.com/callback");
        invalid_config.client_id = String::new();
        gateway.add_provider("invalid", invalid_config);

        assert!(gateway.validate().is_err());
    }

    #[test]
    fn test_oauth_gateway_config_defaults() {
        let gateway = OAuthGatewayConfig::default();
        assert_eq!(gateway.session_ttl_seconds, 3600);
        assert_eq!(gateway.default_role, "user");
        assert!(gateway.auto_create_users);
        assert!(!gateway.allow_multiple_providers);
    }
}
