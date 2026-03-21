use super::*;

// All ProviderError variants now wrap into GatewayError::Provider.
// The ResponseError impl (response.rs) handles HTTP status mapping.

#[test]
fn test_provider_error_wraps_in_provider_variant() {
    let provider_err = ProviderError::Authentication {
        provider: "openai",
        message: "Invalid API key".to_string(),
    };
    let gateway_err: GatewayError = provider_err.into();
    assert!(matches!(gateway_err, GatewayError::Provider(_)));
}

#[test]
fn test_provider_error_preserves_inner() {
    let provider_err = ProviderError::RateLimit {
        provider: "anthropic",
        message: "Too many requests".to_string(),
        retry_after: Some(60),
        rpm_limit: None,
        tpm_limit: None,
        current_usage: None,
    };
    let gateway_err: GatewayError = provider_err.into();
    match &gateway_err {
        GatewayError::Provider(inner) => {
            assert!(inner.to_string().contains("Too many requests"));
        }
        _ => panic!("Expected Provider variant"),
    }
}

#[test]
fn test_all_provider_variants_become_provider() {
    let cases: Vec<ProviderError> = vec![
        ProviderError::Authentication {
            provider: "openai",
            message: "bad key".to_string(),
        },
        ProviderError::ModelNotFound {
            provider: "openai",
            model: "gpt-5".to_string(),
        },
        ProviderError::InvalidRequest {
            provider: "openai",
            message: "bad".to_string(),
        },
        ProviderError::Network {
            provider: "openai",
            message: "refused".to_string(),
        },
        ProviderError::Timeout {
            provider: "openai",
            message: "timed out".to_string(),
        },
        ProviderError::Configuration {
            provider: "azure",
            message: "missing key".to_string(),
        },
        ProviderError::Other {
            provider: "unknown",
            message: "unknown".to_string(),
        },
    ];
    for err in cases {
        let gateway_err: GatewayError = err.into();
        assert!(
            matches!(gateway_err, GatewayError::Provider(_)),
            "Expected Provider variant, got: {:?}",
            gateway_err,
        );
    }
}

// ==================== A2A Error Conversion Tests ====================

#[test]
fn test_a2a_agent_not_found_conversion() {
    let a2a_err = A2AError::AgentNotFound {
        agent_name: "my-agent".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::NotFound(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("my-agent"));
        }
        _ => panic!("Expected NotFound error"),
    }
}

#[test]
fn test_a2a_agent_already_exists_conversion() {
    let a2a_err = A2AError::AgentAlreadyExists {
        agent_name: "existing-agent".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::Conflict(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("existing-agent"));
        }
        _ => panic!("Expected Conflict error"),
    }
}

#[test]
fn test_a2a_connection_error_conversion() {
    let a2a_err = A2AError::ConnectionError {
        agent_name: "remote-agent".to_string(),
        message: "Connection refused".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::Network(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("remote-agent"));
            assert!(msg.contains("Connection refused"));
        }
        _ => panic!("Expected Network error"),
    }
}

#[test]
fn test_a2a_authentication_error_conversion() {
    let a2a_err = A2AError::AuthenticationError {
        agent_name: "secure-agent".to_string(),
        message: "Invalid token".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::Auth(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("secure-agent"));
        }
        _ => panic!("Expected Auth error"),
    }
}

#[test]
fn test_a2a_task_not_found_conversion() {
    let a2a_err = A2AError::TaskNotFound {
        agent_name: "agent".to_string(),
        task_id: "task-456".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::NotFound(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("task-456"));
        }
        _ => panic!("Expected NotFound error"),
    }
}

#[test]
fn test_a2a_task_failed_conversion() {
    let a2a_err = A2AError::TaskFailed {
        agent_name: "agent".to_string(),
        task_id: "task-123".to_string(),
        message: "Something went wrong".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::Internal(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("task-123"));
            assert!(msg.contains("Something went wrong"));
        }
        _ => panic!("Expected Internal error"),
    }
}

#[test]
fn test_a2a_protocol_error_conversion() {
    let a2a_err = A2AError::ProtocolError {
        message: "Invalid JSON-RPC".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::BadRequest(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("protocol error"));
        }
        _ => panic!("Expected BadRequest error"),
    }
}

#[test]
fn test_a2a_timeout_conversion() {
    let a2a_err = A2AError::Timeout {
        agent_name: "slow-agent".to_string(),
        timeout_ms: 30000,
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::Timeout(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("slow-agent"));
            assert!(msg.contains("30000"));
        }
        _ => panic!("Expected Timeout error"),
    }
}

