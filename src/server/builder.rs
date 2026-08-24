//! Server builder and run_server function
//!
//! This module provides the ServerBuilder for easier server configuration
//! and the run_server function for automatic configuration loading.

use crate::config::Config;
use crate::server::HttpServer;
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::path::Path;
use tracing::info;

const DEFAULT_CONFIG_PATH: &str = "config/gateway.yaml";

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
    run_server_with_default_config_overrides(None, None).await
}

/// Run the server with automatic configuration loading and CLI overrides.
pub async fn run_server_with_default_config_overrides(
    host: Option<&str>,
    port: Option<u16>,
) -> Result<()> {
    info!("🚀 Starting Rust LiteLLM Gateway");

    let config = load_default_config_or_env(Path::new(DEFAULT_CONFIG_PATH)).await?;
    run_server_with_loaded_config(config, host, port).await
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

    let config_path = config_path.as_ref();
    let config = load_explicit_config(config_path).await?;
    run_server_with_loaded_config(config, host, port).await
}

pub(super) async fn load_default_config_or_env(config_path: &Path) -> Result<Config> {
    info!(
        "📄 Loading default configuration file: {}",
        config_path.display()
    );

    match Config::from_file(config_path).await {
        Ok(config) => {
            info!("✅ Configuration file loaded successfully");
            Ok(config)
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
                    Ok(config)
                }
                Err(env_error) => Err(GatewayError::Config(format!(
                    "Failed to load default configuration file ({}) and environment ({}).",
                    file_error, env_error
                ))),
            }
        }
    }
}

pub(super) async fn load_explicit_config(config_path: &Path) -> Result<Config> {
    info!(
        "📄 Loading explicit configuration file: {}",
        config_path.display()
    );

    Config::from_file(config_path).await.map_err(|file_error| {
        GatewayError::Config(format!(
            "Failed to load explicit configuration file {}: {}",
            config_path.display(),
            file_error
        ))
    })
}

async fn run_server_with_loaded_config(
    mut config: Config,
    host: Option<&str>,
    port: Option<u16>,
) -> Result<()> {
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
        "🌐 Server starting at: {}://{}:{}",
        server_scheme(config.server()),
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

fn server_scheme(config: &crate::config::models::server::ServerConfig) -> &'static str {
    if config.is_tls_enabled() {
        "https"
    } else {
        "http"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::server::TlsConfig;

    #[test]
    fn startup_scheme_tracks_tls_listener() {
        let mut config = crate::config::models::server::ServerConfig::default();
        assert_eq!(server_scheme(&config), "http");
        config.tls = Some(TlsConfig {
            cert_file: "cert.pem".into(),
            key_file: "key.pem".into(),
            ca_file: None,
            require_client_cert: false,
            http2: false,
        });
        assert_eq!(server_scheme(&config), "https");
    }
}
