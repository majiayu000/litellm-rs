//! Type conversions for GatewayError

use super::types::GatewayError;
#[cfg(feature = "a2a")]
use crate::core::a2a::error::A2AError;
#[cfg(feature = "a2a")]
use crate::core::a2a::message::A2AResponseError;
#[cfg(feature = "mcp")]
use crate::core::mcp::error::McpError;
#[cfg(feature = "mcp")]
use crate::core::mcp::protocol::JsonRpcError;
use crate::core::providers::unified_provider::ProviderError;

// Conversion from unified ProviderError to GatewayError.
// The ResponseError impl in response.rs handles all ProviderError variants
// via GatewayError::Provider, so no destructuring is needed here.
impl From<ProviderError> for GatewayError {
    fn from(err: ProviderError) -> Self {
        GatewayError::Provider(err.redacted())
    }
}

#[cfg(any(feature = "a2a", feature = "mcp"))]
fn retry_after_ms_to_secs(retry_after_ms: Option<u64>) -> Option<u64> {
    retry_after_ms.map(|ms| (ms.saturating_add(999) / 1000).max(1))
}

// Conversion from A2AError to GatewayError
#[cfg(feature = "a2a")]
impl From<A2AError> for GatewayError {
    fn from(err: A2AError) -> Self {
        // Keep protocol mapping in the runtime path so canonical A2A mapping is exercised.
        // GatewayError message text remains unchanged for backward compatibility.
        let _protocol_error = A2AResponseError::from_a2a_error(&err);

        match err {
            A2AError::AgentNotFound { agent_name } => {
                GatewayError::NotFound(format!("A2A agent not found: {}", agent_name))
            }
            A2AError::AgentAlreadyExists { agent_name } => {
                GatewayError::Conflict(format!("A2A agent already exists: {}", agent_name))
            }
            A2AError::ConnectionError {
                agent_name,
                message,
            } => GatewayError::Network(format!(
                "A2A connection error to agent '{}': {}",
                agent_name, message
            )),
            A2AError::AuthenticationError {
                agent_name,
                message,
            } => GatewayError::Auth(format!(
                "A2A authentication failed for agent '{}': {}",
                agent_name, message
            )),
            A2AError::TaskNotFound {
                agent_name,
                task_id,
            } => GatewayError::NotFound(format!(
                "A2A task '{}' not found on agent '{}'",
                task_id, agent_name
            )),
            A2AError::TaskFailed {
                agent_name,
                task_id,
                message,
            } => GatewayError::Internal(format!(
                "A2A task '{}' failed on agent '{}': {}",
                task_id, agent_name, message
            )),
            A2AError::ProtocolError { message } => {
                GatewayError::BadRequest(format!("A2A protocol error: {}", message))
            }
            A2AError::InvalidRequest { message } => {
                GatewayError::BadRequest(format!("Invalid A2A request: {}", message))
            }
            A2AError::Timeout {
                agent_name,
                timeout_ms,
            } => GatewayError::Timeout(format!(
                "A2A timeout waiting for agent '{}' ({}ms)",
                agent_name, timeout_ms
            )),
            A2AError::ConfigurationError { message } => {
                GatewayError::Config(format!("A2A configuration error: {}", message))
            }
            A2AError::SerializationError { message } => {
                GatewayError::Validation(format!("A2A serialization error: {}", message))
            }
            A2AError::UnsupportedProvider { provider } => {
                GatewayError::NotImplemented(format!("A2A provider not supported: {}", provider))
            }
            A2AError::RateLimitExceeded {
                agent_name,
                retry_after_ms,
            } => {
                let msg = if let Some(ms) = retry_after_ms {
                    format!(
                        "A2A rate limit exceeded for agent '{}', retry after {}ms",
                        agent_name, ms
                    )
                } else {
                    format!("A2A rate limit exceeded for agent '{}'", agent_name)
                };
                GatewayError::RateLimit {
                    message: msg,
                    retry_after: retry_after_ms_to_secs(retry_after_ms),
                    rpm_limit: None,
                    tpm_limit: None,
                }
            }
            A2AError::AgentBusy {
                agent_name,
                message,
            } => GatewayError::Unavailable(format!(
                "A2A agent '{}' is busy: {}",
                agent_name, message
            )),
            A2AError::ContentBlocked { agent_name, reason } => GatewayError::BadRequest(format!(
                "A2A content blocked by agent '{}': {}",
                agent_name, reason
            )),
        }
    }
}

