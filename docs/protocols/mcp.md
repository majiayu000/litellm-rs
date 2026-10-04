# MCP gateway and shared types

The executable entry is the authenticated [MCP HTTP gateway](../gateway/mcp.md),
built with `gateway,mcp`. It forwards stateless MCP 2026-07-28 POST requests and
response SSE to one configured server. Its configuration, required protocol
headers, authentication and limits are documented in that guide.

The former `McpGateway`, standalone server/session client and permission manager
have been removed in 0.8.0. The old aggregation, stdio/WebSocket, OAuth acquisition
and per-tool permission examples no longer describe an executable API.

Configuration and protocol/schema helpers remain available at qualified paths:

```rust
use litellm_rs::core::mcp::config::{AuthConfig, McpServerConfig};
use litellm_rs::core::mcp::transport::Transport;

let config = McpServerConfig::new("docs", "https://mcp.example.com/mcp")
    .with_transport(Transport::Http)
    .with_timeout(30_000);
```

This constructs a configuration value, not a client. Supply it through
`GatewayConfig::mcp_servers` or the documented JSON/YAML gateway configuration.
Retained transport/auth enum variants do not establish runtime support; the HTTP
gateway rejects unsupported configuration. See the [subsystem reconciliation](../audit/subsystem-reconciliation-2026-10-04.md).
