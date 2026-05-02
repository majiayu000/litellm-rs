//! MCP Error types
//!
//! Defines error types for MCP operations.

use crate::{impl_from_reqwest_error, impl_from_serde_error};
use std::fmt;

/// Result type for MCP operations
pub type McpResult<T> = Result<T, McpError>;

/// MCP-specific errors
#[derive(Debug, Clone)]
pub enum McpError {
    /// Server not found
    ServerNotFound { server_name: String },

    /// Tool not found
    ToolNotFound {
        server_name: String,
        tool_name: String,
    },

    /// Connection error
    ConnectionError {
        server_name: String,
        message: String,
    },

    /// Transport error
    TransportError { transport: String, message: String },

    /// Authentication error
    AuthenticationError {
        server_name: String,
        message: String,
    },

    /// Authorization error (permission denied)
    AuthorizationError {
        server_name: String,
        tool_name: Option<String>,
        message: String,
    },

    /// Protocol error (invalid JSON-RPC message)
    ProtocolError { message: String },

    /// Tool execution error
    ToolExecutionError {
        server_name: String,
        tool_name: String,
        code: i32,
        message: String,
    },

    /// Timeout error
    Timeout {
        server_name: String,
        timeout_ms: u64,
    },

    /// Configuration error
    ConfigurationError { message: String },

    /// Serialization error
    SerializationError { message: String },

    /// Server already registered
    ServerAlreadyExists { server_name: String },

    /// Invalid URL
    InvalidUrl { url: String, message: String },

    /// Rate limit exceeded
    RateLimitExceeded {
        server_name: String,
        retry_after_ms: Option<u64>,
    },

    /// Validation error for tool arguments
    ValidationError {
        server_name: String,
        tool_name: String,
        errors: Vec<String>,
    },

    /// Tool definitions changed after the initial trust baseline
    ToolDefinitionChanged {
        server_name: String,
        expected_hash: String,
        actual_hash: String,
    },
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::ServerNotFound { server_name } => {
                write!(f, "MCP server not found: {}", server_name)
            }
            McpError::ToolNotFound {
                server_name,
                tool_name,
            } => {
                write!(
                    f,
                    "Tool '{}' not found on MCP server '{}'",
                    tool_name, server_name
                )
            }
            McpError::ConnectionError {
                server_name,
                message,
            } => {
                write!(
                    f,
                    "Connection error to MCP server '{}': {}",
                    server_name, message
                )
            }
            McpError::TransportError { transport, message } => {
                write!(f, "Transport error ({}): {}", transport, message)
            }
            McpError::AuthenticationError {
                server_name,
                message,
            } => {
                write!(
                    f,
                    "Authentication failed for MCP server '{}': {}",
                    server_name, message
                )
            }
            McpError::AuthorizationError {
                server_name,
                tool_name,
                message,
            } => {
                if let Some(tool) = tool_name {
                    write!(
                        f,
                        "Permission denied for tool '{}' on server '{}': {}",
                        tool, server_name, message
                    )
                } else {
                    write!(
                        f,
                        "Permission denied for MCP server '{}': {}",
                        server_name, message
                    )
                }
            }
            McpError::ProtocolError { message } => {
                write!(f, "MCP protocol error: {}", message)
            }
            McpError::ToolExecutionError {
                server_name,
                tool_name,
                code,
                message,
            } => {
                write!(
                    f,
                    "Tool execution failed: server='{}', tool='{}', code={}, message='{}'",
                    server_name, tool_name, code, message
                )
            }
            McpError::Timeout {
                server_name,
                timeout_ms,
            } => {
                write!(
                    f,
                    "Timeout waiting for MCP server '{}' ({}ms)",
                    server_name, timeout_ms
                )
            }
            McpError::ConfigurationError { message } => {
                write!(f, "MCP configuration error: {}", message)
            }
            McpError::SerializationError { message } => {
                write!(f, "MCP serialization error: {}", message)
            }
            McpError::ServerAlreadyExists { server_name } => {
                write!(f, "MCP server already registered: {}", server_name)
            }
            McpError::InvalidUrl { url, message } => {
                write!(f, "Invalid MCP server URL '{}': {}", url, message)
            }
            McpError::RateLimitExceeded {
                server_name,
                retry_after_ms,
            } => {
                if let Some(ms) = retry_after_ms {
                    write!(
                        f,
                        "Rate limit exceeded for MCP server '{}', retry after {}ms",
                        server_name, ms
                    )
                } else {
                    write!(f, "Rate limit exceeded for MCP server '{}'", server_name)
                }
            }
            McpError::ValidationError {
                server_name,
                tool_name,
                errors,
            } => {
                write!(
                    f,
                    "Validation failed for tool '{}' on server '{}': {}",
                    tool_name,
                    server_name,
                    errors.join("; ")
                )
            }
            McpError::ToolDefinitionChanged {
                server_name,
                expected_hash,
                actual_hash,
            } => {
                write!(
                    f,
                    "MCP tool definitions changed for server '{}': expected hash {}, got {}",
                    server_name, expected_hash, actual_hash
                )
            }
        }
    }
}

