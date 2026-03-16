//! Server configuration builder implementation

use super::types::ServerConfigBuilder;
use crate::config::models::server::ServerConfig;
use crate::utils::data::type_utils::Builder;
use std::time::Duration;

impl ServerConfigBuilder {
    /// Create a new server configuration builder
    pub fn new() -> Self {
        Self {
            host: None,
            port: None,
            workers: None,
            timeout: None,
            max_connections: None,
            enable_cors: false,
            cors_origins: Vec::new(),
        }
    }

    /// Set the host
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// Set the port
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Set the number of workers
    pub fn workers(mut self, workers: usize) -> Self {
        self.workers = Some(workers);
        self
    }

    /// Set the request timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the maximum number of connections
    pub fn max_connections(mut self, max_connections: usize) -> Self {
        self.max_connections = Some(max_connections);
        self
    }

    /// Enable CORS
    pub fn enable_cors(mut self) -> Self {
        self.enable_cors = true;
        self
    }

    /// Add CORS origin
    pub fn add_cors_origin(mut self, origin: impl Into<String>) -> Self {
        self.cors_origins.push(origin.into());
        self
    }

    /// Build the server configuration
    pub fn build(self) -> ServerConfig {
        ServerConfig {
            host: self.host.unwrap_or_else(|| "127.0.0.1".to_string()),
            port: self.port.unwrap_or(8080),
            workers: self.workers,
            max_connections: self.max_connections,
            timeout: self.timeout.map(|d| d.as_secs()).unwrap_or(30),
            max_body_size: 1024 * 1024, // 1MB default
            dev_mode: false,
            tls: None,
            cors: crate::config::models::server::CorsConfig {
                enabled: self.enable_cors,
                allowed_origins: self.cors_origins,
                allowed_methods: vec!["GET".to_string(), "POST".to_string(), "OPTIONS".to_string()],
                allowed_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
                max_age: 3600,
                allow_credentials: false,
            },
            features: Vec::new(),
            trusted_proxies: Vec::new(),
        }
    }
}

impl Default for ServerConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder<ServerConfig> for ServerConfigBuilder {
    fn build(self) -> ServerConfig {
        self.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ServerConfigBuilder Construction Tests ====================

    #[test]
    fn test_server_config_builder_new() {
        let builder = ServerConfigBuilder::new();
        assert!(builder.host.is_none());
        assert!(builder.port.is_none());
        assert!(builder.workers.is_none());
        assert!(builder.timeout.is_none());
        assert!(builder.max_connections.is_none());
        assert!(!builder.enable_cors);
        assert!(builder.cors_origins.is_empty());
    }

    #[test]
    fn test_server_config_builder_default() {
        let builder = ServerConfigBuilder::default();
        assert!(builder.host.is_none());
        assert!(builder.port.is_none());
    }

    // ==================== Builder Method Tests ====================

    #[test]
    fn test_server_config_builder_host() {
        let builder = ServerConfigBuilder::new().host("0.0.0.0");
        assert_eq!(builder.host, Some("0.0.0.0".to_string()));
    }

    #[test]
    fn test_server_config_builder_host_string() {
        let builder = ServerConfigBuilder::new().host(String::from("localhost"));
        assert_eq!(builder.host, Some("localhost".to_string()));
    }

    #[test]
    fn test_server_config_builder_port() {
        let builder = ServerConfigBuilder::new().port(3000);
        assert_eq!(builder.port, Some(3000));
    }

    #[test]
    fn test_server_config_builder_workers() {
        let builder = ServerConfigBuilder::new().workers(4);
        assert_eq!(builder.workers, Some(4));
    }

    #[test]
    fn test_server_config_builder_timeout() {
        let timeout = Duration::from_secs(120);
        let builder = ServerConfigBuilder::new().timeout(timeout);
        assert_eq!(builder.timeout, Some(Duration::from_secs(120)));
    }

    #[test]
    fn test_server_config_builder_max_connections() {
        let builder = ServerConfigBuilder::new().max_connections(5000);
        assert_eq!(builder.max_connections, Some(5000));
    }

    #[test]
    fn test_server_config_builder_enable_cors() {
        let builder = ServerConfigBuilder::new().enable_cors();
        assert!(builder.enable_cors);
    }

    #[test]
    fn test_server_config_builder_add_cors_origin() {
        let builder = ServerConfigBuilder::new().add_cors_origin("https://example.com");
        assert_eq!(builder.cors_origins, vec!["https://example.com"]);
    }

    #[test]
    fn test_server_config_builder_add_multiple_cors_origins() {
        let builder = ServerConfigBuilder::new()
            .add_cors_origin("https://example.com")
            .add_cors_origin("https://other.com");
        assert_eq!(builder.cors_origins.len(), 2);
        assert!(
            builder
                .cors_origins
                .contains(&"https://example.com".to_string())
        );
        assert!(
            builder
                .cors_origins
                .contains(&"https://other.com".to_string())
        );
    }

    // ==================== Builder Chain Tests ====================

    #[test]
    fn test_server_config_builder_chain() {
        let builder = ServerConfigBuilder::new()
            .host("0.0.0.0")
            .port(9000)
            .workers(8)
            .timeout(Duration::from_secs(60))
            .max_connections(10000)
            .enable_cors()
            .add_cors_origin("*");

        assert_eq!(builder.host, Some("0.0.0.0".to_string()));
        assert_eq!(builder.port, Some(9000));
        assert_eq!(builder.workers, Some(8));
        assert_eq!(builder.timeout, Some(Duration::from_secs(60)));
        assert_eq!(builder.max_connections, Some(10000));
        assert!(builder.enable_cors);
        assert_eq!(builder.cors_origins, vec!["*"]);
    }

    // ==================== Build Tests ====================

    #[test]
    fn test_server_config_builder_build_defaults() {
        let config = ServerConfigBuilder::new().build();

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert_eq!(config.timeout, 30);
        assert!(!config.cors.enabled);
    }

    #[test]
    fn test_server_config_builder_build_with_values() {
        let config = ServerConfigBuilder::new()
            .host("0.0.0.0")
            .port(3000)
            .workers(4)
            .timeout(Duration::from_secs(120))
            .build();

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 3000);
        assert_eq!(config.workers, Some(4));
        assert_eq!(config.timeout, 120);
    }

