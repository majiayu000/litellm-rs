## Tool Registry

### Tool Definition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct RegisteredTool {
    pub definition: ToolDefinition,
    pub server_name: String,
    pub transport: Arc<dyn McpTransport>,
}
```

### Tool Registry Implementation

```rust
use dashmap::DashMap;

pub struct ToolRegistry {
    tools: DashMap<String, RegisteredTool>,
    permissions: Arc<PermissionManager>,
}

impl ToolRegistry {
    pub fn new(permissions: Arc<PermissionManager>) -> Self {
        Self {
            tools: DashMap::new(),
            permissions,
        }
    }

    pub fn register_tool(&self, tool: RegisteredTool) {
        self.tools.insert(tool.definition.name.clone(), tool);
    }

    pub fn unregister_tools(&self, server_name: &str) {
        self.tools.retain(|_, tool| tool.server_name != server_name);
    }

    pub fn get_tool(&self, name: &str) -> Option<RegisteredTool> {
        self.tools.get(name).map(|t| t.clone())
    }

    pub fn list_tools(&self, user_id: &str) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .filter(|entry| {
                self.permissions.can_use_tool(user_id, &entry.definition.name)
            })
            .map(|entry| entry.definition.clone())
            .collect()
    }

    pub async fn call_tool(
        &self,
        user_id: &str,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        // Check permissions
        if !self.permissions.can_use_tool(user_id, name) {
            return Err(McpError::PermissionDenied(format!(
                "User {} cannot use tool {}",
                user_id, name
            )));
        }

        // Get tool
        let tool = self.tools.get(name)
            .ok_or_else(|| McpError::ToolNotFound(name.to_string()))?;

        // Call via transport
        let request = JsonRpcRequest::call_tool(name, arguments);
        let response = tool.transport.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(McpError::ToolError(error.message));
        }

        response.result.ok_or(McpError::EmptyResponse)
    }
}
```

---
