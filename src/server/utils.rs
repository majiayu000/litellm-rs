//! HTTP server utility methods
//!
//! This module provides utility methods for the HttpServer.

use crate::server::HttpServer;
use crate::utils::error::gateway_error::GatewayError;
use tracing::{info, warn};

impl HttpServer {
    /// Graceful shutdown signal handler
    ///
    /// Public API for external callers to use for graceful shutdown handling.
    pub async fn shutdown_signal() {
        let ctrl_c = async {
            match tokio::signal::ctrl_c().await {
                Ok(()) => info!("Received Ctrl+C signal, shutting down gracefully"),
                Err(e) => warn!("Failed to install Ctrl+C handler: {}", e),
            }
        };

        #[cfg(unix)]
        let terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                    info!("Received terminate signal, shutting down gracefully");
                }
                Err(e) => {
                    warn!("Failed to install SIGTERM handler: {}", e);
                    // Wait indefinitely if signal handler fails
                    std::future::pending::<()>().await;
                }
            }
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }

    /// Format a user-friendly error message for port binding failures
    pub(crate) fn format_bind_error(
        error: std::io::Error,
        bind_addr: &str,
        port: u16,
    ) -> GatewayError {
        let error_str = error.to_string();

        // Check if it's an "address already in use" error
        if error_str.contains("Address already in use")
            || error_str.contains("os error 48")
            || error_str.contains("os error 98")
        {
            let message = format!(
                r#"
┌─────────────────────────────────────────────────────────────────┐
│  ❌ Error: Port {} is already in use
├─────────────────────────────────────────────────────────────────┤
│  Possible solutions:
│
│  1. Kill the existing process:
│     lsof -ti:{} | xargs kill -9
│
│  2. Use a different port:
│     --port {} or PORT={}
│
│  3. Check what's using it:
│     lsof -i:{}
└─────────────────────────────────────────────────────────────────┘
"#,
                port,
                port,
                port + 1,
                port + 1,
                port
            );
            GatewayError::server(message)
        } else if error_str.contains("Permission denied") || error_str.contains("os error 13") {
            let message = format!(
                r#"
┌─────────────────────────────────────────────────────────────────┐
│  ❌ Error: Permission denied for port {}
├─────────────────────────────────────────────────────────────────┤
│  Possible solutions:
│
│  1. Use a port >= 1024 (non-privileged):
│     --port 8000 or PORT=8000
│
│  2. Run with elevated privileges (not recommended):
│     sudo ./gateway
└─────────────────────────────────────────────────────────────────┘
"#,
                port
            );
            GatewayError::server(message)
        } else {
            GatewayError::server(format!("Failed to bind to {}: {}", bind_addr, error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    // ==================== format_bind_error Tests ====================

    #[test]
    fn test_format_bind_error_address_in_use() {
        let error = Error::new(ErrorKind::AddrInUse, "Address already in use");
        let result = HttpServer::format_bind_error(error, "0.0.0.0:8080", 8080);

        let error_msg = result.to_string();
        assert!(error_msg.contains("8080"));
        assert!(error_msg.contains("already in use"));
        assert!(error_msg.contains("8081")); // suggested alternative port
    }

    #[test]
    fn test_format_bind_error_os_error_48() {
        let error = Error::other("os error 48");
        let result = HttpServer::format_bind_error(error, "0.0.0.0:3000", 3000);

        let error_msg = result.to_string();
        assert!(error_msg.contains("3000"));
        assert!(error_msg.contains("3001")); // suggested alternative
    }

    #[test]
    fn test_format_bind_error_os_error_98() {
        let error = Error::other("os error 98");
        let result = HttpServer::format_bind_error(error, "127.0.0.1:9000", 9000);

        let error_msg = result.to_string();
        assert!(error_msg.contains("9000"));
    }

    #[test]
    fn test_format_bind_error_permission_denied() {
        let error = Error::new(ErrorKind::PermissionDenied, "Permission denied");
        let result = HttpServer::format_bind_error(error, "0.0.0.0:80", 80);

        let error_msg = result.to_string();
        assert!(error_msg.contains("80"));
        assert!(error_msg.contains("Permission denied"));
        assert!(error_msg.contains("1024")); // mentions non-privileged ports
    }

    #[test]
    fn test_format_bind_error_os_error_13() {
        let error = Error::other("os error 13");
        let result = HttpServer::format_bind_error(error, "0.0.0.0:443", 443);

        let error_msg = result.to_string();
        assert!(error_msg.contains("443"));
        assert!(error_msg.contains("Permission denied"));
    }

    #[test]
    fn test_format_bind_error_generic() {
        let error = Error::other("Network unreachable");
        let result = HttpServer::format_bind_error(error, "192.168.1.1:8080", 8080);

        let error_msg = result.to_string();
        assert!(error_msg.contains("Failed to bind"));
        assert!(error_msg.contains("192.168.1.1:8080"));
        assert!(error_msg.contains("Network unreachable"));
    }

    #[test]
    fn test_format_bind_error_suggested_port_8000() {
        let error = Error::new(ErrorKind::AddrInUse, "Address already in use");
        let result = HttpServer::format_bind_error(error, "0.0.0.0:8000", 8000);
        assert!(result.to_string().contains("8001"));
    }

    #[test]
    fn test_format_bind_error_suggested_port_3000() {
        let error = Error::new(ErrorKind::AddrInUse, "Address already in use");
        let result = HttpServer::format_bind_error(error, "0.0.0.0:3000", 3000);
        assert!(result.to_string().contains("3001"));
    }

    #[test]
    fn test_format_bind_error_contains_emoji() {
        let error = Error::new(ErrorKind::AddrInUse, "Address already in use");
        let result = HttpServer::format_bind_error(error, "0.0.0.0:8080", 8080);

        let error_msg = result.to_string();
        assert!(error_msg.contains("❌")); // Error emoji
    }

    #[test]
    fn test_format_bind_error_contains_lsof_command() {
        let error = Error::new(ErrorKind::AddrInUse, "Address already in use");
        let result = HttpServer::format_bind_error(error, "0.0.0.0:8080", 8080);

        let error_msg = result.to_string();
        assert!(error_msg.contains("lsof")); // Lists useful command
    }

    #[test]
    fn test_format_bind_error_privileged_port() {
        let error = Error::new(ErrorKind::PermissionDenied, "Permission denied");
        let result = HttpServer::format_bind_error(error, "0.0.0.0:22", 22);

        let error_msg = result.to_string();
        assert!(error_msg.contains("22"));
        assert!(error_msg.contains("8000")); // suggests standard dev port
    }

    #[test]
    fn test_format_bind_error_connection_refused() {
        let error = Error::new(ErrorKind::ConnectionRefused, "Connection refused");
        let result = HttpServer::format_bind_error(error, "localhost:8080", 8080);

        let error_msg = result.to_string();
        // Generic error should contain the original message
        assert!(error_msg.contains("Connection refused"));
    }

    #[test]
    fn test_format_bind_error_invalid_input() {
        let error = Error::new(ErrorKind::InvalidInput, "Invalid address format");
        let result = HttpServer::format_bind_error(error, "invalid:addr", 0);

        let error_msg = result.to_string();
        assert!(error_msg.contains("Failed to bind"));
    }

    // ==================== Error Type Tests ====================

    #[test]
    fn test_format_bind_error_returns_gateway_error() {
        let error = Error::new(ErrorKind::AddrInUse, "Address already in use");
        let result = HttpServer::format_bind_error(error, "0.0.0.0:8080", 8080);

        // Should return a formatted error message string
        let error_str = result.to_string();
        assert!(!error_str.is_empty());
        // The error message should contain port info
        assert!(error_str.contains("8080"));
    }
}
