//! MCP Server Connection
//!
//! Manages connections to individual MCP servers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::config::McpServerConfig;
use super::error::{McpError, McpResult};
use super::protocol::{
    ClientInfo, InitializeParams, JsonRpcRequest, JsonRpcResponse, McpCapabilities, methods,
};
use super::tools::{Tool, ToolCall, ToolList, ToolResult};
use super::transport::Transport;
use crate::utils::net::http::get_client_with_timeout;

/// MCP Server connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    /// Server not connected
    Disconnected,
    /// Connection in progress
    Connecting,
    /// Connected and initialized
    Connected,
    /// Connection failed
    Failed,
}

/// MCP Server connection
///
/// Uses a shared HTTP client pool for optimal connection reuse.
#[derive(Debug)]
pub struct McpServer {
    /// Server configuration
    config: McpServerConfig,

    /// Connection state
    state: RwLock<ServerState>,

    /// HTTP client from shared pool (for HTTP/SSE transports)
    http_client: Arc<reqwest::Client>,

    /// Custom headers for this server
    custom_headers: reqwest::header::HeaderMap,

    /// Cached tools list
    tools_cache: RwLock<Option<Vec<Tool>>>,

    /// Server capabilities (from initialize response)
    capabilities: RwLock<Option<McpCapabilities>>,

    /// Request ID counter
    request_id: std::sync::atomic::AtomicU64,
}

impl McpServer {
    /// Create a new MCP server connection
    pub fn new(config: McpServerConfig) -> McpResult<Self> {
        config
            .validate()
            .map_err(|e| McpError::ConfigurationError { message: e })?;

        // Get shared client with appropriate timeout
        let timeout_secs = config.timeout_ms / 1000;
        let http_client = get_client_with_timeout(Duration::from_secs(timeout_secs.max(1)));

        // Build custom headers for this server
        let mut headers = reqwest::header::HeaderMap::new();
        for (key, value) in &config.static_headers {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                reqwest::header::HeaderValue::from_str(value),
            ) {
                headers.insert(name, val);
            }
        }

