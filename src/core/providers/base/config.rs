//! Configuration
//!
//! Configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseConfig {
    /// API key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// API base URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,

    /// Request
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// maximumNumber of retries
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Custom HTTP headers
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Organization ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,

    /// API version (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
}

fn default_timeout() -> u64 {
    60
}

fn default_max_retries() -> u32 {
    3
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base: None,
            timeout: default_timeout(),
            max_retries: default_max_retries(),
            headers: HashMap::new(),
            organization: None,
            api_version: None,
        }
    }
}

impl BaseConfig {
    fn normalize_provider_name(provider: &str) -> String {
        provider.trim().to_lowercase()
    }

    fn provider_env_key(provider: &str, suffix: &str) -> String {
        let normalized_provider = Self::normalize_provider_name(provider);
        format!("{}_{}", normalized_provider.to_uppercase(), suffix)
    }

    fn env_value(provider: &str, suffix: &str) -> Option<String> {
        std::env::var(Self::provider_env_key(provider, suffix)).ok()
    }

    fn catalog_default_base_url(provider: &str) -> Option<String> {
        crate::core::providers::registry::get_definition(provider)
            .map(|definition| definition.base_url.to_string())
    }

    fn legacy_default_base_url(provider: &str) -> &'static str {
        match provider {
            "openai" => "https://api.openai.com/v1",
            "anthropic" => "https://api.anthropic.com",
            "azure" => "https://{instance}.openai.azure.com",
            "mistral" => "https://api.mistral.ai/v1",
            "vertex_ai" => "https://generativelanguage.googleapis.com",
            "ai21" => "https://api.ai21.com/studio/v1",
            "cerebras" => "https://api.cerebras.ai/v1",
            "gigachat" => "https://gigachat.devices.sberbank.ru/api/v1",
            "nlp_cloud" => "https://api.nlpcloud.io/v1",
            "voyage" => "https://api.voyageai.com/v1",
            "github" => "https://models.inference.ai.azure.com",
            "deepgram" => "https://api.deepgram.com/v1",
            "elevenlabs" => "https://api.elevenlabs.io",
            "clarifai" => "https://api.clarifai.com/v2",
            "replicate" => "https://api.replicate.com/v1",
            "huggingface" => "https://api-inference.huggingface.co",
            "cohere" => "https://api.cohere.ai/v1",
            "datarobot" => "https://app.datarobot.com/api/v2",
            "empower" => "https://api.empower.dev/v1",
            "exa_ai" => "https://api.exa.ai/v1",
            "firecrawl" => "https://api.firecrawl.dev/v1",
            "deepl" => "https://api-free.deepl.com/v2",
            "fal_ai" => "https://fal.run",
            _ => "https://api.openai.com/v1",
        }
    }

    fn default_api_version(provider: &str) -> Option<&'static str> {
        match provider {
            "anthropic" => Some("2023-06-01"),
            "azure" => Some("2024-02-01"),
            _ => None,
        }
    }

    /// Configuration
    pub fn from_env(provider: &str) -> Self {
        Self {
            api_key: Self::env_value(provider, "API_KEY"),
            api_base: Self::env_value(provider, "API_BASE"),
            timeout: Self::env_value(provider, "TIMEOUT")
                .and_then(|t| t.parse().ok())
                .unwrap_or(default_timeout()),
            max_retries: Self::env_value(provider, "MAX_RETRIES")
                .and_then(|r| r.parse().ok())
                .unwrap_or(default_max_retries()),
            headers: HashMap::new(),
            organization: Self::env_value(provider, "ORGANIZATION"),
            api_version: Self::env_value(provider, "API_VERSION"),
        }
    }

    /// Default
    pub fn for_provider(provider: &str) -> Self {
        let normalized_provider = Self::normalize_provider_name(provider);
        let mut config = Self::from_env(provider);

        // Default
        if config.api_base.is_none() {
            config.api_base = Some(
                Self::catalog_default_base_url(&normalized_provider).unwrap_or_else(|| {
                    Self::legacy_default_base_url(&normalized_provider).to_string()
                }),
            );
        }

        // Default API version for specific providers
        if config.api_version.is_none()
            && let Some(default_version) = Self::default_api_version(&normalized_provider)
        {
            config.api_version = Some(default_version.to_string());
        }

        config
    }

    fn build_endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.api_base
                .as_deref()
                .unwrap_or_default()
                .trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Configuration
    pub fn validate(&self, provider: &str) -> Result<(), String> {
        // Validation
        if self.api_key.is_none() {
            return Err(format!("{} API key is required", provider));
        }

        // Validation
        if self.timeout == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }

        // Validation
        if self.max_retries > 10 {
            return Err("Max retries should not exceed 10".to_string());
        }

        // Validation
        if let Some(api_key) = &self.api_key {
            match provider {
                "openai" if !api_key.starts_with("sk-") && !api_key.starts_with("proj-") => {
                    return Err("OpenAI API key should start with 'sk-' or 'proj-'".to_string());
                }
                "anthropic" if !api_key.starts_with("sk-ant-") => {
                    return Err("Anthropic API key should start with 'sk-ant-'".to_string());
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Configuration
    pub fn get_effective_api_key(&self, provider: &str) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| Self::env_value(provider, "API_KEY"))
    }

    /// Get
    pub fn get_effective_api_base(&self, provider: &str) -> String {
        self.api_base.clone().unwrap_or_else(|| {
            Self::env_value(provider, "API_BASE")
                .unwrap_or_else(|| Self::for_provider(provider).api_base.unwrap_or_default())
        })
    }

    /// Get
    pub fn get_chat_endpoint(&self) -> String {
        self.build_endpoint("chat/completions")
    }

    /// Get
    pub fn get_embeddings_endpoint(&self) -> String {
        self.build_endpoint("embeddings")
    }

    /// Convert to Duration
    pub fn timeout_duration(&self) -> Duration {
        Duration::from_secs(self.timeout)
    }
}

