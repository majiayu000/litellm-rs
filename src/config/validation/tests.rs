//! Tests for configuration validation
//!
//! This module contains all tests for the validation logic.

#[cfg(test)]
use super::ssrf::validate_url_against_ssrf;
use super::trait_def::Validate;
use crate::config::models::auth::AuthConfig;
use crate::config::models::cache::CacheConfig;
use crate::config::models::enterprise::{EnterpriseConfig, SsoConfig};
use crate::config::models::gateway::{GatewayConfig, GatewayPricingConfig, UnpricedModelPolicy};
use crate::config::models::provider::ProviderConfig;
use crate::config::models::rate_limit::RateLimitConfig;
use crate::config::models::server::ServerConfig;
use crate::config::models::storage::{DatabaseConfig, RedisConfig};

// ==================== Server Config Validation ====================

#[test]
fn test_server_config_validation() {
    let mut config = ServerConfig::default();
    // Use Validate::validate to call the trait method (not the inherent method)
    assert!(Validate::validate(&config).is_ok());

    config.port = 0;
    assert!(Validate::validate(&config).is_err());

    config.port = 8080;
    config.host = "".to_string();
    assert!(Validate::validate(&config).is_err());
}

#[test]
fn test_server_config_default_values() {
    let config = ServerConfig::default();
    assert_eq!(config.port, 8000);
    assert_eq!(config.host, "0.0.0.0");
}

#[test]
fn test_server_config_custom_port() {
    let config = ServerConfig {
        port: 3000,
        host: "127.0.0.1".to_string(),
        ..Default::default()
    };
    assert!(Validate::validate(&config).is_ok());
    assert_eq!(config.port, 3000);
}

#[test]
fn test_server_config_port_range() {
    // Valid ports
    for port in [80, 443, 8080, 8000, 3000, 65535] {
        let config = ServerConfig {
            port,
            ..Default::default()
        };
        assert!(Validate::validate(&config).is_ok());
    }
}

// ==================== Provider Config Validation ====================

#[test]
fn test_provider_config_validation() {
    let mut config = ProviderConfig {
        name: "test".to_string(),
        provider_type: "openai".to_string(),
        api_key: "test-key".to_string(),
        ..Default::default()
    };

    assert!(config.validate().is_ok());

    // Unknown provider selectors are rejected by runtime factory/catalog validation.
    config.provider_type = "custom_provider".to_string();
    assert!(config.validate().is_err());

    config.provider_type = "openai".to_string();
    config.weight = 0.0;
    assert!(config.validate().is_err());
}

