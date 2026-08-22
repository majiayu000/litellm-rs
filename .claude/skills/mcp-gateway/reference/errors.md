## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Tool error: {0}")]
    ToolError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Server not found: {0}")]
    ServerNotFound(String),

    #[error("Request timeout")]
    Timeout,

    #[error("Empty response")]
    EmptyResponse,

    #[error("JSON-RPC error: {0}")]
    JsonRpc(String),
}
```