/// Configuration
#[macro_export]
macro_rules! define_provider_config {
    // Full version: generates struct, Default, new, from_env, builders, ProviderConfig impl
    ($name:ident, provider: $provider:expr) => {
        $crate::define_provider_config!($name { }, provider: $provider);
    };

    ($name:ident { $($field:ident: $type:ty = $default:expr),* $(,)? }, provider: $provider:expr) => {
        $crate::define_provider_config!($name { $($field: $type = $default),* });

        impl $name {
            pub fn from_env() -> Self {
                Self::new($provider)
            }

            pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
                self.base.api_key = Some(api_key.into());
                self
            }

            pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
                self.base.api_base = Some(base_url.into());
                self
            }

            pub fn with_timeout(mut self, timeout: u64) -> Self {
                self.base.timeout = timeout;
                self
            }

            /// Get API key with environment variable fallback
            pub fn get_api_key(&self) -> Option<String> {
                self.base.get_effective_api_key($provider)
            }

            /// Get API base URL with environment variable fallback
            pub fn get_api_base(&self) -> String {
                self.base.get_effective_api_base($provider)
            }
        }

        impl $crate::core::traits::provider::ProviderConfig for $name {
            fn validate(&self) -> Result<(), String> {
                self.base.validate($provider)
            }

            fn api_key(&self) -> Option<&str> {
                self.base.api_key.as_deref()
            }

            fn api_base(&self) -> Option<&str> {
                self.base.api_base.as_deref()
            }

            fn timeout(&self) -> std::time::Duration {
                self.base.timeout_duration()
            }

            fn max_retries(&self) -> u32 {
                self.base.max_retries
            }
        }
    };

    // Env-required version: from_env() returns Result<Self, String> requiring API key
    ($name:ident, env_key: $env_key:expr, provider: $provider:expr) => {
        $crate::define_provider_config!($name { }, env_key: $env_key, provider: $provider);
    };

    ($name:ident { $($field:ident: $type:ty = $default:expr),* $(,)? }, env_key: $env_key:expr, provider: $provider:expr) => {
        $crate::define_provider_config!($name { $($field: $type = $default),* });

        impl $name {
            pub fn from_env() -> Result<Self, String> {
                let api_key = std::env::var($env_key)
                    .map_err(|_| concat!($env_key, " environment variable is required"))?;
                Ok(Self::new($provider).with_api_key(api_key))
            }

            pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
                self.base.api_key = Some(api_key.into());
                self
            }

            pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
                self.base.api_base = Some(base_url.into());
                self
            }

            pub fn with_timeout(mut self, timeout: u64) -> Self {
                self.base.timeout = timeout;
                self
            }

            /// Get API key with environment variable fallback
            pub fn get_api_key(&self) -> Option<String> {
                self.base.get_effective_api_key($provider)
            }

            /// Get API base URL with environment variable fallback
            pub fn get_api_base(&self) -> String {
                self.base.get_effective_api_base($provider)
            }
        }

        impl $crate::core::traits::provider::ProviderConfig for $name {
            fn validate(&self) -> Result<(), String> {
                self.base.validate($provider)
            }

            fn api_key(&self) -> Option<&str> {
                self.base.api_key.as_deref()
            }

            fn api_base(&self) -> Option<&str> {
                self.base.api_base.as_deref()
            }

            fn timeout(&self) -> std::time::Duration {
                self.base.timeout_duration()
            }

            fn max_retries(&self) -> u32 {
                self.base.max_retries
            }
        }
    };

    ($name:ident { $($field:ident: $type:ty = $default:expr),* $(,)? }) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $name {
            #[serde(flatten)]
            pub base: $crate::core::providers::base::config::BaseConfig,
            $(
                #[serde(default)]
                pub $field: $type,
            )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    base: $crate::core::providers::base::config::BaseConfig::default(),
                    $($field: $default),*
                }
            }
        }

        impl $name {
            pub fn from_base(base: $crate::core::providers::base::config::BaseConfig) -> Self {
                Self {
                    base,
                    $($field: $default),*
                }
            }

            pub fn new(provider: &str) -> Self {
                Self::from_base($crate::core::providers::base::config::BaseConfig::for_provider(provider))
            }
        }

        impl AsRef<$crate::core::providers::base::config::BaseConfig> for $name {
            fn as_ref(&self) -> &$crate::core::providers::base::config::BaseConfig {
                &self.base
            }
        }

        impl AsMut<$crate::core::providers::base::config::BaseConfig> for $name {
            fn as_mut(&mut self) -> &mut $crate::core::providers::base::config::BaseConfig {
                &mut self.base
            }
        }
    };

    // Version without additional fields
    ($name:ident) => {
        $crate::define_provider_config!($name {});
    };
}

