## MCP Gateway

### Gateway Implementation

```rust
pub struct McpGateway {
    servers: DashMap<String, Arc<McpServer>>,
    tool_registry: Arc<ToolRegistry>,
    config: McpGatewayConfig,
}

impl McpGateway {
    pub fn new(config: McpGatewayConfig) -> Self {
        let permissions = Arc::new(PermissionManager::new(config.permissions.clone()));
        let tool_registry = Arc::new(ToolRegistry::new(permissions));

        Self {
            servers: DashMap::new(),
            tool_registry,
            config,
        }
    }

    pub async fn connect_server(&self, server_config: &McpServerConfig) -> Result<(), McpError> {
        let transport: Arc<dyn McpTransport> = match &server_config.transport {
            TransportConfig::Http { url, auth } => {
                Arc::new(HttpTransport::new(url, auth.clone()))
            }
            TransportConfig::Sse { url, auth } => {
                Arc::new(SseTransport::connect(url, auth.clone()).await?)
            }
            TransportConfig::WebSocket { url } => {
                Arc::new(WebSocketTransport::connect(url).await?)
            }
            TransportConfig::Stdio { command, args } => {
                Arc::new(StdioTransport::spawn(command, args).await?)
            }
        };

        // Initialize connection
        let init_request = JsonRpcRequest::initialize(&ClientInfo {
            name: "litellm-gateway".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        });
        transport.send_request(init_request).await?;

        // List tools
        let tools_request = JsonRpcRequest::list_tools();
        let tools_response = transport.send_request(tools_request).await?;

        let tools: Vec<ToolDefinition> = serde_json::from_value(
            tools_response.result.unwrap_or(serde_json::json!({ "tools": [] }))
        ).unwrap_or_default();

        // Register tools
        for tool in tools {
            self.tool_registry.register_tool(RegisteredTool {
                definition: tool,
                server_name: server_config.name.clone(),
                transport: transport.clone(),
            });
        }

        let server = McpServer {
            name: server_config.name.clone(),
            transport,
        };

        self.servers.insert(server_config.name.clone(), Arc::new(server));

        Ok(())
    }

    pub fn list_tools(&self, user_id: &str) -> Vec<ToolDefinition> {
        self.tool_registry.list_tools(user_id)
    }

    pub async fn call_tool(
        &self,
        user_id: &str,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        self.tool_registry.call_tool(user_id, name, arguments).await
    }
}
```

---
