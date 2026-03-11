//! MCP Gateway
//!
//! Main gateway for managing MCP servers and routing tool calls.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::config::{McpGatewayConfig, McpServerConfig};
use super::error::{McpError, McpResult};
use super::server::{McpServerHandle, McpServerRegistry};
use super::tools::{ToolCall, ToolList, ToolResult};

/// MCP Gateway - main entry point for MCP functionality
#[derive(Debug)]
pub struct McpGateway {
    /// Server registry
    registry: McpServerRegistry,

    /// Gateway configuration
    config: RwLock<McpGatewayConfig>,

    /// Server aliases (short name -> full name)
    aliases: RwLock<HashMap<String, String>>,
}

impl Default for McpGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl McpGateway {
    /// Create a new MCP Gateway
    pub fn new() -> Self {
        Self {
            registry: McpServerRegistry::new(),
            config: RwLock::new(McpGatewayConfig::default()),
            aliases: RwLock::new(HashMap::new()),
        }
    }

    /// Create a gateway from configuration
    pub async fn from_config(config: McpGatewayConfig) -> McpResult<Self> {
        let gateway = Self::new();

        // Register all servers from config
        for (name, mut server_config) in config.servers.clone() {
            // Ensure the name is set
            if server_config.name.is_empty() {
                server_config.name = name;
            }
            gateway.register_server(server_config).await?;
        }

        // Set up aliases
        *gateway.aliases.write().await = config.aliases.clone();
        *gateway.config.write().await = config;

        Ok(gateway)
    }

    /// Register a new MCP server
    pub async fn register_server(&self, config: McpServerConfig) -> McpResult<()> {
        self.registry.register(config).await
    }

    /// Unregister an MCP server
    pub async fn unregister_server(&self, name: &str) -> Option<McpServerHandle> {
        // Also remove any aliases pointing to this server
        let mut aliases = self.aliases.write().await;
        aliases.retain(|_, v| v != name);

        self.registry.remove(name).await
    }

    /// Get a server by name (resolves aliases)
    pub async fn get_server(&self, name: &str) -> McpResult<McpServerHandle> {
        let resolved_name = self.resolve_name(name).await;

        self.registry
            .get(&resolved_name)
            .await
            .ok_or_else(|| McpError::ServerNotFound {
                server_name: name.to_string(),
            })
    }

