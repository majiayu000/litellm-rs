---
name: mcp-gateway
description: LiteLLM-RS MCP Gateway Architecture. Covers Model Context Protocol, JSON-RPC 2.0 implementation, multi-transport support (HTTP, SSE, WebSocket, Stdio), and permission system. Use when working on MCP server connections, transports, tool invocation, or permissions.
---

# MCP Gateway Architecture Guide

## Overview

The MCP (Model Context Protocol) Gateway enables LLMs to interact with external tools and services through a standardized protocol. LiteLLM-RS implements MCP with multiple transports and fine-grained permission control.

### MCP Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        LLM Provider                             │
│  (OpenAI, Anthropic, etc. with tool/function calling)          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ Tool calls
┌─────────────────────────────────────────────────────────────────┐
│                      MCP Gateway                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  Tool Registry                           │   │
│  │  - Aggregates tools from all connected servers          │   │
│  │  - Handles routing and permission checks                 │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
         │              │              │              │
         ▼              ▼              ▼              ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│  MCP Server  │ │  MCP Server  │ │  MCP Server  │ │  MCP Server  │
│    (HTTP)    │ │    (SSE)     │ │ (WebSocket)  │ │   (Stdio)    │
│  Database    │ │  File System │ │    Slack     │ │   Local CLI  │
└──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘
```

---
## Configuration

```yaml
mcp:
  enabled: true

  servers:
    - name: "database"
      transport:
        type: "http"
        url: "http://localhost:3001/mcp"
        auth:
          type: "bearer"
          token: ${MCP_DATABASE_TOKEN}

    - name: "filesystem"
      transport:
        type: "stdio"
        command: "npx"
        args: ["-y", "@modelcontextprotocol/server-filesystem", "/data"]

    - name: "slack"
      transport:
        type: "sse"
        url: "http://localhost:3002/events"
        auth:
          type: "api_key"
          header: "X-API-Key"
          key: ${SLACK_MCP_KEY}

  permissions:
    default_policy: "deny"
    rules:
      - users: ["admin*"]
        tools: ["*"]
        servers: ["*"]
        policy: "allow"

      - users: ["*"]
        tools: ["query_*", "read_*"]
        servers: ["database", "filesystem"]
        policy: "allow"

      - users: ["*"]
        tools: ["write_*", "delete_*"]
        servers: ["*"]
        policy: "deny"
```

---

## References

- [reference/protocol.md](reference/protocol.md) — JSON-RPC 2.0 request/response types, IDs, standard error codes, and helper constructors
- [reference/transports.md](reference/transports.md) — McpTransport trait plus HTTP, SSE, WebSocket, and Stdio implementations
- [reference/tool-registry.md](reference/tool-registry.md) — ToolDefinition types and the concurrent, permission-aware ToolRegistry
- [reference/permissions.md](reference/permissions.md) — PermissionConfig rules and the PermissionManager pattern-matching engine
- [reference/gateway.md](reference/gateway.md) — McpGateway server connection lifecycle and tool aggregation
- [reference/errors.md](reference/errors.md) — The McpError error enum variants
