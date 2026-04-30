//! Server builder and run_server function
//!
//! This module provides the ServerBuilder for easier server configuration
//! and the run_server function for automatic configuration loading.

use crate::config::Config;
use crate::server::HttpServer;
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::path::Path;
use tracing::info;

/// Server builder for easier configuration
pub struct ServerBuilder {
    config: Option<Config>,
}

impl ServerBuilder {
    /// Create a new server builder
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Set configuration
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    /// Build the HTTP server
    pub async fn build(self) -> Result<HttpServer> {
        let config = self
            .config
            .ok_or_else(|| GatewayError::Config("Configuration is required".to_string()))?;

        HttpServer::new(&config).await
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the server with automatic configuration loading
pub async fn run_server() -> Result<()> {
    run_server_with_config_path("config/gateway.yaml").await
}

/// Run the server with an explicit configuration path.
pub async fn run_server_with_config_path<P>(config_path: P) -> Result<()>
where
    P: AsRef<Path>,
{
    run_server_with_config_overrides(config_path, None, None).await
}

/// Run the server with an explicit configuration path and CLI overrides.
pub async fn run_server_with_config_overrides<P>(
    config_path: P,
    host: Option<&str>,
    port: Option<u16>,
) -> Result<()>
where
    P: AsRef<Path>,
{
    info!("🚀 Starting Rust LiteLLM Gateway");

    // Auto-load configuration file
    let config_path = config_path.as_ref();
    info!("📄 Loading configuration file: {}", config_path.display());

    let mut config = match Config::from_file(config_path).await {
        Ok(config) => {
            info!("✅ Configuration file loaded successfully");
            config
        }
        Err(file_error) => {
            info!(
                "⚠️  Failed to load {}: {}. Trying environment variables.",
                config_path.display(),
                file_error
            );
            match Config::from_env() {
                Ok(config) => {
                    info!("✅ Loaded configuration from environment variables");
                    config
                }
                Err(env_error) => {
                    return Err(GatewayError::Config(format!(
                        "Failed to load configuration from file ({}) and environment ({}).",
                        file_error, env_error
                    )));
                }
            }
        }
    };

    if let Some(host) = host {
        config.gateway.server.host = host.to_string();
    }
    if let Some(port) = port {
        config.gateway.server.port = port;
    }

    // Ensure configuration is valid (including defaults)
    config.validate()?;

    // Create and start server
    let server = HttpServer::new(&config).await?;
    info!(
        "🌐 Server starting at: http://{}:{}",
        config.server().host,
        config.server().port
    );
    info!("📋 API Endpoints:");
    info!("   GET  /health - Health check");
    info!("   GET  /v1/models - Model list");
    info!("   POST /v1/chat/completions - Chat completions");
    info!("   POST /v1/embeddings - Text embeddings");

    server.start().await
}