impl std::error::Error for McpError {}

impl_from_serde_error!(McpError, |e| McpError::SerializationError {
    message: e.to_string(),
});

impl_from_reqwest_error!(McpError,
    timeout => |_e| McpError::Timeout {
        server_name: "unknown".to_string(),
        timeout_ms: 0,
    },
    connect => |e| McpError::ConnectionError {
        server_name: "unknown".to_string(),
        message: e.to_string(),
    },
    other => |e| McpError::TransportError {
        transport: "http".to_string(),
        message: e.to_string(),
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_not_found_display() {
        let err = McpError::ServerNotFound {
            server_name: "github".to_string(),
        };
        assert!(err.to_string().contains("github"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_tool_not_found_display() {
        let err = McpError::ToolNotFound {
            server_name: "github".to_string(),
            tool_name: "get_repo".to_string(),
        };
        assert!(err.to_string().contains("github"));
        assert!(err.to_string().contains("get_repo"));
    }

    #[test]
    fn test_authentication_error_display() {
        let err = McpError::AuthenticationError {
            server_name: "github".to_string(),
            message: "invalid token".to_string(),
        };
        assert!(err.to_string().contains("Authentication"));
        assert!(err.to_string().contains("invalid token"));
    }

    #[test]
    fn test_authorization_error_with_tool() {
        let err = McpError::AuthorizationError {
            server_name: "github".to_string(),
            tool_name: Some("delete_repo".to_string()),
            message: "admin required".to_string(),
        };
        assert!(err.to_string().contains("delete_repo"));
        assert!(err.to_string().contains("Permission denied"));
    }

    #[test]
    fn test_authorization_error_without_tool() {
        let err = McpError::AuthorizationError {
            server_name: "github".to_string(),
            tool_name: None,
            message: "access denied".to_string(),
        };
        assert!(!err.to_string().contains("tool"));
        assert!(err.to_string().contains("Permission denied"));
    }

    #[test]
    fn test_tool_execution_error() {
        let err = McpError::ToolExecutionError {
            server_name: "github".to_string(),
            tool_name: "create_issue".to_string(),
            code: -32000,
            message: "Repository not found".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("github"));
        assert!(msg.contains("create_issue"));
        assert!(msg.contains("-32000"));
        assert!(msg.contains("Repository not found"));
    }

    #[test]
    fn test_rate_limit_with_retry() {
        let err = McpError::RateLimitExceeded {
            server_name: "github".to_string(),
            retry_after_ms: Some(5000),
        };
        assert!(err.to_string().contains("5000ms"));
    }

    #[test]
    fn test_rate_limit_without_retry() {
        let err = McpError::RateLimitExceeded {
            server_name: "github".to_string(),
            retry_after_ms: None,
        };
        assert!(!err.to_string().contains("retry after"));
    }

    #[test]
    fn test_error_is_error_trait() {
        let err: Box<dyn std::error::Error> = Box::new(McpError::ServerNotFound {
            server_name: "test".to_string(),
        });
        assert!(!err.to_string().is_empty());
    }
}
