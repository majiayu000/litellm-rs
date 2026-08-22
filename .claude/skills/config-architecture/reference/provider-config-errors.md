## Provider Configuration Trait

```rust
pub trait ProviderConfig: Clone + Default {
    /// Validate the configuration
    fn validate(&self) -> Result<(), String>;

    /// Get API key
    fn get_api_key(&self) -> Option<String>;

    /// Get API base URL
    fn get_api_base(&self) -> String;

    /// Get timeout in seconds
    fn get_timeout(&self) -> Duration {
        Duration::from_secs(120)
    }

    /// Get max retries
    fn get_max_retries(&self) -> u32 {
        3
    }

    /// Get custom headers
    fn get_headers(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}
```

---

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    FileRead(String),

    #[error("Failed to parse config: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Missing environment variables: {0:?}")]
    MissingEnvVars(Vec<String>),

    #[error("Watch error: {0}")]
    Watch(String),
}
```
