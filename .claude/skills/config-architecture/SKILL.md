---
name: config-architecture
description: LiteLLM-RS Configuration Architecture. Covers YAML loading, environment variable override, validation patterns, type-safe config models, and hot reloading. Use when changing config models or serde defaults, adding or debugging env var overrides, updating validation rules, altering config loading or hot reload behavior, or troubleshooting gateway.yaml.
---

# Configuration Architecture Guide

## Overview

LiteLLM-RS uses a layered configuration system with YAML files as the base, environment variable overrides, and type-safe Rust models with validation.

### Configuration Priority

```
┌─────────────────────────────────────────────────────────────────┐
│                    Runtime Overrides                            │
│  (API calls, hot reload)                                       │
│  Priority: Highest                                              │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                  Environment Variables                          │
│  (${VAR} syntax in YAML)                                       │
│  Priority: High                                                 │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                    Config Files                                 │
│  (config/gateway.yaml)                                         │
│  Priority: Medium                                               │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                    Default Values                               │
│  (Rust Default trait)                                          │
│  Priority: Lowest                                               │
└─────────────────────────────────────────────────────────────────┘
```

---

## YAML Configuration Structure

### Main Configuration File

```yaml
# config/gateway.yaml

# Server configuration
server:
  host: "0.0.0.0"
  port: 8080
  workers: 0  # 0 = auto-detect CPU cores
  keep_alive: 75
  request_timeout: 300
  max_request_size: "10MB"

# Logging configuration
logging:
  level: "info"  # trace, debug, info, warn, error
  format: "json"  # json, pretty
  include_timestamp: true
  include_target: true

# Authentication
auth:
  enabled: true
  jwt:
    secret: ${JWT_SECRET}
    issuer: "litellm-gateway"
    audience: "litellm-api"
    expiry_seconds: 3600
  api_key:
    enabled: true
    header_name: "Authorization"
    prefix: "Bearer "
  rate_limiting:
    enabled: true
    requests_per_minute: 60
    tokens_per_minute: 100000

# Provider configurations
providers:
  openai:
    enabled: true
    api_key: ${OPENAI_API_KEY}
    api_base: "https://api.openai.com/v1"
    default_model: "gpt-4o"
    timeout: 120
    max_retries: 3

  anthropic:
    enabled: true
    api_key: ${ANTHROPIC_API_KEY}
    api_base: "https://api.anthropic.com"
    default_model: "claude-3-5-sonnet-20241022"
    timeout: 120

  azure:
    enabled: true
    api_key: ${AZURE_API_KEY}
    api_base: ${AZURE_API_BASE}
    api_version: "2024-02-01"
    deployment_map:
      gpt-4o: "gpt-4o-deployment"
      gpt-4o-mini: "gpt-4o-mini-deployment"

  google:
    enabled: true
    api_key: ${GOOGLE_API_KEY}
    project_id: ${GOOGLE_PROJECT_ID}
    location: "us-central1"

# Routing configuration
routing:
  strategy: "latency_based"
  fallback_enabled: true
  max_retries: 3
  retry_delay_ms: 1000
  health_check_interval_seconds: 30

# Caching configuration
cache:
  enabled: true
  redis:
    url: ${REDIS_URL}
    prefix: "litellm"
    ttl_seconds: 3600
  semantic:
    enabled: false
    provider: "qdrant"
    url: ${QDRANT_URL}
    similarity_threshold: 0.95

# Observability
observability:
  metrics:
    enabled: true
    endpoint: "/metrics"
    include_labels: ["provider", "model", "status"]
  tracing:
    enabled: true
    exporter: "otlp"
    endpoint: ${OTEL_EXPORTER_OTLP_ENDPOINT}
    sample_rate: 0.1
  health:
    enabled: true
    endpoint: "/health"
```

---

## Type-Safe Configuration Models