/// Macro for standalone provider configs that don't use BaseConfig.
///
/// Generates a flat struct with `api_key`, `api_base`, `timeout`, `max_retries`,
/// plus optional extra fields. Also generates:
/// - `Default` impl with provider-specific defaults
/// - `ProviderConfig` trait impl
/// - `from_env()` loading from `{PROVIDER}_API_KEY` etc.
/// - `get_api_key()` / `get_api_base()` with env var fallback
/// - Builder methods: `with_api_key()`, `with_base_url()`, `with_timeout()`
///
/// # Example
/// ```ignore
/// define_standalone_provider_config!(MorphConfig,
///     provider: "morph",
///     env_prefix: "MORPH",
///     default_base_url: "https://api.morph.so/v1",
///     default_timeout: 60,
/// );
///
/// // With extra fields:
/// define_standalone_provider_config!(BasetenConfig,
///     provider: "baseten",
///     env_prefix: "BASETEN",
///     default_base_url: "https://inference.baseten.co/v1",
///     default_timeout: 30,
///     extra_fields: { debug: bool = false },
/// );
/// ```
#[macro_export]
macro_rules! define_standalone_provider_config {
    // Version with extra fields
    ($name:ident,
     provider: $provider:expr,
     env_prefix: $env_prefix:expr,
     default_base_url: $default_base_url:expr,
     default_timeout: $default_timeout:expr,
     extra_fields: { $($field:ident: $type:ty = $default:expr),* $(,)? } $(,)?
    ) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $name {
            /// API key for authentication
            pub api_key: Option<String>,
            /// API base URL
            pub api_base: Option<String>,
            /// Request timeout in seconds
            #[serde(default)]
            pub timeout: u64,
            /// Maximum number of retries
            #[serde(default)]
            pub max_retries: u32,
            $(
                #[serde(default)]
                pub $field: $type,
            )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    api_key: None,
                    api_base: None,
                    timeout: $default_timeout,
                    max_retries: 3,
                    $($field: $default,)*
                }
            }
        }

        impl $crate::core::traits::provider::ProviderConfig for $name {
            fn validate(&self) -> Result<(), String> {
                if self.api_key.is_none()
                    && std::env::var(concat!($env_prefix, "_API_KEY")).is_err()
                {
                    return Err(format!(
                        "{} API key not provided and {}_API_KEY environment variable not set",
                        $provider, $env_prefix
                    ));
                }
                if self.timeout == 0 {
                    return Err("Timeout must be greater than 0".to_string());
                }
                Ok(())
            }

            fn api_key(&self) -> Option<&str> {
                self.api_key.as_deref()
            }

            fn api_base(&self) -> Option<&str> {
                self.api_base.as_deref()
            }

            fn timeout(&self) -> std::time::Duration {
                std::time::Duration::from_secs(self.timeout)
            }

            fn max_retries(&self) -> u32 {
                self.max_retries
            }
        }

        impl $name {
            /// Create config from environment variables
            pub fn from_env() -> Self {
                Self {
                    api_key: std::env::var(concat!($env_prefix, "_API_KEY")).ok(),
                    api_base: std::env::var(concat!($env_prefix, "_API_BASE")).ok(),
                    timeout: std::env::var(concat!($env_prefix, "_TIMEOUT"))
                        .ok()
                        .and_then(|t| t.parse().ok())
                        .unwrap_or($default_timeout),
                    max_retries: std::env::var(concat!($env_prefix, "_MAX_RETRIES"))
                        .ok()
                        .and_then(|r| r.parse().ok())
                        .unwrap_or(3),
                    $($field: $default,)*
                }
            }

            /// Create a new configuration with API key
            pub fn new(api_key: impl Into<String>) -> Self {
                Self {
                    api_key: Some(api_key.into()),
                    ..Default::default()
                }
            }

            /// Get API key with environment variable fallback
            pub fn get_api_key(&self) -> Option<String> {
                self.api_key
                    .clone()
                    .or_else(|| std::env::var(concat!($env_prefix, "_API_KEY")).ok())
            }

            /// Get API base with environment variable fallback
            pub fn get_api_base(&self) -> String {
                self.api_base
                    .clone()
                    .or_else(|| std::env::var(concat!($env_prefix, "_API_BASE")).ok())
                    .unwrap_or_else(|| $default_base_url.to_string())
            }

            /// Set the API key
            pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
                self.api_key = Some(api_key.into());
                self
            }

            /// Set the API base URL
            pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
                self.api_base = Some(base_url.into());
                self
            }

            /// Set the timeout in seconds
            pub fn with_timeout(mut self, timeout: u64) -> Self {
                self.timeout = timeout;
                self
            }

            /// Set the maximum number of retries
            pub fn with_max_retries(mut self, max_retries: u32) -> Self {
                self.max_retries = max_retries;
                self
            }
        }
    };

    // Version without extra fields
    ($name:ident,
     provider: $provider:expr,
     env_prefix: $env_prefix:expr,
     default_base_url: $default_base_url:expr,
     default_timeout: $default_timeout:expr $(,)?
    ) => {
        $crate::define_standalone_provider_config!($name,
            provider: $provider,
            env_prefix: $env_prefix,
            default_base_url: $default_base_url,
            default_timeout: $default_timeout,
            extra_fields: {},
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_config_default() {
        let config = BaseConfig::default();
        assert_eq!(config.timeout, 60);
        assert_eq!(config.max_retries, 3);
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_provider_specific_defaults() {
        let openai = BaseConfig::for_provider("openai");
        assert_eq!(
            openai.api_base,
            Some("https://api.openai.com/v1".to_string())
        );

        let anthropic = BaseConfig::for_provider("anthropic");
        assert_eq!(anthropic.api_version, Some("2023-06-01".to_string()));
    }

    #[test]
    fn test_catalog_provider_defaults_are_used() {
        let anyscale = BaseConfig::for_provider("anyscale");
        assert_eq!(
            anyscale.api_base,
            Some("https://api.endpoints.anyscale.com/v1".to_string())
        );

        let aleph_alpha = BaseConfig::for_provider("aleph_alpha");
        assert_eq!(
            aleph_alpha.api_base,
            Some("https://api.aleph-alpha.com/v1".to_string())
        );
    }

    #[test]
    fn test_legacy_unknown_provider_fallback_default() {
        let unknown = BaseConfig::for_provider("legacy_unknown");
        assert_eq!(
            unknown.api_base,
            Some("https://api.openai.com/v1".to_string())
        );
    }

    #[test]
    fn test_provider_name_normalization_for_defaults() {
        let mixed_case = BaseConfig::for_provider(" OpenAI ");
        assert_eq!(
            mixed_case.api_base,
            Some("https://api.openai.com/v1".to_string())
        );

        let tier1_mixed_case = BaseConfig::for_provider(" Anyscale ");
        assert_eq!(
            tier1_mixed_case.api_base,
            Some("https://api.endpoints.anyscale.com/v1".to_string())
        );
    }

    #[test]
    fn test_default_api_version_assignment() {
        let anthropic = BaseConfig::for_provider("anthropic");
        assert_eq!(anthropic.api_version, Some("2023-06-01".to_string()));

        let azure = BaseConfig::for_provider("azure");
        assert_eq!(azure.api_version, Some("2024-02-01".to_string()));

        let openai = BaseConfig::for_provider("openai");
        assert!(openai.api_version.is_none());
    }

    #[test]
    fn test_endpoint_building_trims_slashes() {
        let config = BaseConfig {
            api_base: Some("https://api.example.com/v1/".to_string()),
            ..Default::default()
        };

        assert_eq!(
            config.get_chat_endpoint(),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            config.get_embeddings_endpoint(),
            "https://api.example.com/v1/embeddings"
        );
    }

    #[test]
    fn test_provider_env_key_builder() {
        assert_eq!(
            BaseConfig::provider_env_key("openai", "API_KEY"),
            "OPENAI_API_KEY"
        );
        assert_eq!(
            BaseConfig::provider_env_key("mixed_case", "TIMEOUT"),
            "MIXED_CASE_TIMEOUT"
        );
        assert_eq!(
            BaseConfig::provider_env_key(" OpenAI ", "ORGANIZATION"),
            "OPENAI_ORGANIZATION"
        );
    }

    #[test]
    fn test_validation() {
        let mut config = BaseConfig::for_provider("openai");

        // Missing API key
        assert!(config.validate("openai").is_err());

        // Add valid API key
        config.api_key = Some("sk-test123".to_string());
        assert!(config.validate("openai").is_ok());

        // Invalid API key format
        config.api_key = Some("invalid-key".to_string());
        assert!(config.validate("openai").is_err());
    }
}
