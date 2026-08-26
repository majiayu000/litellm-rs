## Provider Configuration Trait

Two distinct things share the `ProviderConfig` name:

1. The YAML-facing struct `crate::config::models::provider::ProviderConfig` (see SKILL.md).
2. The trait below, implemented by provider runtime configs — `src/core/traits/provider/config.rs`.

```rust
pub trait ProviderConfig: Send + Sync + Clone + Debug + 'static {
    /// Validate configuration; called before provider initialization
    fn validate(&self) -> Result<(), String>;

    /// API key for authentication; None if the provider needs no key
    fn api_key(&self) -> Option<&str>;

    /// Base URL override; None uses the provider default endpoint
    fn api_base(&self) -> Option<&str>;

    /// Request timeout (connection + read)
    fn timeout(&self) -> std::time::Duration;

    /// Number of times to retry failed requests
    fn max_retries(&self) -> u32;

    /// Network scope allowed for this provider's configured endpoint
    fn endpoint_access(&self) -> ProviderEndpointAccess {
        ProviderEndpointAccess::PublicOnly
    }

    /// True when the endpoint URL is user-controlled and the SSRF-safe
    /// client (re-validating the resolved IP per request) is required
    fn use_ssrf_safe_client(&self) -> bool {
        false
    }

    /// Shared checks: key present, timeout > 0, max_retries <= 10.
    /// Call from validate() unless the provider has optional keys or
    /// custom requirements.
    fn validate_standard(&self, provider_name: &str) -> Result<(), String> { ... }
}
```

Method names carry no `get_` prefix, and there is no `get_headers` method. The trait does not require `Default`. A typical implementation delegates to `validate_standard` from `validate()`.

---

## Error Types

`src/core/types/errors/config.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid value for field '{field}': {value}")]
    InvalidValue { field: String, value: String },

    #[error("Configuration file not found: {path}")]
    FileNotFound { path: String },

    #[error("Failed to read configuration file: {path}")]
    ReadError { path: String },

    #[error("Failed to parse configuration: {reason}")]
    ParseError { reason: String },

    #[error("Unsupported configuration format")]
    UnsupportedFormat,

    #[error("Configuration validation failed: {reason}")]
    ValidationFailed { reason: String },

    #[error("Environment variable error: {var}")]
    EnvVarError { var: String },
}

pub type ConfigResult<T> = Result<T, ConfigError>;
```

Usage notes:

- This enum is **not** what the gateway's YAML loader returns. Loading, substitution, parsing, and validation failures all surface as `GatewayError::Config(String)` (`src/utils/error/gateway_error/types.rs`).
- `ConfigError` is used by narrower paths — notably `std::str::FromStr for ProviderType` (`src/core/providers/provider_type.rs`), which returns `ConfigError::InvalidValue` for unrecognized provider-type strings.