#[test]
fn test_a2a_configuration_error_conversion() {
    let a2a_err = A2AError::ConfigurationError {
        message: "Missing endpoint".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    assert!(matches!(gateway_err, GatewayError::Config(_)));
}

#[test]
fn test_a2a_serialization_error_conversion() {
    let a2a_err = A2AError::SerializationError {
        message: "Invalid UTF-8".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::Validation(msg) => assert!(msg.contains("A2A")),
        _ => panic!("Expected Parsing error"),
    }
}

#[test]
fn test_a2a_unsupported_provider_conversion() {
    let a2a_err = A2AError::UnsupportedProvider {
        provider: "unknown-provider".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::NotImplemented(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("unknown-provider"));
        }
        _ => panic!("Expected NotImplemented error"),
    }
}

#[test]
fn test_a2a_rate_limit_with_retry_conversion() {
    let a2a_err = A2AError::RateLimitExceeded {
        agent_name: "agent".to_string(),
        retry_after_ms: Some(5000),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::RateLimit {
            message: msg,
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        } => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("5000ms"));
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[test]
fn test_a2a_conversion_keeps_legacy_message_shape() {
    let a2a_err = A2AError::RateLimitExceeded {
        agent_name: "agent".to_string(),
        retry_after_ms: Some(1200),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::RateLimit {
            message: msg,
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        } => {
            assert!(msg.contains("A2A rate limit exceeded"));
            assert!(!msg.contains("protocol_code="));
            assert!(!msg.contains("canonical_code="));
            assert!(!msg.contains("retryable="));
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[test]
fn test_a2a_rate_limit_without_retry_conversion() {
    let a2a_err = A2AError::RateLimitExceeded {
        agent_name: "agent".to_string(),
        retry_after_ms: None,
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::RateLimit {
            message: msg,
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        } => {
            assert!(msg.contains("A2A"));
            assert!(!msg.contains("retry after"));
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[test]
fn test_a2a_agent_busy_conversion() {
    let a2a_err = A2AError::AgentBusy {
        agent_name: "busy-agent".to_string(),
        message: "Processing another request".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::Unavailable(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("busy-agent"));
        }
        _ => panic!("Expected ProviderUnavailable error"),
    }
}

#[test]
fn test_a2a_content_blocked_conversion() {
    let a2a_err = A2AError::ContentBlocked {
        agent_name: "safe-agent".to_string(),
        reason: "Harmful content".to_string(),
    };
    let gateway_err: GatewayError = a2a_err.into();
    match gateway_err {
        GatewayError::BadRequest(msg) => {
            assert!(msg.contains("A2A"));
            assert!(msg.contains("blocked"));
        }
        _ => panic!("Expected BadRequest error"),
    }
}

// ==================== MCP Error Conversion Tests ====================

#[test]
fn test_mcp_server_not_found_conversion() {
    let mcp_err = McpError::ServerNotFound {
        server_name: "github".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::NotFound(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("github"));
        }
        _ => panic!("Expected NotFound error"),
    }
}

#[test]
fn test_mcp_tool_not_found_conversion() {
    let mcp_err = McpError::ToolNotFound {
        server_name: "github".to_string(),
        tool_name: "get_repo".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::NotFound(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("get_repo"));
            assert!(msg.contains("github"));
        }
        _ => panic!("Expected NotFound error"),
    }
}

#[test]
fn test_mcp_connection_error_conversion() {
    let mcp_err = McpError::ConnectionError {
        server_name: "github".to_string(),
        message: "Connection refused".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Network(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("github"));
        }
        _ => panic!("Expected Network error"),
    }
}

#[test]
fn test_mcp_transport_error_conversion() {
    let mcp_err = McpError::TransportError {
        transport: "http".to_string(),
        message: "Connection reset".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Network(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("http"));
        }
        _ => panic!("Expected Network error"),
    }
}

#[test]
fn test_mcp_authentication_error_conversion() {
    let mcp_err = McpError::AuthenticationError {
        server_name: "github".to_string(),
        message: "Invalid token".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Auth(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("github"));
        }
        _ => panic!("Expected Auth error"),
    }
}

#[test]
fn test_mcp_authorization_error_with_tool_conversion() {
    let mcp_err = McpError::AuthorizationError {
        server_name: "github".to_string(),
        tool_name: Some("delete_repo".to_string()),
        message: "Admin required".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Forbidden(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("delete_repo"));
        }
        _ => panic!("Expected Forbidden error"),
    }
}

#[test]
fn test_mcp_authorization_error_without_tool_conversion() {
    let mcp_err = McpError::AuthorizationError {
        server_name: "github".to_string(),
        tool_name: None,
        message: "Access denied".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Forbidden(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("github"));
        }
        _ => panic!("Expected Forbidden error"),
    }
}

