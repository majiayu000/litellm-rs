//! Provider configuration trait
//!
//! Defines the configuration interface for all AI providers

use std::fmt::Debug;

/// Provider configuration trait
///
/// All provider configurations must implement this trait to ensure consistent
/// validation and access to common settings.
///
/// # Design Principles
/// - Type-safe configuration management
/// - Validation before provider initialization
/// - Common settings abstraction (API key, timeout, retries)
/// - Clone and Debug for testability
///
/// # Example
/// ```rust
/// use std::time::Duration;
/// use litellm_rs::core::traits::provider::ProviderConfig;
///
/// #[derive(Debug, Clone)]
/// struct MyProviderConfig {
///     api_key: String,
///     api_base: Option<String>,
///     timeout_secs: u64,
///     max_retries: u32,
/// }
///
/// impl ProviderConfig for MyProviderConfig {
///     fn validate(&self) -> Result<(), String> {
///         if self.api_key.is_empty() {
///             return Err("API key is required".to_string());
///         }
///         Ok(())
///     }
///
///     fn api_key(&self) -> Option<&str> {
///         Some(&self.api_key)
///     }
///
///     fn api_base(&self) -> Option<&str> {
///         self.api_base.as_deref()
///     }
///
///     fn timeout(&self) -> Duration {
///         Duration::from_secs(self.timeout_secs)
///     }
///
///     fn max_retries(&self) -> u32 {
///         self.max_retries
///     }
/// }
/// ```
pub trait ProviderConfig: Send + Sync + Clone + Debug + 'static {
    /// Validate configuration
    ///
    /// # Returns
    /// `Ok(())` if configuration is valid, `Err(String)` with error message otherwise
    ///
    /// # Implementation Notes
    /// - Check required fields are present
    /// - Validate field formats (e.g., URL format)
    /// - Verify value ranges are acceptable
    /// - Called before provider initialization
    fn validate(&self) -> Result<(), String>;

    /// Get API key
    ///
    /// # Returns
    /// Optional API key for authentication with the provider
    ///
    /// # Note
    /// Returns `None` if the provider doesn't require an API key
    fn api_key(&self) -> Option<&str>;

    /// Get API base URL
    ///
    /// # Returns
    /// Optional base URL for the provider API
    ///
    /// # Use Cases
    /// - Custom API endpoints (e.g., Azure OpenAI)
    /// - Self-hosted models
    /// - Proxy configurations
    /// - Development/testing environments
    fn api_base(&self) -> Option<&str>;

    /// Get request timeout
    ///
    /// # Returns
    /// Maximum duration to wait for a request to complete
    ///
    /// # Implementation Notes
    /// - Should include both connection and read timeouts
    /// - Typical values: 30-120 seconds for chat completion
    /// - Consider longer timeouts for streaming requests
    fn timeout(&self) -> std::time::Duration;

    /// Get maximum retry attempts
    ///
    /// # Returns
    /// Number of times to retry failed requests
    ///
    /// # Implementation Notes
    /// - Retries should use exponential backoff
    /// - Typical values: 2-5 retries
    /// - Consider rate limits when setting this value
    fn max_retries(&self) -> u32;

    /// Whether this provider requires an SSRF-safe HTTP client.
    ///
    /// Return `true` for providers whose endpoint URL is user-controlled.
    /// The SSRF-safe client re-validates the resolved IP on every request,
    /// preventing DNS-rebinding attacks.
    fn use_ssrf_safe_client(&self) -> bool {
        false
    }

    /// Standard validation: API key required, timeout > 0, max_retries <= 10.
    ///
    /// Call from `validate()` to avoid repeating common checks.
    /// Providers with optional API keys or custom fields should implement
    /// `validate()` directly instead.
    fn validate_standard(&self, provider_name: &str) -> Result<(), String> {
        if self.api_key().is_none_or(|k| k.is_empty()) {
            return Err(format!("{} API key is required", provider_name));
        }
        if self.timeout().as_secs() == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }
        if self.max_retries() > 10 {
            return Err("Max retries should not exceed 10".to_string());
        }
        Ok(())
    }
}
