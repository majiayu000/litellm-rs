//! Provider Definition - data-driven provider configuration
//!
//! Tier 1 providers are pure OpenAI-compatible endpoints that differ only in
//! base_url, auth, and supported models. Instead of maintaining separate
//! implementations, they are defined as static data entries.

/// How the provider authenticates requests
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    /// `Authorization: Bearer <key>`
    Bearer,
    /// `x-api-key: <key>` (or similar header-based key)
    ApiKeyHeader(&'static str),
    /// No authentication required (local endpoints)
    None,
}

/// Static definition of an OpenAI-compatible provider.
///
/// Each Tier 1 provider is fully described by one of these entries.
/// At runtime the gateway creates an `OpenAILikeProvider` from this data.
#[derive(Debug, Clone)]
pub struct ProviderDefinition {
    /// Internal identifier, e.g. "groq"
    pub name: &'static str,
    /// Human-readable name, e.g. "Groq"
    pub display_name: &'static str,
    /// Default API base URL
    pub base_url: &'static str,
    /// Environment variable that holds the API key
    pub auth_env_var: &'static str,
    /// Compatibility environment variables checked after `auth_env_var`
    pub alternate_auth_env_vars: &'static [&'static str],
    /// Authentication method
    pub auth_type: AuthType,
    /// Whether API key can be skipped (local providers)
    pub skip_api_key: bool,
    /// Model name prefix to strip, if any (e.g. "groq/")
    pub model_prefix: Option<&'static str>,
}

impl ProviderDefinition {
    /// Build an `OpenAILikeConfig` from this definition and an optional API key.
    pub fn to_openai_like_config(
        &self,
        api_key: Option<&str>,
        base_url_override: Option<&str>,
    ) -> crate::core::providers::openai_like::OpenAILikeConfig {
        use crate::core::providers::openai_like::OpenAILikeConfig;

        let effective_base = base_url_override.unwrap_or(self.base_url);

        let mut config = if let Some(key) = api_key {
            OpenAILikeConfig::with_api_key(effective_base, key)
        } else {
            OpenAILikeConfig::new(effective_base)
        };

        config.provider_name = self.name.to_string();
        config.skip_api_key = self.skip_api_key;

        if let Some(prefix) = self.model_prefix {
            config.model_prefix = Some(prefix.to_string());
        }

        // Set auth-specific custom headers
        match self.auth_type {
            AuthType::ApiKeyHeader(header_name) => {
                if let Some(key) = api_key {
                    config
                        .custom_headers
                        .insert(header_name.to_string(), key.to_string());
                    // Clear the base api_key so Bearer isn't also sent
                    config.base.api_key = None;
                }
            }
            AuthType::None => {
                config.skip_api_key = true;
            }
            AuthType::Bearer => {
                // Default behavior - api_key in base config is used as Bearer
            }
        }

        config
    }

    /// Resolve the API key: explicit value > environment variable
    pub fn resolve_api_key(&self, explicit: Option<&str>) -> Option<String> {
        explicit
            .map(|s| s.to_string())
            .or_else(|| std::env::var(self.auth_env_var).ok())
            .or_else(|| {
                self.alternate_auth_env_vars
                    .iter()
                    .find_map(|env_var| std::env::var(env_var).ok())
            })
    }
}
