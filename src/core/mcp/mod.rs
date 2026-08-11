//! MCP (Model Context Protocol) Gateway
//!
//! This module implements MCP support for litellm-rs, providing a unified gateway
//! for connecting MCP servers (tools) to any LLM provider.
//!
//! # Overview
//!
//! MCP (Model Context Protocol) is a standard for connecting external tools and
//! data sources to LLMs. This implementation supports:
//!
//! - Multiple transport protocols (HTTP, SSE, stdio)
//! - OAuth 2.0 and API key authentication
//! - Permission control by Key, Team, and Organization
//! - Dynamic tool discovery and registration
//! - Cost tracking for tool invocations
//!
//! # Usage
//!
//! ```rust,no_run
//! # use litellm_rs::core::mcp::{McpGateway, McpServerConfig, Transport, AuthConfig};
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Configure MCP servers
//! let config = McpServerConfig {
//!     name: "github".to_string(),
//!     url: "https://api.github.com/mcp".parse()?,
//!     transport: Transport::Http,
//!     auth: Some(AuthConfig::bearer("token123")),
//!     ..Default::default()
//! };
//!
//! // Create gateway and register server
//! let gateway = McpGateway::new();
//! gateway.register_server(config).await?;
//!
//! // List available tools
//! let tools = gateway.list_tools("github").await?;
//!
//! // Call a tool
//! let params = serde_json::json!({});
//! let result = gateway.call_tool("github", "get_repo", params).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Configuration
//!
//! MCP servers can be configured via YAML:
//!
//! ```yaml
//! mcp_servers:
//!   github:
//!     url: "https://api.github.com/mcp"
//!     transport: http
//!     auth_type: bearer_token
//!     auth_value: "${GITHUB_TOKEN}"
//!
//!   local_tools:
//!     url: "/path/to/mcp-server"
//!     transport: stdio
//! ```

pub mod config;
pub mod error;
pub mod gateway;
pub mod permissions;
pub mod protocol;
pub mod server;
pub mod tools;
pub mod transport;
pub mod validation;

// Re-export commonly used types
#[doc(hidden)]
#[deprecated(
    since = "0.6.0",
    note = "core::mcp is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use config::AuthType;
#[deprecated(
    since = "0.6.0",
    note = "core::mcp is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use config::{AuthConfig, McpAuthType, McpServerConfig};
#[deprecated(
    since = "0.6.0",
    note = "core::mcp is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use error::{McpError, McpResult};
#[deprecated(
    since = "0.6.0",
    note = "core::mcp is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use gateway::McpGateway;
#[doc(hidden)]
#[deprecated(
    since = "0.6.0",
    note = "core::mcp is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use permissions::PermissionLevel;
#[deprecated(
    since = "0.6.0",
    note = "core::mcp is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use permissions::{McpPermissionLevel, PermissionManager, PermissionPolicy, PermissionRule};
#[deprecated(
    since = "0.6.0",
    note = "core::mcp is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use protocol::{JsonRpcRequest, JsonRpcResponse, McpMessage};
#[deprecated(
    since = "0.6.0",
    note = "core::mcp is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use server::McpServer;
#[deprecated(
    since = "0.6.0",
    note = "core::mcp is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use tools::{Tool, ToolCall, ToolResult};
#[deprecated(
    since = "0.6.0",
    note = "core::mcp is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use transport::Transport;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let transport = transport::Transport::Http;
        let auth_type = config::McpAuthType::ApiKey;
        assert!(matches!(transport, transport::Transport::Http));
        assert!(matches!(auth_type, config::McpAuthType::ApiKey));
    }
}