    #[test]
    fn test_server_config_builder_build_with_cors() {
        let config = ServerConfigBuilder::new()
            .enable_cors()
            .add_cors_origin("https://example.com")
            .add_cors_origin("https://other.com")
            .build();

        assert!(config.cors.enabled);
        assert_eq!(config.cors.allowed_origins.len(), 2);
        assert!(
            config
                .cors
                .allowed_origins
                .contains(&"https://example.com".to_string())
        );
    }

    #[test]
    fn test_server_config_builder_build_cors_default_origins() {
        let config = ServerConfigBuilder::new().enable_cors().build();

        assert!(config.cors.enabled);
        assert!(config.cors.allowed_origins.is_empty());
    }

    #[test]
    fn test_server_config_builder_build_cors_methods() {
        let config = ServerConfigBuilder::new().build();

        assert!(config.cors.allowed_methods.contains(&"GET".to_string()));
        assert!(config.cors.allowed_methods.contains(&"POST".to_string()));
        assert!(config.cors.allowed_methods.contains(&"OPTIONS".to_string()));
    }

    #[test]
    fn test_server_config_builder_build_cors_headers() {
        let config = ServerConfigBuilder::new().build();

        assert!(
            config
                .cors
                .allowed_headers
                .contains(&"Content-Type".to_string())
        );
        assert!(
            config
                .cors
                .allowed_headers
                .contains(&"Authorization".to_string())
        );
    }

    #[test]
    fn test_server_config_builder_build_max_body_size() {
        let config = ServerConfigBuilder::new().build();
        assert_eq!(config.max_body_size, 1024 * 1024); // 1MB
    }

    #[test]
    fn test_server_config_builder_build_dev_mode() {
        let config = ServerConfigBuilder::new().build();
        assert!(!config.dev_mode);
    }

    #[test]
    fn test_server_config_builder_build_tls() {
        let config = ServerConfigBuilder::new().build();
        assert!(config.tls.is_none());
    }

    // ==================== Builder Trait Tests ====================

    #[test]
    fn test_server_config_builder_trait() {
        use crate::utils::data::type_utils::Builder;

        let builder = ServerConfigBuilder::new().host("localhost").port(8000);

        let config: ServerConfig = Builder::build(builder);
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 8000);
    }

    // ==================== Clone and Debug Tests ====================

    #[test]
    fn test_server_config_builder_clone() {
        let builder = ServerConfigBuilder::new().host("test").port(1234);
        let cloned = builder.clone();

        assert_eq!(builder.host, cloned.host);
        assert_eq!(builder.port, cloned.port);
    }

    #[test]
    fn test_server_config_builder_debug() {
        let builder = ServerConfigBuilder::new().port(8080);
        let debug_str = format!("{:?}", builder);

        assert!(debug_str.contains("ServerConfigBuilder"));
        assert!(debug_str.contains("8080"));
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_server_config_builder_port_zero() {
        let config = ServerConfigBuilder::new().port(0).build();
        assert_eq!(config.port, 0);
    }

    #[test]
    fn test_server_config_builder_port_max() {
        let config = ServerConfigBuilder::new().port(65535).build();
        assert_eq!(config.port, 65535);
    }

    #[test]
    fn test_server_config_builder_workers_zero() {
        let config = ServerConfigBuilder::new().workers(0).build();
        assert_eq!(config.workers, Some(0));
    }

    #[test]
    fn test_server_config_builder_timeout_zero() {
        let config = ServerConfigBuilder::new().timeout(Duration::ZERO).build();
        assert_eq!(config.timeout, 0);
    }

    #[test]
    fn test_server_config_builder_empty_host() {
        let config = ServerConfigBuilder::new().host("").build();
        assert_eq!(config.host, "");
    }

    #[test]
    fn test_server_config_builder_empty_cors_origin() {
        let builder = ServerConfigBuilder::new().add_cors_origin("");
        assert_eq!(builder.cors_origins, vec![""]);
    }
}