        // Add auth header if configured
        if let Some(auth) = &config.auth
            && let Some(header_value) = auth.get_header_value()
        {
            let header_name = auth.get_header_name();
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(header_name.as_bytes()),
                reqwest::header::HeaderValue::from_str(&header_value),
            ) {
                headers.insert(name, val);
            }
        }

        Ok(Self {
            config,
            state: RwLock::new(ServerState::Disconnected),
            http_client,
            custom_headers: headers,
            tools_cache: RwLock::new(None),
            capabilities: RwLock::new(None),
            request_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    /// Get server name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get server URL
    pub fn url(&self) -> &str {
        &self.config.url
    }

    /// Get transport type
    pub fn transport(&self) -> Transport {
        self.config.transport
    }

    /// Check if server is connected
    pub async fn is_connected(&self) -> bool {
        *self.state.read().await == ServerState::Connected
    }

    /// Get current state
    pub async fn state(&self) -> ServerState {
        *self.state.read().await
    }

    /// Connect and initialize the server
    pub async fn connect(&self) -> McpResult<()> {
        {
            let mut state = self.state.write().await;
            if *state == ServerState::Connected {
                return Ok(());
            }
            *state = ServerState::Connecting;
        }

        match self.initialize().await {
            Ok(caps) => {
                *self.capabilities.write().await = Some(caps);
                *self.state.write().await = ServerState::Connected;
                Ok(())
            }
            Err(e) => {
                *self.state.write().await = ServerState::Failed;
                Err(e)
            }
        }
    }

    /// Initialize the MCP connection
    async fn initialize(&self) -> McpResult<McpCapabilities> {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: McpCapabilities::default(),
            client_info: ClientInfo::default(),
        };

        let response = self
            .send_request(methods::INITIALIZE, Some(serde_json::to_value(params)?))
            .await?;

        if let Some(result) = response.result {
            let caps: McpCapabilities =
                serde_json::from_value(result.get("capabilities").cloned().unwrap_or_default())
                    .unwrap_or_default();
            Ok(caps)
        } else if let Some(error) = response.error {
            Err(McpError::ProtocolError {
                message: format!("Initialize failed: {}", error.message),
            })
        } else {
            Ok(McpCapabilities::default())
        }
    }

    /// Disconnect from the server
    pub async fn disconnect(&self) {
        *self.state.write().await = ServerState::Disconnected;
        *self.tools_cache.write().await = None;
        *self.capabilities.write().await = None;
    }

    /// List available tools
    pub async fn list_tools(&self) -> McpResult<ToolList> {
        // Check cache first
        if let Some(tools) = self.tools_cache.read().await.as_ref() {
            return Ok(ToolList {
                tools: tools.clone(),
                next_cursor: None,
            });
        }

        let response = self.send_request(methods::LIST_TOOLS, None).await?;

        if let Some(result) = response.result {
            let list: ToolList = serde_json::from_value(result)?;

            // Cache the tools
            *self.tools_cache.write().await = Some(list.tools.clone());

            Ok(list)
        } else if let Some(error) = response.error {
            Err(McpError::ProtocolError {
                message: format!("List tools failed: {}", error.message),
            })
        } else {
            Ok(ToolList::empty())
        }
    }

    /// Get a specific tool by name
    pub async fn get_tool(&self, name: &str) -> McpResult<Option<Tool>> {
        let list = self.list_tools().await?;
        Ok(list.tools.into_iter().find(|t| t.name == name))
    }

    /// Call a tool
    pub async fn call_tool(&self, call: ToolCall) -> McpResult<ToolResult> {
        let params = serde_json::json!({
            "name": call.name,
            "arguments": call.arguments
        });

        let response = self.send_request(methods::CALL_TOOL, Some(params)).await?;

        if let Some(result) = response.result {
            let tool_result: ToolResult = serde_json::from_value(result)?;
            Ok(tool_result)
        } else if let Some(error) = response.error {
            Err(McpError::ToolExecutionError {
                server_name: self.config.name.clone(),
                tool_name: call.name,
                code: error.code,
                message: error.message,
            })
        } else {
            Err(McpError::ProtocolError {
                message: "Empty response from tool call".to_string(),
            })
        }
    }

    /// Send a JSON-RPC request
    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> McpResult<JsonRpcResponse> {
        match self.config.transport {
            Transport::Http => self.send_http_request(method, params).await,
            Transport::Sse => self.send_sse_request(method, params).await,
            Transport::Stdio => Err(McpError::TransportError {
                transport: "stdio".to_string(),
                message: "Stdio transport not yet implemented".to_string(),
            }),
            Transport::WebSocket => Err(McpError::TransportError {
                transport: "websocket".to_string(),
                message: "WebSocket transport not yet implemented".to_string(),
            }),
        }
    }

    /// Send HTTP request
    async fn send_http_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> McpResult<JsonRpcResponse> {
        let id = self
            .request_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let request =
            JsonRpcRequest::new(method, params).with_id(serde_json::Value::Number(id.into()));

        let response = self
            .http_client
            .post(&self.config.url)
            .headers(self.custom_headers.clone())
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    McpError::Timeout {
                        server_name: self.config.name.clone(),
                        timeout_ms: self.config.timeout_ms,
                    }
                } else if e.is_connect() {
                    McpError::ConnectionError {
                        server_name: self.config.name.clone(),
                        message: e.to_string(),
                    }
                } else {
                    McpError::TransportError {
                        transport: "http".to_string(),
                        message: e.to_string(),
                    }
                }
            })?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(McpError::AuthenticationError {
                server_name: self.config.name.clone(),
                message: "Unauthorized".to_string(),
            });
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(McpError::AuthorizationError {
                server_name: self.config.name.clone(),
                tool_name: None,
                message: "Forbidden".to_string(),
            });
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .map(|s| s * 1000);

            return Err(McpError::RateLimitExceeded {
                server_name: self.config.name.clone(),
                retry_after_ms: retry_after,
            });
        }

        let rpc_response: JsonRpcResponse =
            response.json().await.map_err(|e| McpError::ProtocolError {
                message: format!("Failed to parse response: {}", e),
            })?;

        Ok(rpc_response)
    }

    /// Send SSE request (for now, falls back to HTTP POST)
    async fn send_sse_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> McpResult<JsonRpcResponse> {
        // For non-streaming requests, SSE can use regular HTTP POST
        // Streaming would require a different implementation
        self.send_http_request(method, params).await
    }

    /// Invalidate tools cache
    pub async fn invalidate_cache(&self) {
        *self.tools_cache.write().await = None;
    }

    /// Get server capabilities
    pub async fn capabilities(&self) -> Option<McpCapabilities> {
        self.capabilities.read().await.clone()
    }
}

