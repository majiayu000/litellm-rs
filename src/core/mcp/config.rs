//! MCP Server Configuration
//!
//! Configuration types for MCP servers including authentication and transport settings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::transport::Transport;
use crate::core::net::validate_outbound_url_str;

/// MCP Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name/alias (used as identifier)
    pub name: String,

    /// Server URL or command path (for stdio transport)
    pub url: String,

    /// Transport protocol
    #[serde(default)]
    pub transport: Transport,

    /// Authentication configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,

    /// Static headers to send with every request
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub static_headers: HashMap<String, String>,

    /// Headers from client to forward to MCP server
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forward_headers: Vec<String>,

    /// Connection timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Whether this server is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Rate limit: max requests per minute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_rpm: Option<u32>,

    /// Description of this MCP server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Tags for categorization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// OpenAPI spec path for automatic tool generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec_path: Option<String>,
}

fn default_timeout() -> u64 {
    30000 // 30 seconds
}

fn default_enabled() -> bool {
    true
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            url: String::new(),
            transport: Transport::default(),
            auth: None,
            static_headers: HashMap::new(),
            forward_headers: Vec::new(),
            timeout_ms: default_timeout(),
            enabled: true,
            rate_limit_rpm: None,
            description: None,
            tags: Vec::new(),
            spec_path: None,
        }
    }
}

impl McpServerConfig {
    /// Create a new MCP server config with name and URL
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set transport type
    pub fn with_transport(mut self, transport: Transport) -> Self {
        self.transport = transport;
        self
    }

    /// Set authentication
    pub fn with_auth(mut self, auth: AuthConfig) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Add a static header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.static_headers.insert(key.into(), value.into());
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Server name cannot be empty".to_string());
        }
        if self.url.is_empty() {
            return Err("Server URL cannot be empty".to_string());
        }

        // Validate URL based on transport
        match self.transport {
            Transport::Http | Transport::Sse => {
                if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
                    return Err(format!(
                        "HTTP/SSE transport requires http:// or https:// URL, got: {}",
                        self.url
                    ));
                }
                validate_outbound_url_str(&self.url).map_err(|error| error.to_string())?;
            }
            Transport::Stdio => {
                // For stdio, URL is a command path - no strict validation
            }
            Transport::WebSocket => {
                if !self.url.starts_with("ws://") && !self.url.starts_with("wss://") {
                    return Err(format!(
                        "WebSocket transport requires ws:// or wss:// URL, got: {}",
                        self.url
                    ));
                }
                validate_outbound_url_str(&self.url).map_err(|error| error.to_string())?;
            }
        }

        // Validate auth if present
        if let Some(auth) = &self.auth {
            auth.validate()?;
        }

        Ok(())
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Authentication type
    #[serde(rename = "type")]
    pub auth_type: McpAuthType,

    /// Authentication value (API key, token, etc.)
    /// Can use environment variable syntax: ${ENV_VAR}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// OAuth 2.0 client ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    /// OAuth 2.0 client secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// OAuth 2.0 token URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,

    /// OAuth 2.0 scopes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,

    /// Header name for API key (default: Authorization)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_name: Option<String>,

    /// Header prefix (e.g., "Bearer", "Api-Key")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_prefix: Option<String>,
}

impl AuthConfig {
    /// Create API key authentication
    pub fn api_key(value: impl Into<String>) -> Self {
        Self {
            auth_type: McpAuthType::ApiKey,
            value: Some(value.into()),
            client_id: None,
            client_secret: None,
            token_url: None,
            scopes: Vec::new(),
            header_name: None,
            header_prefix: None,
        }
    }

    /// Create Bearer token authentication
    pub fn bearer(token: impl Into<String>) -> Self {
        Self {
            auth_type: McpAuthType::BearerToken,
            value: Some(token.into()),
            client_id: None,
            client_secret: None,
            token_url: None,
            scopes: Vec::new(),
            header_name: None,
            header_prefix: Some("Bearer".to_string()),
        }
    }

