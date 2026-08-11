# MCP Gateway

> Deprecated 0.6 library-only surface scheduled for 0.7 removal. Compile with Cargo feature `mcp`. The
> gateway binary does not construct this module or mount an MCP HTTP route;
> enabling the feature alone does not make MCP a runtime capability.

Model Context Protocol (MCP) Gateway enables LiteLLM-RS to connect with external tools and services, allowing LLMs to interact with databases, APIs, file systems, and more.

## Overview

The MCP Gateway implements the [Model Context Protocol](https://modelcontextprotocol.io/) specification, providing:

- **Tool Discovery**: Automatic discovery of tools from MCP servers
- **Tool Invocation**: Execute tools with proper argument validation
- **Multi-Transport**: HTTP, SSE, WebSocket, and stdio support
- **Authentication**: Bearer tokens, API keys, Basic auth, and OAuth 2.0
- **Permission Control**: Fine-grained access control per API key, team, or organization

## Quick Start

```rust
use litellm_rs::core::mcp::{McpGateway, McpServerConfig, AuthConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create gateway
    let gateway = McpGateway::new();

    // Register an MCP server
    let config = McpServerConfig::new("filesystem", "https://mcp.example.com/fs")
        .with_auth(AuthConfig::bearer("your-token"))
        .with_timeout(30000);

    gateway.register_server(config).await?;

    // Discover tools
    let tools = gateway.list_all_tools().await?;
    println!("Available tools: {:?}", tools);

    // Invoke a tool
    let result = gateway.call_tool(
        "filesystem",
        "read_file",
        serde_json::json!({"path": "/etc/hostname"})
    ).await?;

    println!("Result: {:?}", result);
    Ok(())
}
```

## Configuration

### Server Configuration

```rust
use litellm_rs::core::mcp::{McpServerConfig, Transport, AuthConfig};

let config = McpServerConfig {
    name: "my-server".to_string(),
    url: "https://mcp.example.com".to_string(),
    transport: Transport::Http,
    enabled: true,
    timeout_ms: 30000,
    auth: Some(AuthConfig::bearer("token")),
    static_headers: HashMap::new(),
};
```

### Authentication Types

```rust
// Bearer Token
AuthConfig::bearer("your-jwt-token")

// API Key
AuthConfig::api_key("your-api-key")

// Basic Auth
AuthConfig::basic("username", "password")

// OAuth 2.0
AuthConfig::oauth2("client_id", "client_secret", "https://auth.example.com/token")
```

### Transport Types

| Transport | Description | Use Case |
|-----------|-------------|----------|
| `Http` | Standard HTTP POST | Most common, request/response |
| `Sse` | Server-Sent Events | Streaming responses |
| `WebSocket` | Full duplex | Real-time bidirectional |
| `Stdio` | Standard I/O | Local process communication |

## Permission System

The MCP Gateway includes a comprehensive permission system for controlling tool access.

### Permission Levels

```rust
use litellm_rs::core::mcp::permissions::{PermissionLevel, PermissionManager};

// Hierarchy: Admin > Execute > Read > Deny
PermissionLevel::Admin    // Full access including configuration
PermissionLevel::Execute  // Can invoke tools
PermissionLevel::Read     // Can list tools but not invoke
PermissionLevel::Deny     // No access
```

### Permission Rules

```rust
use litellm_rs::core::mcp::permissions::{PermissionPolicy, PermissionRule};

let policy = PermissionPolicy {
    name: "production".to_string(),
    default_level: PermissionLevel::Deny,
    rules: vec![
        // Allow specific tool
        PermissionRule::new("filesystem", "read_file", PermissionLevel::Execute),
        // Allow all tools from a server
        PermissionRule::new("database", "*", PermissionLevel::Execute),
        // Deny dangerous operations
        PermissionRule::new("*", "delete_*", PermissionLevel::Deny),
    ],
};
```

### Per-Key Permissions

```rust
let mut manager = PermissionManager::new();
manager.set_default_level(PermissionLevel::Read);

// Set policy for specific API key
manager.set_key_policy("sk-prod-123", production_policy);
manager.set_key_policy("sk-dev-456", development_policy);

// Check access
let can_execute = manager.check_tool_access(
    Some("sk-prod-123"),
    "filesystem",
    "read_file"
);
```

## Tool Integration

### Tool Definition

Tools are automatically discovered from MCP servers, but you can also define custom tools:

```rust
use litellm_rs::core::mcp::tools::{Tool, ToolInputSchema, PropertySchema};

let tool = Tool::new("get_weather", "Get current weather for a location")
    .with_input_schema(
        ToolInputSchema::new()
            .with_property("city", PropertySchema::string("City name"))
            .with_property("units", PropertySchema::string("Temperature units (celsius/fahrenheit)"))
            .with_required(vec!["city".to_string()])
    );
```

### OpenAI Function Calling Integration

MCP tools can be converted to OpenAI function format:

```rust
let tools = gateway.list_all_tools().await?;

// Convert to OpenAI functions
let functions: Vec<serde_json::Value> = tools
    .iter()
    .map(|t| t.to_openai_function())
    .collect();
```

## Server Registry

### Managing Multiple Servers

```rust
let gateway = McpGateway::new();

// Register multiple servers
gateway.register_server(filesystem_config).await?;
gateway.register_server(database_config).await?;
gateway.register_server(api_config).await?;

// List all servers
let servers = gateway.list_servers().await;

// Get specific server
let server = gateway.get_server("filesystem").await?;

// Unregister
gateway.unregister_server("filesystem").await;
```

### Server Aliases

```rust
// Add alias for convenience
gateway.add_alias("fs", "filesystem").await?;

// Use alias to call tools
gateway.call_tool("fs", "read_file", args).await?;
```

## Error Handling

```rust
use litellm_rs::core::mcp::error::McpError;

match gateway.call_tool("server", "tool", args).await {
    Ok(result) => println!("Success: {:?}", result),
    Err(McpError::ServerNotFound { server_name }) => {
        println!("Server {} not found", server_name);
    }
    Err(McpError::ToolNotFound { server_name, tool_name }) => {
        println!("Tool {} not found on {}", tool_name, server_name);
    }
    Err(McpError::AuthenticationError { server_name, message }) => {
        println!("Auth failed for {}: {}", server_name, message);
    }
    Err(McpError::ToolExecutionError { tool_name, message, .. }) => {
        println!("Tool {} failed: {}", tool_name, message);
    }
    Err(e) => println!("Other error: {}", e),
}
```

## Architecture

```
src/core/mcp/
├── mod.rs          # Module entry point and re-exports
├── config.rs       # Server configuration and auth types
├── error.rs        # Error types (McpError, McpResult)
├── transport.rs    # Transport layer (HTTP/SSE/WebSocket/stdio)
├── protocol.rs     # JSON-RPC 2.0 protocol implementation
├── tools.rs        # Tool definitions and invocation
├── server.rs       # Individual server connection management
├── gateway.rs      # Main gateway aggregating all servers
└── permissions.rs  # Permission control system
```

## Best Practices

1. **Use Connection Pooling**: The gateway uses shared HTTP clients for optimal performance
2. **Set Appropriate Timeouts**: Configure timeout based on expected tool execution time
3. **Implement Permission Policies**: Always use the permission system in production
4. **Handle Errors Gracefully**: MCP servers may be unavailable; implement retry logic
5. **Cache Tool Lists**: Tool discovery can be expensive; the gateway caches automatically

## References

- [Model Context Protocol Specification](https://modelcontextprotocol.io/)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [LiteLLM Python MCP Documentation](https://docs.litellm.ai/docs/mcp)