#[test]
fn test_mcp_protocol_error_conversion() {
    let mcp_err = McpError::ProtocolError {
        message: "Invalid JSON-RPC".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::BadRequest(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("protocol error"));
        }
        _ => panic!("Expected BadRequest error"),
    }
}

#[test]
fn test_mcp_tool_execution_error_conversion() {
    let mcp_err = McpError::ToolExecutionError {
        server_name: "github".to_string(),
        tool_name: "create_issue".to_string(),
        code: -32000,
        message: "Repository not found".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Internal(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("create_issue"));
            assert!(msg.contains("-32000"));
        }
        _ => panic!("Expected Internal error"),
    }
}

#[test]
fn test_mcp_timeout_conversion() {
    let mcp_err = McpError::Timeout {
        server_name: "slow-server".to_string(),
        timeout_ms: 30000,
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Timeout(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("slow-server"));
            assert!(msg.contains("30000"));
        }
        _ => panic!("Expected Timeout error"),
    }
}

#[test]
fn test_mcp_configuration_error_conversion() {
    let mcp_err = McpError::ConfigurationError {
        message: "Missing URL".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    assert!(matches!(gateway_err, GatewayError::Config(_)));
}

#[test]
fn test_mcp_serialization_error_conversion() {
    let mcp_err = McpError::SerializationError {
        message: "Invalid JSON".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Validation(msg) => assert!(msg.contains("MCP")),
        _ => panic!("Expected Parsing error"),
    }
}

#[test]
fn test_mcp_server_already_exists_conversion() {
    let mcp_err = McpError::ServerAlreadyExists {
        server_name: "github".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Conflict(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("github"));
        }
        _ => panic!("Expected Conflict error"),
    }
}

#[test]
fn test_mcp_invalid_url_conversion() {
    let mcp_err = McpError::InvalidUrl {
        url: "not-a-url".to_string(),
        message: "Invalid format".to_string(),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::BadRequest(msg) => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("not-a-url"));
        }
        _ => panic!("Expected BadRequest error"),
    }
}

#[test]
fn test_mcp_rate_limit_with_retry_conversion() {
    let mcp_err = McpError::RateLimitExceeded {
        server_name: "github".to_string(),
        retry_after_ms: Some(5000),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::RateLimit {
            message: msg,
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        } => {
            assert!(msg.contains("MCP"));
            assert!(msg.contains("5000ms"));
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[test]
fn test_mcp_conversion_keeps_legacy_message_shape() {
    let mcp_err = McpError::RateLimitExceeded {
        server_name: "github".to_string(),
        retry_after_ms: Some(800),
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::RateLimit {
            message: msg,
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        } => {
            assert!(msg.contains("MCP rate limit exceeded"));
            assert!(!msg.contains("protocol_code="));
            assert!(!msg.contains("canonical_code="));
            assert!(!msg.contains("retryable="));
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[test]
fn test_mcp_rate_limit_without_retry_conversion() {
    let mcp_err = McpError::RateLimitExceeded {
        server_name: "github".to_string(),
        retry_after_ms: None,
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::RateLimit {
            message: msg,
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        } => {
            assert!(msg.contains("MCP"));
            assert!(!msg.contains("retry after"));
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[test]
fn test_mcp_validation_error_conversion() {
    let mcp_err = McpError::ValidationError {
        server_name: "github".to_string(),
        tool_name: "create_pr".to_string(),
        errors: vec!["title is required".to_string(), "body too long".to_string()],
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Validation(msg) => {
            assert!(msg.contains("create_pr"));
            assert!(msg.contains("github"));
            assert!(msg.contains("title is required"));
            assert!(msg.contains("body too long"));
        }
        _ => panic!("Expected Validation error"),
    }
}

#[test]
fn test_mcp_validation_error_joins_errors_with_semicolons() {
    let mcp_err = McpError::ValidationError {
        server_name: "srv".to_string(),
        tool_name: "tool".to_string(),
        errors: vec!["err1".to_string(), "err2".to_string(), "err3".to_string()],
    };
    let gateway_err: GatewayError = mcp_err.into();
    match gateway_err {
        GatewayError::Validation(msg) => {
            assert!(msg.contains("err1"));
            assert!(msg.contains("err2"));
            assert!(msg.contains("err3"));
            assert!(
                msg.contains("err1; err2; err3"),
                "errors must be joined with semicolons, got: {msg}"
            );
        }
        _ => panic!("Expected Validation error"),
    }
}