    /// Create Basic authentication
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let credentials = format!("{}:{}", username.into(), password.into());
        let encoded = STANDARD.encode(credentials.as_bytes());
        Self {
            auth_type: McpAuthType::Basic,
            value: Some(encoded),
            client_id: None,
            client_secret: None,
            token_url: None,
            scopes: Vec::new(),
            header_name: None,
            header_prefix: Some("Basic".to_string()),
        }
    }

    /// Create OAuth 2.0 client credentials authentication
    pub fn oauth2(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        Self {
            auth_type: McpAuthType::OAuth2,
            value: None,
            client_id: Some(client_id.into()),
            client_secret: Some(client_secret.into()),
            token_url: Some(token_url.into()),
            scopes: Vec::new(),
            header_name: None,
            header_prefix: None,
        }
    }

    /// Add OAuth 2.0 scopes
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    /// Set custom header name
    pub fn with_header_name(mut self, name: impl Into<String>) -> Self {
        self.header_name = Some(name.into());
        self
    }

    /// Validate the authentication configuration
    pub fn validate(&self) -> Result<(), String> {
        match self.auth_type {
            McpAuthType::ApiKey | McpAuthType::BearerToken => {
                if self.value.is_none() {
                    return Err(format!(
                        "{:?} authentication requires a value",
                        self.auth_type
                    ));
                }
            }
            McpAuthType::Basic => {
                if self.value.is_none() {
                    return Err("Basic authentication requires credentials".to_string());
                }
            }
            McpAuthType::OAuth2 => {
                if self.client_id.is_none() {
                    return Err("OAuth2 authentication requires client_id".to_string());
                }
                if self.client_secret.is_none() {
                    return Err("OAuth2 authentication requires client_secret".to_string());
                }
                if self.token_url.is_none() {
                    return Err("OAuth2 authentication requires token_url".to_string());
                }
            }
            McpAuthType::None => {}
        }
        Ok(())
    }

    /// Get the authorization header value
    pub fn get_header_value(&self) -> Option<String> {
        match self.auth_type {
            McpAuthType::None => None,
            McpAuthType::ApiKey => self.value.as_ref().map(|v| {
                if let Some(prefix) = &self.header_prefix {
                    format!("{} {}", prefix, v)
                } else {
                    v.clone()
                }
            }),
            McpAuthType::BearerToken => self.value.as_ref().map(|v| format!("Bearer {}", v)),
            McpAuthType::Basic => self.value.as_ref().map(|v| format!("Basic {}", v)),
            McpAuthType::OAuth2 => {
                // OAuth2 tokens are obtained separately
                None
            }
        }
    }

    /// Get the header name to use
    pub fn get_header_name(&self) -> &str {
        self.header_name.as_deref().unwrap_or("Authorization")
    }
}

/// Authentication type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpAuthType {
    /// No authentication
    #[default]
    None,
    /// API key authentication
    ApiKey,
    /// Bearer token authentication
    BearerToken,
    /// Basic authentication (username:password)
    Basic,
    /// OAuth 2.0 client credentials
    OAuth2,
}

/// Backward-compatible alias for the prefixed MCP authentication type.
#[doc(hidden)]
pub type AuthType = McpAuthType;

/// MCP gateway configuration (collection of servers)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpGatewayConfig {
    /// Registered MCP servers
    #[serde(default)]
    pub servers: HashMap<String, McpServerConfig>,

    /// Enable MCP in database storage
    #[serde(default)]
    pub store_in_db: bool,

    /// Global rate limit (requests per minute)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_rate_limit: Option<u32>,

    /// Default timeout for all servers
    #[serde(default = "default_timeout")]
    pub default_timeout_ms: u64,

    /// Server aliases for shorter names
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub aliases: HashMap<String, String>,
}

impl McpGatewayConfig {
    /// Add a server to the configuration
    pub fn add_server(&mut self, config: McpServerConfig) {
        self.servers.insert(config.name.clone(), config);
    }

    /// Get a server by name or alias
    pub fn get_server(&self, name: &str) -> Option<&McpServerConfig> {
        // Check direct name first
        if let Some(server) = self.servers.get(name) {
            return Some(server);
        }
        // Check aliases
        if let Some(real_name) = self.aliases.get(name) {
            return self.servers.get(real_name);
        }
        None
    }

    /// Resolve a server name (handles aliases)
    pub fn resolve_name<'a>(&'a self, name: &'a str) -> Option<&'a str> {
        if self.servers.contains_key(name) {
            return Some(name);
        }
        self.aliases.get(name).map(|s| s.as_str())
    }

    /// Validate all server configurations
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let errors: Vec<String> = self
            .servers
            .values()
            .filter_map(|s| s.validate().err())
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
    fn test_server_config_new() {
        let config = McpServerConfig::new("github", "https://api.github.com/mcp");
        assert_eq!(config.name, "github");
        assert_eq!(config.url, "https://api.github.com/mcp");
        assert!(config.enabled);
    }