    /// Resolve a server name (handle aliases)
    async fn resolve_name(&self, name: &str) -> String {
        let aliases = self.aliases.read().await;
        aliases
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    /// Add an alias for a server
    pub async fn add_alias(&self, alias: &str, server_name: &str) -> McpResult<()> {
        // Verify server exists
        if self.registry.get(server_name).await.is_none() {
            return Err(McpError::ServerNotFound {
                server_name: server_name.to_string(),
            });
        }

        self.aliases
            .write()
            .await
            .insert(alias.to_string(), server_name.to_string());
        Ok(())
    }

    /// Connect to a specific server
    pub async fn connect(&self, server_name: &str) -> McpResult<()> {
        let server = self.get_server(server_name).await?;
        server.connect().await
    }

    /// Connect to all registered servers
    pub async fn connect_all(&self) -> Vec<(String, McpResult<()>)> {
        let names = self.registry.list_names().await;
        let mut results = Vec::new();

        for name in names {
            let result = self.connect(&name).await;
            results.push((name, result));
        }

        results
    }

    /// List all registered server names
    pub async fn list_servers(&self) -> Vec<String> {
        self.registry.list_names().await
    }

    /// List tools from a specific server
    pub async fn list_tools(&self, server_name: &str) -> McpResult<ToolList> {
        let server = self.get_server(server_name).await?;

        // Auto-connect if not connected
        if !server.is_connected().await {
            server.connect().await?;
        }

        server.list_tools().await
    }

    /// List tools from all servers
    pub async fn list_all_tools(&self) -> HashMap<String, McpResult<ToolList>> {
        let names = self.registry.list_names().await;
        let mut results = HashMap::new();

        for name in names {
            let result = self.list_tools(&name).await;
            results.insert(name, result);
        }

        results
    }

    /// Get all tools as OpenAI function format
    pub async fn get_openai_tools(&self) -> Vec<serde_json::Value> {
        let all_tools = self.list_all_tools().await;
        let mut functions = Vec::new();

        for (server_name, result) in all_tools {
            if let Ok(tool_list) = result {
                for tool in tool_list.tools {
                    let mut func = tool.to_openai_function();
                    // Prefix tool name with server name for disambiguation
                    if let Some(name) = func.get_mut("function")
                        && let Some(name_field) = name.get_mut("name")
                    {
                        *name_field = serde_json::Value::String(format!(
                            "mcp_{}__{}",
                            server_name,
                            name_field.as_str().unwrap_or("")
                        ));
                    }
                    functions.push(func);
                }
            }
        }

        functions
    }

    /// Call a tool on a specific server
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<ToolResult> {
        let server = self.get_server(server_name).await?;

        // Auto-connect if not connected
        if !server.is_connected().await {
            server.connect().await?;
        }

        let call = ToolCall::new(tool_name, arguments);
        server.call_tool(call).await
    }

    /// Call a tool using prefixed name (e.g., "mcp_github__get_repo")
    pub async fn call_prefixed_tool(
        &self,
        prefixed_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<ToolResult> {
        // Parse the prefixed name
        let (server_name, tool_name) = self.parse_prefixed_name(prefixed_name)?;
        self.call_tool(&server_name, &tool_name, arguments).await
    }

    /// Parse a prefixed tool name into (server_name, tool_name)
    fn parse_prefixed_name(&self, prefixed: &str) -> McpResult<(String, String)> {
        // Format: mcp_{server}__{tool}
        if !prefixed.starts_with("mcp_") {
            return Err(McpError::ToolNotFound {
                server_name: "unknown".to_string(),
                tool_name: prefixed.to_string(),
            });
        }

        let rest = &prefixed[4..]; // Remove "mcp_" prefix
        if let Some(idx) = rest.find("__") {
            let server = rest[..idx].to_string();
            let tool = rest[idx + 2..].to_string();
            Ok((server, tool))
        } else {
            Err(McpError::ToolNotFound {
                server_name: "unknown".to_string(),
                tool_name: prefixed.to_string(),
            })
        }
    }

    /// Get gateway statistics
    pub async fn stats(&self) -> GatewayStats {
        let server_names = self.registry.list_names().await;
        let mut connected = 0;
        let mut total_tools = 0;

        for name in &server_names {
            if let Ok(server) = self.get_server(name).await
                && server.is_connected().await
            {
                connected += 1;
                if let Ok(tools) = server.list_tools().await {
                    total_tools += tools.tools.len();
                }
            }
        }

        GatewayStats {
            total_servers: server_names.len(),
            connected_servers: connected,
            total_tools,
        }
    }

    /// Health check for the gateway
    pub async fn health_check(&self) -> GatewayHealth {
        let stats = self.stats().await;
        let healthy = stats.connected_servers > 0 || stats.total_servers == 0;

        GatewayHealth {
            healthy,
            stats,
            servers: self.server_health().await,
        }
    }

    /// Get health status for each server
    async fn server_health(&self) -> HashMap<String, ServerHealth> {
        let names = self.registry.list_names().await;
        let mut health = HashMap::new();

        for name in names {
            if let Ok(server) = self.get_server(&name).await {
                let state = server.state().await;
                health.insert(
                    name,
                    ServerHealth {
                        connected: server.is_connected().await,
                        state: format!("{:?}", state),
                    },
                );
            }
        }

        health
    }
}

/// Gateway statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct GatewayStats {
    /// Total registered servers
    pub total_servers: usize,
    /// Currently connected servers
    pub connected_servers: usize,
    /// Total available tools across all servers
    pub total_tools: usize,
}

/// Gateway health status
#[derive(Debug, Clone, serde::Serialize)]
pub struct GatewayHealth {
    /// Whether the gateway is healthy
    pub healthy: bool,
    /// Gateway statistics
    pub stats: GatewayStats,
    /// Per-server health
    pub servers: HashMap<String, ServerHealth>,
}

/// Individual server health
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerHealth {
    /// Whether the server is connected
    pub connected: bool,
    /// Server state
    pub state: String,
}

/// Thread-safe gateway handle
pub type McpGatewayHandle = Arc<McpGateway>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gateway_creation() {
        let gateway = McpGateway::new();
        assert_eq!(gateway.list_servers().await.len(), 0);
    }

