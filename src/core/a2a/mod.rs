//! A2A (Agent-to-Agent) Protocol Gateway
//!
//! This module implements A2A protocol support for litellm-rs, enabling
//! invocation and management of AI agents across multiple platforms.
//!
//! # Runtime entrypoints
//!
//! With `gateway,a2a`, configured A2A 1.0 JSON-RPC agents are exposed through
//! authenticated HTTP routes. The gateway supports messages, tasks and SSE with
//! caller-bound task/context ownership. See `docs/gateway/a2a.md` for limits.
//!
//! The library client below retains its older protocol contract. Its provider
//! enum is not evidence that every platform has a working native adapter.
//!
//! # Usage
//!
//! ```rust,no_run
//! # use litellm_rs::core::a2a::{A2AGateway, AgentConfig, AgentProvider};
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Configure an agent
//! let config = AgentConfig {
//!     name: "my-agent".to_string(),
//!     provider: AgentProvider::LangGraph,
//!     url: "https://my-agent.example.com".parse()?,
//!     ..Default::default()
//! };
//!
//! // Create gateway and register agent
//! let gateway = A2AGateway::new();
//! gateway.register_agent(config).await?;
//!
//! // Invoke the agent (send_message is an example - actual API may differ)
//! // let response = gateway.send_message("my-agent", message).await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod gateway;
pub mod message;
pub mod provider;
pub mod registry;

// Re-export commonly used types
#[deprecated(
    since = "0.6.0",
    note = "core::a2a is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use config::{AgentConfig, AgentProvider};
#[deprecated(
    since = "0.6.0",
    note = "core::a2a is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use error::{A2AError, A2AResult};
#[deprecated(
    since = "0.6.0",
    note = "core::a2a is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use gateway::A2AGateway;
#[deprecated(
    since = "0.6.0",
    note = "core::a2a is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use message::{A2AMessage, A2AResponse, MessagePart, TaskState};
#[deprecated(
    since = "0.6.0",
    note = "core::a2a is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use registry::AgentRegistry;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let provider = config::AgentProvider::LangGraph;
        let state = message::TaskState::Pending;
        assert!(matches!(provider, config::AgentProvider::LangGraph));
        assert!(matches!(state, message::TaskState::Pending));
    }
}