/// Thread-safe MCP Server handle
pub type McpServerHandle = Arc<McpServer>;

/// MCP Server registry
#[derive(Debug, Default)]
pub struct McpServerRegistry {
    servers: RwLock<HashMap<String, McpServerHandle>>,
}

impl McpServerRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a server
    pub async fn register(&self, config: McpServerConfig) -> McpResult<()> {
        let name = config.name.clone();

        if self.servers.read().await.contains_key(&name) {
            return Err(McpError::ServerAlreadyExists { server_name: name });
        }

        let server = Arc::new(McpServer::new(config)?);
        self.servers.write().await.insert(name, server);
        Ok(())
    }

    /// Get a server by name
    pub async fn get(&self, name: &str) -> Option<McpServerHandle> {
        self.servers.read().await.get(name).cloned()
    }

    /// Remove a server
    pub async fn remove(&self, name: &str) -> Option<McpServerHandle> {
        self.servers.write().await.remove(name)
    }

    /// List all server names
    pub async fn list_names(&self) -> Vec<String> {
        self.servers.read().await.keys().cloned().collect()
    }

    /// Get server count
    pub async fn count(&self) -> usize {
        self.servers.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::mcp::config::AuthConfig;

    #[test]
    fn test_server_state_variants() {
        assert_eq!(ServerState::Disconnected, ServerState::Disconnected);
        assert_ne!(ServerState::Connected, ServerState::Disconnected);
    }

    #[tokio::test]
    async fn test_server_creation() {
        let config = McpServerConfig::new("test", "https://example.com/mcp");
        let server = McpServer::new(config).unwrap();

        assert_eq!(server.name(), "test");
        assert_eq!(server.url(), "https://example.com/mcp");
        assert!(!server.is_connected().await);
    }

    #[tokio::test]
    async fn test_server_with_auth() {
        let config = McpServerConfig::new("test", "https://example.com/mcp")
            .with_auth(AuthConfig::bearer("token123"));

        let server = McpServer::new(config).unwrap();
        assert_eq!(server.name(), "test");
    }

    #[tokio::test]
    async fn test_server_registry() {
        let registry = McpServerRegistry::new();

        // Register a server
        registry
            .register(McpServerConfig::new("server1", "https://example.com/mcp1"))
            .await
            .unwrap();

        // Should exist
        assert!(registry.get("server1").await.is_some());
        assert!(registry.get("nonexistent").await.is_none());

        // Count
        assert_eq!(registry.count().await, 1);

        // List names
        let names = registry.list_names().await;
        assert!(names.contains(&"server1".to_string()));
    }

    #[tokio::test]
    async fn test_registry_duplicate_server() {
        let registry = McpServerRegistry::new();

        registry
            .register(McpServerConfig::new("server1", "https://example.com/mcp1"))
            .await
            .unwrap();

        // Registering again should fail
        let result = registry
            .register(McpServerConfig::new("server1", "https://example.com/mcp2"))
            .await;

        assert!(matches!(result, Err(McpError::ServerAlreadyExists { .. })));
    }

    #[tokio::test]
    async fn test_registry_remove() {
        let registry = McpServerRegistry::new();

        registry
            .register(McpServerConfig::new("server1", "https://example.com/mcp1"))
            .await
            .unwrap();

        let removed = registry.remove("server1").await;
        assert!(removed.is_some());
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_server_initial_state() {
        let config = McpServerConfig::new("test", "https://example.com/mcp");
        let server = McpServer::new(config).unwrap();

        assert_eq!(server.state().await, ServerState::Disconnected);
    }

    #[test]
    fn test_invalid_config() {
        let config = McpServerConfig {
            name: "".to_string(), // Invalid: empty name
            url: "https://example.com".to_string(),
            ..Default::default()
        };

        let result = McpServer::new(config);
        assert!(result.is_err());
    }
}