// Conversion from McpError to GatewayError
#[cfg(feature = "mcp")]
impl From<McpError> for GatewayError {
    fn from(err: McpError) -> Self {
        // Keep protocol mapping in the runtime path so canonical MCP mapping is exercised.
        // GatewayError message text remains unchanged for backward compatibility.
        let _protocol_error = JsonRpcError::from_mcp_error(&err);

        match err {
            McpError::ServerNotFound { server_name } => {
                GatewayError::NotFound(format!("MCP server not found: {}", server_name))
            }
            McpError::ToolNotFound {
                server_name,
                tool_name,
            } => GatewayError::NotFound(format!(
                "MCP tool '{}' not found on server '{}'",
                tool_name, server_name
            )),
            McpError::ConnectionError {
                server_name,
                message,
            } => GatewayError::Network(format!(
                "MCP connection error to server '{}': {}",
                server_name, message
            )),
            McpError::TransportError { transport, message } => {
                GatewayError::Network(format!("MCP transport error ({}): {}", transport, message))
            }
            McpError::AuthenticationError {
                server_name,
                message,
            } => GatewayError::Auth(format!(
                "MCP authentication failed for server '{}': {}",
                server_name, message
            )),
            McpError::AuthorizationError {
                server_name,
                tool_name,
                message,
            } => {
                let msg = if let Some(tool) = tool_name {
                    format!(
                        "MCP permission denied for tool '{}' on server '{}': {}",
                        tool, server_name, message
                    )
                } else {
                    format!(
                        "MCP permission denied for server '{}': {}",
                        server_name, message
                    )
                };
                GatewayError::Forbidden(msg)
            }
            McpError::ProtocolError { message } => {
                GatewayError::BadRequest(format!("MCP protocol error: {}", message))
            }
            McpError::ToolExecutionError {
                server_name,
                tool_name,
                code,
                message,
            } => GatewayError::Internal(format!(
                "MCP tool execution failed: server='{}', tool='{}', code={}, message='{}'",
                server_name, tool_name, code, message
            )),
            McpError::Timeout {
                server_name,
                timeout_ms,
            } => GatewayError::Timeout(format!(
                "MCP timeout waiting for server '{}' ({}ms)",
                server_name, timeout_ms
            )),
            McpError::ConfigurationError { message } => {
                GatewayError::Config(format!("MCP configuration error: {}", message))
            }
            McpError::SerializationError { message } => {
                GatewayError::Validation(format!("MCP serialization error: {}", message))
            }
            McpError::ServerAlreadyExists { server_name } => {
                GatewayError::Conflict(format!("MCP server already registered: {}", server_name))
            }
            McpError::ToolDefinitionChanged {
                server_name,
                expected_hash,
                actual_hash,
            } => GatewayError::Conflict(format!(
                "MCP tool definitions changed for server '{}': expected hash {}, got {}",
                server_name, expected_hash, actual_hash
            )),
            McpError::InvalidUrl { url, message } => {
                GatewayError::BadRequest(format!("Invalid MCP server URL '{}': {}", url, message))
            }
            McpError::RateLimitExceeded {
                server_name,
                retry_after_ms,
            } => {
                let msg = if let Some(ms) = retry_after_ms {
                    format!(
                        "MCP rate limit exceeded for server '{}', retry after {}ms",
                        server_name, ms
                    )
                } else {
                    format!("MCP rate limit exceeded for server '{}'", server_name)
                };
                GatewayError::RateLimit {
                    message: msg,
                    retry_after: retry_after_ms_to_secs(retry_after_ms),
                    rpm_limit: None,
                    tpm_limit: None,
                }
            }
            McpError::ValidationError {
                server_name,
                tool_name,
                errors,
            } => GatewayError::Validation(format!(
                "Validation failed for tool '{}' on server '{}': {}",
                tool_name,
                server_name,
                errors.join("; ")
            )),
        }
    }
}

#[cfg(all(test, feature = "a2a", feature = "mcp"))]
#[path = "conversions_tests.rs"]
mod protocol_tests;

#[cfg(test)]
#[path = "provider_conversion_tests.rs"]
mod provider_tests;
