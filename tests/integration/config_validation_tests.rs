//! Configuration validation integration tests
//!
//! Tests for configuration validation across all config components.
//! These tests verify that configuration validates correctly and fails
//! appropriately for invalid configurations.

#[cfg(test)]
mod tests {
    use litellm_rs::config::models::gateway::GatewayConfig;
    use litellm_rs::config::models::provider::{
        ProviderConfig, ProviderHealthCheckConfig, RetryConfig,
    };
    use litellm_rs::config::models::server::{CorsConfig, ServerConfig, TlsConfig};

    // ==================== GatewayConfig Validation ====================

    /// Test that valid gateway config passes validation
    #[test]
    fn test_valid_gateway_config() {
        let config = create_valid_gateway_config();
        assert!(config.validate().is_ok());
    }

    /// Test that server port 0 fails validation
    #[test]
    fn test_gateway_config_port_zero() {
        let mut config = create_valid_gateway_config();
        config.server.port = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("port"));
    }

    /// Test that empty providers list fails validation
    #[test]
    fn test_gateway_config_no_providers() {
        let mut config = create_valid_gateway_config();
        config.providers.clear();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("provider"));
    }

    /// Test that duplicate provider names fail validation
    #[test]
    fn test_gateway_config_duplicate_providers() {
        let mut config = create_valid_gateway_config();
        let duplicate_provider = config.providers[0].clone();
        config.providers.push(duplicate_provider);

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate"));
    }

    /// Test that provider without name fails validation
    #[test]
    fn test_gateway_config_empty_provider_name() {
        let mut config = create_valid_gateway_config();
        config.providers[0].name = String::new();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("name"));
    }

    /// Test that provider without api_key fails validation
    #[test]
    fn test_gateway_config_empty_api_key() {
        let mut config = create_valid_gateway_config();
        config.providers[0].api_key = String::new();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key"));
    }

    /// Test that empty database URL fails validation
    #[test]
    fn test_gateway_config_empty_database_url() {
        let mut config = create_valid_gateway_config();
        config.storage.database.enabled = true;
        config.storage.database.url = String::new();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Database URL"));
    }

    /// Test that empty JWT secret fails validation
    #[test]
    fn test_gateway_config_empty_jwt_secret() {
        let mut config = create_valid_gateway_config();
        config.auth.jwt_secret = String::new();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("JWT secret"));
    }

    // ==================== ServerConfig Validation ====================

    /// Test server config validation for port 0
    #[test]
    fn test_server_config_port_zero() {
        let config = ServerConfig {
            port: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Port"));
    }

    /// Test server config validation for timeout 0
    #[test]
    fn test_server_config_timeout_zero() {
        let config = ServerConfig {
            timeout: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Timeout"));
    }

    /// Test server config validation for max_body_size 0
    #[test]
    fn test_server_config_max_body_size_zero() {
        let config = ServerConfig {
            max_body_size: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Max body size"));
    }

    /// Test server address format
    #[test]
    fn test_server_config_address() {
        let config = ServerConfig::default();
        let address = config.address();
        assert!(address.contains(":"));
        assert!(address.ends_with(&config.port.to_string()));
    }

    /// Test server TLS detection
    #[test]
    fn test_server_config_tls_detection() {
        let mut config = ServerConfig::default();
        assert!(!config.is_tls_enabled());

        config.tls = Some(TlsConfig {
            cert_file: "/path/to/cert".to_string(),
            key_file: "/path/to/key".to_string(),
            ca_file: None,
            require_client_cert: false,
            http2: false,
        });
        assert!(config.is_tls_enabled());
    }

    // ==================== CorsConfig Validation ====================

    /// Test CORS validation for wildcard with credentials
    #[test]
    fn test_cors_config_wildcard_with_credentials() {
        let config = CorsConfig {
            allowed_origins: vec!["*".to_string()],
            allow_credentials: true,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("credentials"));
    }

    /// Test CORS allows all origins detection
    #[test]
    fn test_cors_allows_all_origins() {
        // Empty origins means allow all
        let config_empty = CorsConfig {
            allowed_origins: vec![],
            ..Default::default()
        };
        assert!(config_empty.allows_all_origins());

        // Explicit wildcard
        let config_wildcard = CorsConfig {
            allowed_origins: vec!["*".to_string()],
            ..Default::default()
        };
        assert!(config_wildcard.allows_all_origins());

        // Specific origins
        let config_specific = CorsConfig {
            allowed_origins: vec!["https://example.com".to_string()],
            ..Default::default()
        };
        assert!(!config_specific.allows_all_origins());
    }

    // ==================== ProviderConfig Defaults ====================

    /// Test provider config default values
    #[test]
    fn test_provider_config_defaults() {
        let config = ProviderConfig::default();

        assert!(config.name.is_empty());
        assert!(config.provider_type.is_empty());
        assert!(config.api_key.is_empty());
        assert!(config.base_url.is_none());
        assert!(config.enabled);
        assert!(config.weight > 0.0);
        assert!(config.rpm > 0);
        assert!(config.tpm > 0);
        assert!(config.timeout > 0);
    }

    /// Test provider config with custom values
    #[test]
    fn test_provider_config_custom_values() {
        let config = ProviderConfig {
            name: "openai-custom".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test-key".to_string(),
            base_url: Some("https://custom.openai.com/v1".to_string()),
            weight: 2.0,
            rpm: 100,
            tpm: 100000,
            timeout: 60,
            enabled: true,
            ..Default::default()
        };

        assert_eq!(config.name, "openai-custom");
        assert_eq!(config.weight, 2.0);
        assert!(config.base_url.is_some());
    }

    // ==================== RetryConfig Defaults ====================

    /// Test retry config default values
    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();

        assert!(config.base_delay > 0);
        assert!(config.max_delay > 0);
        assert!(config.max_delay > config.base_delay);
        assert!(config.backoff_multiplier > 1.0);
        assert!(config.jitter >= 0.0 && config.jitter <= 1.0);
    }

    // ==================== ProviderHealthCheckConfig Defaults ====================

    /// Test health check config default values
    #[test]
    fn test_health_check_config_defaults() {
        let config = ProviderHealthCheckConfig::default();

        assert!(config.interval > 0);
        assert!(config.failure_threshold > 0);
        assert!(config.recovery_timeout > 0);
        assert!(config.endpoint.is_none());
        assert!(!config.expected_codes.is_empty());
        assert!(config.expected_codes.contains(&200));
    }

    // ==================== GatewayConfig Feature Detection ====================

    /// Test feature detection
    #[test]
    fn test_gateway_config_features() {
        let config = create_valid_gateway_config();

        // Default values
        assert!(config.is_feature_enabled("health_checks"));

        // Based on config
        assert_eq!(
            config.is_feature_enabled("jwt_auth"),
            config.auth.enable_jwt
        );
        assert_eq!(
            config.is_feature_enabled("api_key_auth"),
            config.auth.enable_api_key
        );
        assert_eq!(config.is_feature_enabled("caching"), config.cache.enabled);
        assert_eq!(
            config.is_feature_enabled("rate_limiting"),
            config.rate_limit.enabled
        );

        // Unknown feature
        assert!(!config.is_feature_enabled("unknown_feature"));
    }

    /// Test provider lookup by name
    #[test]
    fn test_gateway_config_get_provider() {
        let config = create_valid_gateway_config();

        let provider = config.get_provider("test-openai");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name, "test-openai");

        let missing = config.get_provider("non-existent");
        assert!(missing.is_none());
    }

    /// Test provider lookup by type
    #[test]
    fn test_gateway_config_get_providers_by_type() {
        let config = create_valid_gateway_config();

        let openai_providers = config.get_providers_by_type("openai");
        assert!(!openai_providers.is_empty());

        let missing_type = config.get_providers_by_type("non-existent");
        assert!(missing_type.is_empty());
    }

    /// Test provider lookup by tag
    #[test]
    fn test_gateway_config_get_providers_by_tag() {
        let mut config = create_valid_gateway_config();
        config.providers[0].tags = vec!["fast".to_string(), "reliable".to_string()];

        let fast_providers = config.get_providers_by_tag("fast");
        assert!(!fast_providers.is_empty());

        let missing_tag = config.get_providers_by_tag("non-existent");
        assert!(missing_tag.is_empty());
    }

    // ==================== GatewayConfig Environment Presets ====================

    /// Test development environment configuration
    #[test]
    fn test_gateway_config_development_env() {
        let config = create_valid_gateway_config().for_environment("development");

        assert!(config.server.dev_mode);
        assert!(config.monitoring.tracing.enabled);
    }

    /// Test production environment configuration
    #[test]
    fn test_gateway_config_production_env() {
        let config = create_valid_gateway_config().for_environment("production");

        assert!(!config.server.dev_mode);
        assert!(config.monitoring.metrics.enabled);
        assert!(config.monitoring.tracing.enabled);
    }

    /// Test testing environment configuration
    #[test]
    fn test_gateway_config_testing_env() {
        let mut config = create_valid_gateway_config();
        config.cache.enabled = true;
        config.rate_limit.enabled = true;

        let test_config = config.for_environment("testing");

        assert!(test_config.server.dev_mode);
        assert!(!test_config.cache.enabled);
        assert!(!test_config.rate_limit.enabled);
    }

    // ==================== GatewayConfig Merge ====================

    /// Test configuration merge - server config
    #[test]
    fn test_gateway_config_merge_server() {
        let base = create_valid_gateway_config();
        let mut override_config = GatewayConfig::default();
        override_config.server.port = 9000;
        override_config.server.host = "192.168.1.1".to_string();

        let merged = base.merge(override_config);

        assert_eq!(merged.server.port, 9000);
        assert_eq!(merged.server.host, "192.168.1.1");
    }

    /// Test configuration merge - provider precedence
    #[test]
    fn test_gateway_config_merge_providers() {
        let base = create_valid_gateway_config();
        let mut override_config = GatewayConfig::default();

        // Add same provider name with different api_key
        override_config.providers.push(ProviderConfig {
            name: "test-openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "new-api-key".to_string(),
            ..Default::default()
        });

        let merged = base.merge(override_config);

        // Override should take precedence
        let provider = merged.get_provider("test-openai").unwrap();
        assert_eq!(provider.api_key, "new-api-key");
    }

    // ==================== Default Config Tests ====================

    /// Test GatewayConfig default values
    #[test]
    fn test_gateway_config_default() {
        let config = GatewayConfig::default();

        // Server has sensible defaults
        assert!(config.server.port > 0);
        assert!(!config.server.host.is_empty());

        // Providers is empty by default
        assert!(config.providers.is_empty());
    }

    /// Test ServerConfig default values
    #[test]
    fn test_server_config_default_values() {
        let config = ServerConfig::default();

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8000); // Default port is 8000
        assert!(config.timeout > 0);
        assert!(config.max_body_size > 0);
        assert!(!config.dev_mode);
        assert!(config.tls.is_none());
    }

    /// Test CorsConfig default values
    #[test]
    fn test_cors_config_default_values() {
        let config = CorsConfig::default();

        assert!(config.enabled);
        assert!(!config.allowed_methods.is_empty());
        assert!(!config.allowed_headers.is_empty());
        assert!(config.max_age > 0);
        assert!(!config.allow_credentials);
    }

    // ==================== Helper Functions ====================

    /// Create a valid gateway config for testing
    fn create_valid_gateway_config() -> GatewayConfig {
        let mut config = GatewayConfig::default();

        // Add required fields
        config.providers.push(ProviderConfig {
            name: "test-openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test-key-123".to_string(),
            ..Default::default()
        });

        config.storage.database.url = "postgres://localhost:5432/test".to_string();
        config.auth.jwt_secret = "test-jwt-secret-minimum-32-chars-long".to_string();

        config
    }
}