#[test]
fn test_provider_config_all_types() {
    // Supported types should match what runtime factory/catalog can instantiate.
    let provider_types = ["openai", "anthropic", "mistral", "cloudflare", "groq"];

    for provider_type in provider_types {
        let config = ProviderConfig {
            name: "test".to_string(),
            provider_type: provider_type.to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        assert!(
            config.validate().is_ok(),
            "Provider type '{}' should be valid",
            provider_type
        );
    }
}

#[test]
fn test_provider_config_local_catalog_type_allows_empty_api_key() {
    let config = ProviderConfig {
        name: "local-vllm".to_string(),
        provider_type: "vllm".to_string(),
        api_key: "".to_string(),
        ..Default::default()
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_provider_config_weight_validation() {
    let mut config = ProviderConfig {
        name: "test".to_string(),
        provider_type: "openai".to_string(),
        api_key: "test-key".to_string(),
        weight: 1.0,
        ..Default::default()
    };

    assert!(config.validate().is_ok());

    // Zero weight is invalid
    config.weight = 0.0;
    assert!(config.validate().is_err());

    // Negative weight is invalid
    config.weight = -1.0;
    assert!(config.validate().is_err());

    // High weight is valid
    config.weight = 100.0;
    assert!(config.validate().is_ok());
}

#[test]
fn test_provider_config_empty_name() {
    let config = ProviderConfig {
        name: "".to_string(),
        provider_type: "openai".to_string(),
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_provider_health_check_resolves_relative_endpoint() {
    let config = ProviderConfig {
        name: "test".to_string(),
        provider_type: "openai".to_string(),
        api_key: "test-key".to_string(),
        base_url: Some("https://8.8.8.8/v1".to_string()),
        health_check: crate::config::models::provider::ProviderHealthCheckConfig {
            endpoint: Some("health".to_string()),
            expected_codes: vec![200, 204],
            ..Default::default()
        },
        ..Default::default()
    };

    assert!(config.validate().is_ok());
    assert_eq!(
        config
            .resolved_health_check_endpoint()
            .expect("endpoint should resolve")
            .expect("endpoint should be present")
            .as_str(),
        "https://8.8.8.8/v1/health"
    );

    let mut trailing_slash = config.clone();
    trailing_slash.base_url = Some("https://8.8.8.8/v1/".to_string());
    assert_eq!(
        trailing_slash
            .resolved_health_check_endpoint()
            .expect("endpoint should resolve")
            .expect("endpoint should be present")
            .as_str(),
        "https://8.8.8.8/v1/health"
    );

    let mut root_relative = config;
    root_relative.health_check.endpoint = Some("/health".to_string());
    assert_eq!(
        root_relative
            .resolved_health_check_endpoint()
            .expect("endpoint should resolve")
            .expect("endpoint should be present")
            .as_str(),
        "https://8.8.8.8/health"
    );
}

#[test]
fn test_provider_health_check_rejects_unsupported_combinations() {
    let mut config = ProviderConfig {
        name: "test".to_string(),
        provider_type: "openai".to_string(),
        api_key: "test-key".to_string(),
        ..Default::default()
    };

    config.health_check.endpoint = Some("/health".to_string());
    assert!(config.validate().unwrap_err().contains("requires base_url"));

    config.health_check.endpoint = None;
    config.health_check.expected_codes = vec![204];
    assert!(
        config
            .validate()
            .unwrap_err()
            .contains("requires a custom endpoint")
    );

    config.health_check.endpoint = Some("https://8.8.8.8/health".to_string());
    config.health_check.expected_codes = vec![200, 200];
    assert!(config.validate().unwrap_err().contains("duplicated"));

    config.health_check.expected_codes = vec![99];
    assert!(
        config
            .validate()
            .unwrap_err()
            .contains("between 100 and 599")
    );

    config.health_check.expected_codes = vec![600];
    assert!(
        config
            .validate()
            .unwrap_err()
            .contains("between 100 and 599")
    );

    config.health_check.endpoint = Some("https://user:secret@8.8.8.8/health".to_string());
    config.health_check.expected_codes = vec![200];
    assert!(
        config
            .validate()
            .unwrap_err()
            .contains("cannot contain URL credentials")
    );
}

#[test]
fn test_provider_health_check_endpoint_uses_ssrf_validation() {
    let config = ProviderConfig {
        name: "test".to_string(),
        provider_type: "openai".to_string(),
        api_key: "test-key".to_string(),
        health_check: crate::config::models::provider::ProviderHealthCheckConfig {
            endpoint: Some("http://127.0.0.1/health".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    assert!(config.validate().unwrap_err().contains("SSRF protection"));
}

// ==================== Auth Config Validation ====================

#[test]
fn test_auth_config_validation() {
    let mut config = AuthConfig {
        enable_jwt: true,
        jwt_secret: "A-very-long-secret-key-for-testing-purposes123!".to_string(),
        ..Default::default()
    };
    assert!(config.validate().is_ok());

    config.jwt_secret = "short".to_string();
    assert!(config.validate().is_err());

    config.jwt_secret = "".to_string();
    assert!(config.validate().is_err());
}

#[test]
fn test_auth_config_jwt_secret_min_length() {
    // Exactly minimum length with mixed characters (required by validation)
    let config = AuthConfig {
        enable_jwt: true,
        jwt_secret: "aA1!".repeat(8), // 32 bytes with mixed case, numbers, and special chars
        ..Default::default()
    };
    assert!(config.validate().is_ok());

    // Just under minimum length
    let config = AuthConfig {
        enable_jwt: true,
        jwt_secret: "aA1!".repeat(7) + "aA1", // 31 bytes
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

// ==================== Enabled Flag Validation ====================

#[test]
fn test_cache_validation_skips_when_disabled() {
    let config = CacheConfig {
        enabled: false,
        ttl: 0,
        max_size: 0,
        semantic_cache: false,
        similarity_threshold: 2.0,
    };
    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn test_cache_validation_accepts_wired_cache_enabled() {
    let config = CacheConfig {
        enabled: true,
        ..Default::default()
    };

    assert!(config.validate().is_ok());
}

#[test]
fn test_cache_validation_rejects_enabled_zero_ttl() {
    let config = CacheConfig {
        enabled: true,
        ttl: 0,
        ..Default::default()
    };

    let error = config.validate().unwrap_err();
    assert!(error.contains("ttl"));
    assert!(error.contains("greater than 0"));
}

#[test]
fn test_cache_validation_rejects_unwired_semantic_cache() {
    let config = CacheConfig {
        semantic_cache: true,
        ..Default::default()
    };

    let error = config.validate().unwrap_err();
    assert!(error.contains("not wired into runtime"));
}

#[test]
fn test_gateway_validation_rejects_unwired_semantic_cache() {
    let mut config = GatewayConfig {
        providers: vec![ProviderConfig {
            name: "test-provider".to_string(),
            provider_type: "openai".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    config.storage.database.enabled = true;
    config.storage.database.url = "postgres://localhost/test".to_string();
    config.auth.jwt_secret = "StrongJwtSecretWithMixedCaseAndNumbers1234!".to_string();
    config.cache.semantic_cache = true;

    let error = config.validate().unwrap_err();
    assert!(error.to_ascii_lowercase().contains("semantic cache"));
    assert!(error.contains("not wired into runtime"));
}

#[test]
fn test_rate_limit_validation_skips_when_disabled() {
    let config = RateLimitConfig {
        enabled: false,
        default_rpm: 0,
        default_tpm: 0,
        ..Default::default()
    };
    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn test_rate_limit_validation_rejects_unenforced_tpm_when_enabled() {
    let config = RateLimitConfig {
        enabled: true,
        default_tpm: 50_000,
        ..Default::default()
    };

    let error = Validate::validate(&config).unwrap_err();
    assert!(error.contains("default_tpm"));
    assert!(error.contains("not enforced"));
}

#[test]
fn test_rate_limit_validation_rejects_unenforced_burst_when_enabled() {
    let config = RateLimitConfig {
        enabled: true,
        burst_size: Some(20),
        ..Default::default()
    };

    let error = Validate::validate(&config).unwrap_err();
    assert!(error.contains("burst_size"));
    assert!(error.contains("not enforced"));
}

#[test]
fn test_rate_limit_validation_allows_enforced_rpm_only() {
    let config = RateLimitConfig {
        enabled: true,
        default_rpm: 500,
        ..Default::default()
    };

    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn test_rate_limit_validation_allows_requests_per_minute_alias() {
    let config = RateLimitConfig {
        enabled: true,
        requests_per_minute: Some(25),
        ..Default::default()
    };

    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn test_rate_limit_validation_allows_requests_per_minute_alias_with_zero_default_rpm() {
    let config = RateLimitConfig {
        enabled: true,
        default_rpm: 0,
        requests_per_minute: Some(25),
        ..Default::default()
    };

    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn test_rate_limit_validation_rejects_zero_requests_per_minute_alias() {
    let config = RateLimitConfig {
        enabled: true,
        requests_per_minute: Some(0),
        ..Default::default()
    };

    let error = Validate::validate(&config).unwrap_err();
    assert!(error.contains("Effective RPM"));
    assert!(error.contains("greater than 0"));
}

#[test]
fn test_database_validation_skips_when_disabled() {
    let config = DatabaseConfig {
        enabled: false,
        url: "".to_string(),
        max_connections: 0,
        connection_timeout: 0,
        ssl: false,
        auto_migrate: false,
        auto_migrate_configured: false,
        fallback_to_sqlite: false,
        allow_degraded: false,
    };
    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn test_redis_validation_skips_when_disabled() {
    let config = RedisConfig {
        enabled: false,
        url: "".to_string(),
        max_connections: 0,
        connection_timeout: 0,
        cluster: false,
        allow_degraded: false,
    };
    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn test_pricing_validation_defaults_to_reject_unpriced_models() {
    let config = GatewayPricingConfig::default();

    assert_eq!(config.unpriced_model_policy, UnpricedModelPolicy::Reject);
    assert!(config.unpriced_fallback_cost_per_1k_tokens.is_none());
    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn test_pricing_validation_accepts_finite_unpriced_fallback() {
    for cost in [0.0, 0.25] {
        let mut config = GatewayPricingConfig::default();
        config.unpriced_fallback_cost_per_1k_tokens = Some(cost);

        assert!(Validate::validate(&config).is_ok());
    }
}

#[test]
fn test_pricing_validation_rejects_invalid_unpriced_fallback() {
    for cost in [-0.01, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut config = GatewayPricingConfig::default();
        config.unpriced_fallback_cost_per_1k_tokens = Some(cost);

        let error = Validate::validate(&config).unwrap_err();
        assert!(error.contains("unpriced_fallback_cost_per_1k_tokens"));
        assert!(error.contains("finite"));
    }
}

#[test]
fn test_enterprise_validation_skips_when_disabled() {
    let config = EnterpriseConfig {
        enabled: false,
        sso: Some(SsoConfig {
            provider: "invalid-provider".to_string(),
            client_id: "".to_string(),
            client_secret: "".to_string(),
            redirect_url: "".to_string(),
            settings: Default::default(),
        }),
        audit_logging: false,
        advanced_analytics: false,
    };
    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn test_enterprise_validation_rejects_unwired_audit_logging() {
    let config = EnterpriseConfig {
        audit_logging: true,
        ..Default::default()
    };

    let error = Validate::validate(&config).unwrap_err();
    assert!(error.contains("enterprise.audit_logging"));
    assert!(error.contains("not wired into the gateway runtime"));
}

#[test]
fn test_enterprise_validation_rejects_unwired_advanced_analytics() {
    let config = EnterpriseConfig {
        advanced_analytics: true,
        ..Default::default()
    };

    let error = Validate::validate(&config).unwrap_err();
    assert!(error.contains("enterprise.advanced_analytics"));
    assert!(error.contains("not wired into the gateway runtime"));
}

// ==================== SSRF Validation - Valid URLs ====================

#[test]
fn test_ssrf_validation_valid_urls() {
    // Valid public URLs should pass
    assert!(validate_url_against_ssrf("https://api.openai.com/v1", "test").is_ok());
    assert!(validate_url_against_ssrf("https://api.anthropic.com", "test").is_ok());
    assert!(validate_url_against_ssrf("http://example.com:8080/api", "test").is_ok());
}

#[test]
fn test_ssrf_validation_https_urls() {
    // Use literal public IPs — fictional subdomains may not resolve in all test environments
    assert!(validate_url_against_ssrf("https://8.8.8.8", "test").is_ok());
    assert!(validate_url_against_ssrf("https://api.github.com", "test").is_ok());
    assert!(validate_url_against_ssrf("https://google.com", "test").is_ok());
}

#[test]
fn test_ssrf_validation_http_urls() {
    // Use literal public IPs — fictional subdomains may not resolve in all test environments
    assert!(validate_url_against_ssrf("http://8.8.8.8", "test").is_ok());
    assert!(validate_url_against_ssrf("http://example.com:9000", "test").is_ok());
}

#[test]
fn test_ssrf_validation_url_with_path() {
    assert!(validate_url_against_ssrf("https://8.8.8.8/v1/chat", "test").is_ok());
    assert!(validate_url_against_ssrf("https://example.com/api/v2/messages", "test").is_ok());
}

#[test]
fn test_ssrf_validation_url_with_query() {
    assert!(validate_url_against_ssrf("https://8.8.8.8/v1?key=value", "test").is_ok());
}

// ==================== SSRF Validation - Localhost ====================

#[test]
fn test_ssrf_validation_localhost() {
    // Localhost should be blocked
    assert!(validate_url_against_ssrf("http://localhost/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://localhost:8080/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://LOCALHOST/api", "test").is_err());
}

#[test]
fn test_ssrf_validation_localhost_variations() {
    assert!(validate_url_against_ssrf("https://localhost", "test").is_err());
    assert!(validate_url_against_ssrf("http://localhost:3000", "test").is_err());
    assert!(validate_url_against_ssrf("http://LocalHost/api", "test").is_err());
}

// ==================== SSRF Validation - Loopback ====================

#[test]
fn test_ssrf_validation_loopback() {
    // Loopback addresses should be blocked
    assert!(validate_url_against_ssrf("http://127.0.0.1/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://127.0.0.1:8080/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://[::1]/api", "test").is_err());
}

#[test]
fn test_ssrf_validation_loopback_range() {
    // All 127.x.x.x addresses are loopback
    assert!(validate_url_against_ssrf("http://127.0.0.2/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://127.255.255.255/api", "test").is_err());
}

// ==================== SSRF Validation - Private IP ====================

#[test]
fn test_ssrf_validation_private_ip() {
    // Private IP ranges should be blocked
    assert!(validate_url_against_ssrf("http://10.0.0.1/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://172.16.0.1/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://192.168.1.1/api", "test").is_err());
}

#[test]
fn test_ssrf_validation_private_ip_10_range() {
    assert!(validate_url_against_ssrf("http://10.0.0.0/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://10.255.255.255/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://10.100.50.25/api", "test").is_err());
}

#[test]
fn test_ssrf_validation_private_ip_172_range() {
    assert!(validate_url_against_ssrf("http://172.16.0.0/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://172.31.255.255/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://172.20.100.50/api", "test").is_err());
}

#[test]
fn test_ssrf_validation_private_ip_192_range() {
    assert!(validate_url_against_ssrf("http://192.168.0.0/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://192.168.255.255/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://192.168.100.50/api", "test").is_err());
}

// ==================== SSRF Validation - Metadata Endpoints ====================

#[test]
fn test_ssrf_validation_metadata_endpoints() {
    // Cloud metadata endpoints should be blocked
    assert!(validate_url_against_ssrf("http://169.254.169.254/latest/meta-data", "test").is_err());
    assert!(
        validate_url_against_ssrf("http://metadata.google.internal/computeMetadata", "test")
            .is_err()
    );
}

#[test]
fn test_ssrf_validation_aws_metadata() {
    assert!(validate_url_against_ssrf("http://169.254.169.254/latest", "test").is_err());
    assert!(
        validate_url_against_ssrf("http://169.254.169.254/latest/meta-data/iam", "test").is_err()
    );
}

#[test]
fn test_ssrf_validation_link_local() {
    // Link-local addresses (169.254.0.0/16) should be blocked
    assert!(validate_url_against_ssrf("http://169.254.0.1/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://169.254.100.50/api", "test").is_err());
}

// ==================== SSRF Validation - Encoded IP ====================

#[test]
fn test_ssrf_validation_encoded_ip() {
    // Decimal-encoded IP addresses should be blocked
    // 2130706433 = 127.0.0.1
    assert!(validate_url_against_ssrf("http://2130706433/api", "test").is_err());
    // 167772161 = 10.0.0.1
    assert!(validate_url_against_ssrf("http://167772161/api", "test").is_err());
}

#[test]
fn test_ssrf_validation_hex_encoded_ip() {
    // Hex-encoded IP addresses should be blocked
    // 0x7f000001 = 127.0.0.1
    assert!(validate_url_against_ssrf("http://0x7f000001/api", "test").is_err());
    // 0x0a000001 = 10.0.0.1
    assert!(validate_url_against_ssrf("http://0x0a000001/api", "test").is_err());
}

// ==================== SSRF Validation - Invalid Scheme ====================

#[test]
fn test_ssrf_validation_invalid_scheme() {
    // Non-HTTP schemes should be blocked
    assert!(validate_url_against_ssrf("file:///etc/passwd", "test").is_err());
    assert!(validate_url_against_ssrf("ftp://example.com", "test").is_err());
    assert!(validate_url_against_ssrf("gopher://example.com", "test").is_err());
}

#[test]
fn test_ssrf_validation_other_schemes() {
    assert!(validate_url_against_ssrf("ssh://example.com", "test").is_err());
    assert!(validate_url_against_ssrf("telnet://example.com", "test").is_err());
    assert!(validate_url_against_ssrf("data:text/html,<script>", "test").is_err());
}

// ==================== SSRF Validation - Invalid URL ====================

#[test]
fn test_ssrf_validation_invalid_url() {
    assert!(validate_url_against_ssrf("not a url", "test").is_err());
    assert!(validate_url_against_ssrf("", "test").is_err());
    assert!(validate_url_against_ssrf("://missing-scheme", "test").is_err());
}

// ==================== Provider Config SSRF ====================

#[test]
fn test_provider_config_ssrf_validation() {
    let mut config = ProviderConfig {
        name: "test".to_string(),
        provider_type: "openai".to_string(),
        api_key: "test-key".to_string(),
        base_url: Some("http://localhost:8080".to_string()),
        ..Default::default()
    };

    // Should fail with localhost
    assert!(config.validate().is_err());

    // Should pass with valid public URL
    config.base_url = Some("https://api.openai.com/v1".to_string());
    assert!(config.validate().is_ok());

    // Should fail with private IP
    config.base_url = Some("http://192.168.1.1/api".to_string());
    assert!(config.validate().is_err());

    // Should fail with metadata endpoint
    config.base_url = Some("http://169.254.169.254/latest".to_string());
    assert!(config.validate().is_err());
}

#[test]
fn test_provider_config_no_base_url() {
    let config = ProviderConfig {
        name: "test".to_string(),
        provider_type: "openai".to_string(),
        api_key: "test-key".to_string(),
        base_url: None,
        ..Default::default()
    };
    assert!(config.validate().is_ok());
}

// ==================== Edge Cases ====================

#[test]
fn test_ssrf_unspecified_address() {
    assert!(validate_url_against_ssrf("http://0.0.0.0/api", "test").is_err());
    assert!(validate_url_against_ssrf("http://0/api", "test").is_err());
}

#[test]
fn test_ssrf_context_message() {
    let result = validate_url_against_ssrf("http://localhost/api", "provider_url");
    assert!(result.is_err());
    let error_message = result.unwrap_err();
    assert!(error_message.contains("provider_url"));
    assert!(error_message.contains("SSRF"));
}