    #[tokio::test]
    async fn test_register_server() {
        let gateway = McpGateway::new();

        gateway
            .register_server(McpServerConfig::new("test", "https://example.com/mcp"))
            .await
            .unwrap();

        let servers = gateway.list_servers().await;
        assert_eq!(servers.len(), 1);
        assert!(servers.contains(&"test".to_string()));
    }

    #[tokio::test]
    async fn test_unregister_server() {
        let gateway = McpGateway::new();

        gateway
            .register_server(McpServerConfig::new("test", "https://example.com/mcp"))
            .await
            .unwrap();

        let removed = gateway.unregister_server("test").await;
        assert!(removed.is_some());
        assert_eq!(gateway.list_servers().await.len(), 0);
    }

    #[tokio::test]
    async fn test_get_server() {
        let gateway = McpGateway::new();

        gateway
            .register_server(McpServerConfig::new("test", "https://example.com/mcp"))
            .await
            .unwrap();

        let server = gateway.get_server("test").await;
        assert!(server.is_ok());

        let not_found = gateway.get_server("nonexistent").await;
        assert!(matches!(not_found, Err(McpError::ServerNotFound { .. })));
    }

    #[tokio::test]
    async fn test_aliases() {
        let gateway = McpGateway::new();

        gateway
            .register_server(McpServerConfig::new(
                "github_mcp_server",
                "https://api.github.com/mcp",
            ))
            .await
            .unwrap();

        gateway
            .add_alias("github", "github_mcp_server")
            .await
            .unwrap();

        // Should be able to get server by alias
        let server = gateway.get_server("github").await;
        assert!(server.is_ok());
        assert_eq!(server.unwrap().name(), "github_mcp_server");
    }

    #[tokio::test]
    async fn test_alias_nonexistent_server() {
        let gateway = McpGateway::new();

        let result = gateway.add_alias("alias", "nonexistent").await;
        assert!(matches!(result, Err(McpError::ServerNotFound { .. })));
    }

    #[tokio::test]
    async fn test_parse_prefixed_name() {
        let gateway = McpGateway::new();

        let (server, tool) = gateway.parse_prefixed_name("mcp_github__get_repo").unwrap();
        assert_eq!(server, "github");
        assert_eq!(tool, "get_repo");
    }

    #[tokio::test]
    async fn test_parse_prefixed_name_invalid() {
        let gateway = McpGateway::new();

        // No mcp_ prefix
        assert!(gateway.parse_prefixed_name("github__get_repo").is_err());

        // No double underscore
        assert!(gateway.parse_prefixed_name("mcp_github_get_repo").is_err());
    }

    #[tokio::test]
    async fn test_gateway_stats_empty() {
        let gateway = McpGateway::new();
        let stats = gateway.stats().await;

        assert_eq!(stats.total_servers, 0);
        assert_eq!(stats.connected_servers, 0);
        assert_eq!(stats.total_tools, 0);
    }

    #[tokio::test]
    async fn test_gateway_health_empty() {
        let gateway = McpGateway::new();
        let health = gateway.health_check().await;

        // Empty gateway should be healthy
        assert!(health.healthy);
    }

    #[tokio::test]
    async fn test_from_config() {
        let mut config = McpGatewayConfig::default();
        config.add_server(McpServerConfig::new("test", "https://example.com/mcp"));

        let gateway = McpGateway::from_config(config).await.unwrap();
        assert_eq!(gateway.list_servers().await.len(), 1);
    }
}
