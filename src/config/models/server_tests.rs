use super::*;

// ==================== ServerConfig Default Tests ====================

#[test]
fn test_server_config_default() {
    let config = ServerConfig::default();
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 8000);
    assert!(config.workers.is_none());
    assert_eq!(config.timeout, 30);
    assert_eq!(config.max_body_size, 10 * 1024 * 1024);
    assert!(!config.dev_mode);
    assert!(config.tls.is_none());
}

#[test]
fn test_server_config_structure() {
    let config = ServerConfig {
        host: "127.0.0.1".to_string(),
        port: 3000,
        workers: Some(4),
        timeout: 60,
        max_body_size: 5 * 1024 * 1024,
        dev_mode: true,
        tls: None,
        cors: CorsConfig::default(),
        ..ServerConfig::default()
    };
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 3000);
    assert_eq!(config.workers, Some(4));
    assert!(config.dev_mode);
}

#[test]
fn test_server_config_address() {
    let config = ServerConfig::default();
    assert_eq!(config.address(), "0.0.0.0:8000");

    let custom = ServerConfig {
        host: "localhost".to_string(),
        port: 3000,
        ..ServerConfig::default()
    };
    assert_eq!(custom.address(), "localhost:3000");
}

#[test]
fn test_server_config_is_tls_enabled() {
    let config = ServerConfig::default();
    assert!(!config.is_tls_enabled());

    let with_tls = ServerConfig {
        tls: Some(TlsConfig {
            cert_file: "/path/to/cert.pem".to_string(),
            key_file: "/path/to/key.pem".to_string(),
            ca_file: None,
            require_client_cert: false,
            http2: false,
        }),
        ..ServerConfig::default()
    };
    assert!(with_tls.is_tls_enabled());
}

#[test]
fn test_server_config_worker_count() {
    let config = ServerConfig::default();
    assert!(config.worker_count() > 0);

    let with_workers = ServerConfig {
        workers: Some(8),
        ..ServerConfig::default()
    };
    assert_eq!(with_workers.worker_count(), 8);
}

// ==================== ServerConfig Validation Tests ====================

#[test]
fn test_server_config_validate_success() {
    let config = ServerConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_server_config_validate_port_zero() {
    let config = ServerConfig {
        port: 0,
        ..ServerConfig::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("between 1 and 65535"));
}

#[test]
fn test_server_config_deserialize_rejects_port_above_65535() {
    let json = r#"{"port": 70000}"#;
    let result = serde_json::from_str::<ServerConfig>(json);
    assert!(
        result.is_err(),
        "port 70000 should be rejected by u16 deserialization"
    );
}

#[test]
fn test_server_config_deserialize_rejects_negative_port() {
    let json = r#"{"port": -1}"#;
    let result = serde_json::from_str::<ServerConfig>(json);
    assert!(
        result.is_err(),
        "negative port should be rejected by u16 deserialization"
    );
}

#[test]
fn test_server_config_validate_timeout_zero() {
    let config = ServerConfig {
        timeout: 0,
        ..ServerConfig::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Timeout"));
}

#[test]
fn test_server_config_validate_max_body_size_zero() {
    let config = ServerConfig {
        max_body_size: 0,
        ..ServerConfig::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("body size"));
}

// ==================== ServerConfig Merge Tests ====================

#[test]
fn test_server_config_merge_host() {
    let base = ServerConfig::default();
    let other = ServerConfig {
        host: "192.168.1.1".to_string(),
        ..ServerConfig::default()
    };
    let merged = base.merge(other);
    assert_eq!(merged.host, "192.168.1.1");
}

#[test]
fn test_server_config_merge_port() {
    let base = ServerConfig::default();
    let other = ServerConfig {
        port: 9000,
        ..ServerConfig::default()
    };
    let merged = base.merge(other);
    assert_eq!(merged.port, 9000);
}

#[test]
fn test_server_config_merge_workers() {
    let base = ServerConfig::default();
    let other = ServerConfig {
        workers: Some(16),
        ..ServerConfig::default()
    };
    let merged = base.merge(other);
    assert_eq!(merged.workers, Some(16));
}

#[test]
fn test_server_config_merge_dev_mode() {
    let base = ServerConfig::default();
    let other = ServerConfig {
        dev_mode: true,
        ..ServerConfig::default()
    };
    let merged = base.merge(other);
    assert!(merged.dev_mode);
}

// ==================== ServerConfig Serialization Tests ====================

#[test]
fn test_server_config_serialization() {
    let config = ServerConfig::default();
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["host"], "0.0.0.0");
    assert_eq!(json["port"], 8000);
    assert_eq!(json["timeout"], 30);
}

#[test]
fn test_server_config_deserialization() {
    let json = r#"{
            "host": "10.0.0.1",
            "port": 4000,
            "timeout": 120,
            "max_body_size": 20971520,
            "dev_mode": true
        }"#;
    let config: ServerConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.host, "10.0.0.1");
    assert_eq!(config.port, 4000);
    assert_eq!(config.timeout, 120);
    assert!(config.dev_mode);
}

