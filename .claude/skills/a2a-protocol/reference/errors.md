## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum A2AProtocolError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Provider error: {provider} - {message}")]
    ProviderError {
        provider: AgentProvider,
        message: String,
    },

    #[error("Invalid state transition: {from:?} -> {to:?}")]
    InvalidTransition {
        from: TaskState,
        to: TaskState,
    },

    #[error("Task timeout: {0}")]
    Timeout(String),

    #[error("Rate limited: retry after {retry_after} seconds")]
    RateLimited { retry_after: u64 },

    #[error("Capability not found: {0}")]
    CapabilityNotFound(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Network error: {0}")]
    Network(String),
}
```

---
