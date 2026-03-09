//! Tests for server module
//!
//! This module contains all tests for the server components.

#[cfg(test)]
use crate::server::builder::ServerBuilder;
use crate::server::types::ServerRequestMetrics;
use crate::utils::error::gateway_error::GatewayError;

#[tokio::test]
async fn test_server_builder_requires_config() {
    let result = ServerBuilder::new().build().await;
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("builder without configuration should fail"),
    };

    match error {
        GatewayError::Config(message) => assert_eq!(message, "Configuration is required"),
        other => panic!("expected config error, got: {other:?}"),
    }
}

#[test]
fn test_request_metrics_creation() {
    let metrics = ServerRequestMetrics {
        request_id: "req-123".to_string(),
        method: "GET".to_string(),
        path: "/health".to_string(),
        status_code: 200,
        response_time_ms: 50,
        request_size: 0,
        response_size: 100,
        user_agent: Some("test-agent".to_string()),
        client_ip: Some("127.0.0.1".to_string()),
        user_id: None,
        api_key_id: None,
    };

    assert_eq!(metrics.request_id, "req-123");
    assert_eq!(metrics.method, "GET");
    assert_eq!(metrics.status_code, 200);
}