#[test]
fn test_server_config_clone() {
    let config = ServerConfig::default();
    let cloned = config.clone();
    assert_eq!(config.host, cloned.host);
    assert_eq!(config.port, cloned.port);
}

// ==================== TlsConfig Tests ====================

#[test]
fn test_tls_config_structure() {
    let config = TlsConfig {
        cert_file: "/etc/ssl/cert.pem".to_string(),
        key_file: "/etc/ssl/key.pem".to_string(),
        ca_file: Some("/etc/ssl/ca.pem".to_string()),
        require_client_cert: true,
        http2: false,
    };
    assert_eq!(config.cert_file, "/etc/ssl/cert.pem");
    assert_eq!(config.key_file, "/etc/ssl/key.pem");
    assert!(config.ca_file.is_some());
    assert!(config.require_client_cert);
}

#[test]
fn test_tls_config_validate_empty_cert() {
    let config = TlsConfig {
        cert_file: "".to_string(),
        key_file: "/path/to/key.pem".to_string(),
        ca_file: None,
        require_client_cert: false,
        http2: false,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("certificate"));
}

#[test]
fn test_tls_config_validate_empty_key() {
    let config = TlsConfig {
        cert_file: "/path/to/cert.pem".to_string(),
        key_file: "".to_string(),
        ca_file: None,
        require_client_cert: false,
        http2: false,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("key"));
}

#[test]
fn test_tls_config_serialization() {
    let config = TlsConfig {
        cert_file: "cert.pem".to_string(),
        key_file: "key.pem".to_string(),
        ca_file: None,
        require_client_cert: false,
        http2: false,
    };
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["cert_file"], "cert.pem");
    assert_eq!(json["key_file"], "key.pem");
}

#[test]
fn test_tls_config_deserialization() {
    let json = r#"{
            "cert_file": "/ssl/cert.pem",
            "key_file": "/ssl/key.pem",
            "ca_file": "/ssl/ca.pem",
            "require_client_cert": true
        }"#;
    let config: TlsConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.cert_file, "/ssl/cert.pem");
    assert!(config.require_client_cert);
}

#[test]
fn test_tls_config_clone() {
    let config = TlsConfig {
        cert_file: "cert.pem".to_string(),
        key_file: "key.pem".to_string(),
        ca_file: None,
        require_client_cert: false,
        http2: false,
    };
    let cloned = config.clone();
    assert_eq!(config.cert_file, cloned.cert_file);
}

// ==================== CorsConfig Default Tests ====================

#[test]
fn test_cors_config_default() {
    let config = CorsConfig::default();
    assert!(config.enabled);
    assert!(config.allowed_origins.is_empty());
    assert_eq!(config.allowed_methods.len(), 5);
    assert!(config.allowed_methods.contains(&"GET".to_string()));
    assert!(config.allowed_methods.contains(&"POST".to_string()));
    assert!(config.allowed_methods.contains(&"PUT".to_string()));
    assert!(config.allowed_methods.contains(&"DELETE".to_string()));
    assert!(config.allowed_methods.contains(&"OPTIONS".to_string()));
    assert_eq!(config.allowed_headers.len(), 3);
    assert_eq!(config.max_age, 3600);
    assert!(!config.allow_credentials);
}

#[test]
fn test_cors_config_allows_all_origins_empty() {
    let config = CorsConfig::default();
    assert!(!config.allows_all_origins());
}

#[test]
fn test_cors_config_allows_all_origins_wildcard() {
    let config = CorsConfig {
        allowed_origins: vec!["*".to_string()],
        ..CorsConfig::default()
    };
    assert!(config.allows_all_origins());
}

#[test]
fn test_cors_config_allows_specific_origins() {
    let config = CorsConfig {
        allowed_origins: vec!["https://example.com".to_string()],
        ..CorsConfig::default()
    };
    assert!(!config.allows_all_origins());
}

// ==================== CorsConfig Validation Tests ====================