### Root Configuration

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub logging: LoggingConfig,

    #[serde(default)]
    pub auth: AuthConfig,

    #[serde(default)]
    pub providers: ProvidersConfig,

    #[serde(default)]
    pub routing: RoutingConfig,

    #[serde(default)]
    pub cache: CacheConfig,

    #[serde(default)]
    pub observability: ObservabilityConfig,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            logging: LoggingConfig::default(),
            auth: AuthConfig::default(),
            providers: ProvidersConfig::default(),
            routing: RoutingConfig::default(),
            cache: CacheConfig::default(),
            observability: ObservabilityConfig::default(),
        }
    }
}
```

### Server Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default)]
    pub workers: usize,  // 0 = auto-detect

    #[serde(default = "default_keep_alive")]
    pub keep_alive: u64,

    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,

    #[serde(default = "default_max_request_size")]
    pub max_request_size: String,
}

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_keep_alive() -> u64 { 75 }
fn default_request_timeout() -> u64 { 300 }
fn default_max_request_size() -> String { "10MB".to_string() }

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            workers: 0,
            keep_alive: default_keep_alive(),
            request_timeout: default_request_timeout(),
            max_request_size: default_max_request_size(),
        }
    }
}
```

### Provider Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    pub api_key: Option<String>,

    pub api_base: Option<String>,

    pub default_model: Option<String>,

    #[serde(default = "default_timeout")]
    pub timeout: u64,

    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    #[serde(default)]
    pub headers: HashMap<String, String>,

    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_true() -> bool { true }
fn default_timeout() -> u64 { 120 }
fn default_max_retries() -> u32 { 3 }
```

---

## Environment Variable Override

### Variable Substitution

```rust
use regex::Regex;
use std::env;

pub struct EnvSubstitutor {
    pattern: Regex,
}

impl EnvSubstitutor {
    pub fn new() -> Self {
        Self {
            // Matches ${VAR_NAME} or ${VAR_NAME:-default}
            pattern: Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)(?::-([^}]*))?\}").unwrap(),
        }
    }

    pub fn substitute(&self, content: &str) -> Result<String, ConfigError> {
        let mut result = content.to_string();
        let mut errors = Vec::new();

        for cap in self.pattern.captures_iter(content) {
            let full_match = cap.get(0).unwrap().as_str();
            let var_name = cap.get(1).unwrap().as_str();
            let default_value = cap.get(2).map(|m| m.as_str());

            let value = match env::var(var_name) {
                Ok(v) => v,
                Err(_) => {
                    if let Some(default) = default_value {
                        default.to_string()
                    } else {
                        errors.push(var_name.to_string());
                        continue;
                    }
                }
            };

            result = result.replace(full_match, &value);
        }

        if !errors.is_empty() {
            return Err(ConfigError::MissingEnvVars(errors));
        }

        Ok(result)
    }
}
```

### Usage Example

```yaml
# config/gateway.yaml
providers:
  openai:
    api_key: ${OPENAI_API_KEY}                    # Required
    api_base: ${OPENAI_API_BASE:-https://api.openai.com/v1}  # With default
    timeout: ${OPENAI_TIMEOUT:-120}               # Numeric with default
```

---

## Configuration Loading

### Config Loader

```rust
use std::path::Path;

pub struct ConfigLoader {
    env_substitutor: EnvSubstitutor,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            env_substitutor: EnvSubstitutor::new(),
        }
    }

    /// Load configuration from file path
    pub fn load_from_file<P: AsRef<Path>>(&self, path: P) -> Result<GatewayConfig, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ConfigError::FileRead(e.to_string()))?;

        self.load_from_string(&content)
    }

    /// Load configuration from string
    pub fn load_from_string(&self, content: &str) -> Result<GatewayConfig, ConfigError> {
        // Substitute environment variables
        let substituted = self.env_substitutor.substitute(content)?;

        // Parse YAML
        let config: GatewayConfig = serde_yaml::from_str(&substituted)
            .map_err(|e| ConfigError::Parse(e.to_string()))?;

        // Validate
        config.validate()?;

        Ok(config)
    }

    /// Load from default locations
    pub fn load_default(&self) -> Result<GatewayConfig, ConfigError> {
        let paths = [
            "config/gateway.yaml",
            "gateway.yaml",
            "/etc/litellm/gateway.yaml",
        ];

        for path in paths {
            if Path::new(path).exists() {
                return self.load_from_file(path);
            }
        }

        // Return default config if no file found
        Ok(GatewayConfig::default())
    }
}
```

## References

- [reference/validation-hot-reload.md](reference/validation-hot-reload.md) — Validation trait implementations and file-watch based hot reloading.
- [reference/provider-config-errors.md](reference/provider-config-errors.md) — Provider configuration trait and ConfigError variants.
- [reference/best-practices.md](reference/best-practices.md) — Defaults, early validation, secret handling, type safety, and documentation practices.