    #[test]
    fn test_server_config_builder() {
        let config = McpServerConfig::new("github", "https://api.github.com/mcp")
            .with_transport(Transport::Http)
            .with_auth(AuthConfig::bearer("token123"))
            .with_timeout(5000)
            .with_description("GitHub MCP Server");

        assert_eq!(config.transport, Transport::Http);
        assert!(config.auth.is_some());
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.description.as_deref(), Some("GitHub MCP Server"));
    }

    #[test]
    fn test_server_config_validation_empty_name() {
        let config = McpServerConfig {
            name: "".to_string(),
            url: "https://example.com".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_config_validation_empty_url() {
        let config = McpServerConfig {
            name: "test".to_string(),
            url: "".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_config_validation_invalid_http_url() {
        let config = McpServerConfig {
            name: "test".to_string(),
            url: "ftp://example.com".to_string(),
            transport: Transport::Http,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_config_validation_valid_http() {
        let config =
            McpServerConfig::new("test", "https://example.com").with_transport(Transport::Http);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_server_config_validation_rejects_http_private_targets() {
        for url in [
            "http://127.0.0.1:8080/mcp",
            "http://localhost:8080/mcp",
            "http://169.254.169.254/latest/meta-data/",
            "http://10.0.0.1/mcp",
        ] {
            let config = McpServerConfig::new("test", url).with_transport(Transport::Http);
            let error = config.validate().expect_err("private target must fail");
            assert!(
                error.contains("private or reserved"),
                "unexpected error for {url}: {error}"
            );
        }
    }

    #[test]
    fn test_server_config_validation_rejects_websocket_private_targets() {
        let config = McpServerConfig::new("test", "ws://[::1]:9000/mcp")
            .with_transport(Transport::WebSocket);
        let error = config
            .validate()
            .expect_err("private websocket target must fail");
        assert!(error.contains("private or reserved"));
    }

    #[test]
    fn test_server_config_validation_allows_public_websocket() {
        let config = McpServerConfig::new("test", "wss://mcp.example.com/socket")
            .with_transport(Transport::WebSocket);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_server_config_validation_leaves_stdio_paths_alone() {
        let config = McpServerConfig::new("local", "/usr/local/bin/mcp-server")
            .with_transport(Transport::Stdio);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_auth_config_api_key() {
        let auth = AuthConfig::api_key("my-api-key");
        assert_eq!(auth.auth_type, McpAuthType::ApiKey);
        assert_eq!(auth.value.as_deref(), Some("my-api-key"));
        assert!(auth.validate().is_ok());
    }

    #[test]
    fn test_auth_config_bearer() {
        let auth = AuthConfig::bearer("my-token");
        assert_eq!(auth.auth_type, McpAuthType::BearerToken);
        assert_eq!(auth.get_header_value(), Some("Bearer my-token".to_string()));
    }

    #[test]
    fn test_auth_config_basic() {
        let auth = AuthConfig::basic("user", "pass");
        assert_eq!(auth.auth_type, McpAuthType::Basic);
        let header = auth.get_header_value().unwrap();
        assert!(header.starts_with("Basic "));
    }

    #[test]
    fn test_auth_config_oauth2() {
        let auth = AuthConfig::oauth2(
            "client-id",
            "client-secret",
            "https://auth.example.com/token",
        )
        .with_scopes(vec!["read".to_string(), "write".to_string()]);
        assert_eq!(auth.auth_type, McpAuthType::OAuth2);
        assert_eq!(auth.client_id.as_deref(), Some("client-id"));
        assert_eq!(auth.scopes.len(), 2);
        assert!(auth.validate().is_ok());
    }

    #[test]
    fn test_auth_config_oauth2_missing_client_id() {
        let auth = AuthConfig {
            auth_type: McpAuthType::OAuth2,
            value: None,
            client_id: None,
            client_secret: Some("secret".to_string()),
            token_url: Some("https://auth.example.com/token".to_string()),
            scopes: Vec::new(),
            header_name: None,
            header_prefix: None,
        };
        assert!(auth.validate().is_err());
    }

    #[test]
    fn test_gateway_config_add_server() {
        let mut config = McpGatewayConfig::default();
        config.add_server(McpServerConfig::new("github", "https://api.github.com/mcp"));
        assert!(config.servers.contains_key("github"));
    }

    #[test]
    fn test_gateway_config_aliases() {
        let mut config = McpGatewayConfig::default();
        config.add_server(McpServerConfig::new(
            "github_mcp_server",
            "https://api.github.com/mcp",
        ));
        config
            .aliases
            .insert("github".to_string(), "github_mcp_server".to_string());

        // Can get by full name
        assert!(config.get_server("github_mcp_server").is_some());
        // Can get by alias
        assert!(config.get_server("github").is_some());
    }

    #[test]
    fn test_auth_type_default() {
        let auth_type = McpAuthType::default();
        assert_eq!(auth_type, McpAuthType::None);
    }

    #[test]
    fn test_server_config_serde() {
        let config = McpServerConfig::new("github", "https://api.github.com/mcp")
            .with_auth(AuthConfig::bearer("token123"));

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: McpServerConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "github");
        assert!(deserialized.auth.is_some());
    }
}