#[test]
fn test_cors_config_validate_success() {
    let config = CorsConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_cors_config_validate_all_origins_with_credentials() {
    let config = CorsConfig {
        enabled: true,
        allowed_origins: vec!["*".to_string()],
        allow_credentials: true,
        ..CorsConfig::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("credentials"));
}

#[test]
fn test_cors_config_validate_wildcard_with_credentials() {
    let config = CorsConfig {
        enabled: true,
        allowed_origins: vec!["*".to_string()],
        allow_credentials: true,
        ..CorsConfig::default()
    };
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_cors_config_validate_specific_origins_with_credentials() {
    let config = CorsConfig {
        enabled: true,
        allowed_origins: vec!["https://example.com".to_string()],
        allow_credentials: true,
        ..CorsConfig::default()
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_cors_config_validate_disabled() {
    let config = CorsConfig {
        enabled: false,
        allowed_origins: vec![],
        allow_credentials: true,
        ..CorsConfig::default()
    };
    assert!(config.validate().is_ok());
}

// ==================== CorsConfig Merge Tests ====================

#[test]
fn test_cors_config_merge_disabled() {
    let base = CorsConfig::default();
    let other = CorsConfig {
        enabled: false,
        ..CorsConfig::default()
    };
    let merged = base.merge(other);
    assert!(!merged.enabled);
}

#[test]
fn test_cors_config_merge_origins() {
    let base = CorsConfig::default();
    let other = CorsConfig {
        allowed_origins: vec!["https://test.com".to_string()],
        ..CorsConfig::default()
    };
    let merged = base.merge(other);
    assert_eq!(merged.allowed_origins, vec!["https://test.com".to_string()]);
}

#[test]
fn test_cors_config_merge_credentials() {
    let base = CorsConfig::default();
    let other = CorsConfig {
        allowed_origins: vec!["https://example.com".to_string()],
        allow_credentials: true,
        ..CorsConfig::default()
    };
    let merged = base.merge(other);
    assert!(merged.allow_credentials);
}

#[test]
fn test_cors_config_merge_max_age() {
    let base = CorsConfig::default();
    let other = CorsConfig {
        max_age: 7200,
        ..CorsConfig::default()
    };
    let merged = base.merge(other);
    assert_eq!(merged.max_age, 7200);
}

// ==================== CorsConfig Serialization Tests ====================

#[test]
fn test_cors_config_serialization() {
    let config = CorsConfig::default();
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["enabled"], true);
    assert!(json["allowed_methods"].is_array());
    assert_eq!(json["max_age"], 3600);
}

#[test]
fn test_cors_config_deserialization() {
    let json = r#"{
            "enabled": true,
            "allowed_origins": ["https://app.example.com"],
            "allowed_methods": ["GET", "POST"],
            "allowed_headers": ["content-type"],
            "max_age": 1800,
            "allow_credentials": true
        }"#;
    let config: CorsConfig = serde_json::from_str(json).unwrap();
    assert!(config.enabled);
    assert_eq!(config.allowed_origins.len(), 1);
    assert_eq!(config.allowed_methods.len(), 2);
    assert_eq!(config.max_age, 1800);
    assert!(config.allow_credentials);
}

#[test]
fn test_cors_config_clone() {
    let config = CorsConfig::default();
    let cloned = config.clone();
    assert_eq!(config.enabled, cloned.enabled);
    assert_eq!(config.max_age, cloned.max_age);
}

// ==================== trusted_proxies Tests ====================

#[test]
fn test_server_config_trusted_proxies_default_empty() {
    let config = ServerConfig::default();
    assert!(config.trusted_proxies.is_empty());
}

#[test]
fn test_server_config_trusted_proxies_deserialization() {
    let json = r#"{
            "trusted_proxies": ["127.0.0.1", "10.0.0.1"]
        }"#;
    let config: ServerConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.trusted_proxies, vec!["127.0.0.1", "10.0.0.1"]);
}

#[test]
fn test_server_config_trusted_proxies_missing_key_defaults_empty() {
    let json = r#"{"host": "0.0.0.0", "port": 8080, "timeout": 30, "max_body_size": 1048576}"#;
    let config: ServerConfig = serde_json::from_str(json).unwrap();
    assert!(config.trusted_proxies.is_empty());
}

#[test]
fn test_server_config_trusted_proxies_merge_non_empty() {
    let base = ServerConfig::default();
    let other = ServerConfig {
        trusted_proxies: vec!["192.168.1.1".to_string()],
        ..ServerConfig::default()
    };
    let merged = base.merge(other);
    assert_eq!(merged.trusted_proxies, vec!["192.168.1.1"]);
}

#[test]
fn test_server_config_trusted_proxies_merge_empty_keeps_base() {
    let base = ServerConfig {
        trusted_proxies: vec!["10.0.0.1".to_string()],
        ..ServerConfig::default()
    };
    let other = ServerConfig::default(); // trusted_proxies is empty
    let merged = base.merge(other);
    assert_eq!(merged.trusted_proxies, vec!["10.0.0.1"]);
}
