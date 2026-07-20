//! Provider configuration

use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

use crate::core::net::ProviderEndpointAccess;

/// A single entry in a provider's `models` list.
///
/// Two forms are accepted. A plain string means the user-facing model group and
/// the upstream model name are identical. The mapped form lets one provider serve
/// a shared group under its own upstream name, which is what makes it possible for
/// Vercel to answer `apex-verify` with `xai/grok-4.3` while OpenRouter answers the
/// same group with `x-ai/grok-4.3`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProviderModelEntry {
    /// `- "anthropic/claude-opus-4.7"`
    Plain(String),
    /// `- { group: "apex-verify", upstream: "x-ai/grok-4.3" }`
    Mapped {
        /// User-facing model group used for routing lookups.
        group: String,
        /// Model name actually sent to this provider.
        upstream: String,
    },
}

impl ProviderModelEntry {
    /// User-facing model group this entry serves.
    pub fn group(&self) -> &str {
        match self {
            Self::Plain(name) => name,
            Self::Mapped { group, .. } => group,
        }
    }

    /// Model name to send upstream to the provider.
    pub fn upstream(&self) -> &str {
        match self {
            Self::Plain(name) => name,
            Self::Mapped { upstream, .. } => upstream,
        }
    }
}

impl From<String> for ProviderModelEntry {
    fn from(name: String) -> Self {
        Self::Plain(name)
    }
}

impl From<&str> for ProviderModelEntry {
    fn from(name: &str) -> Self {
        Self::Plain(name.to_string())
    }
}

impl std::fmt::Display for ProviderModelEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.group())
    }
}

/// Entries compare against the user-facing group, so existing `models.iter().any(|m| m == requested)`
/// checks keep matching on what the caller actually asked for.
impl PartialEq<str> for ProviderModelEntry {
    fn eq(&self, other: &str) -> bool {
        self.group() == other
    }
}

impl PartialEq<&str> for ProviderModelEntry {
    fn eq(&self, other: &&str) -> bool {
        self.group() == *other
    }
}

impl PartialEq<String> for ProviderModelEntry {
    fn eq(&self, other: &String) -> bool {
        self.group() == other.as_str()
    }
}

/// Provider configuration
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    /// Provider name
    pub name: String,
    /// Provider type (openai, anthropic, etc.)
    pub provider_type: String,
    /// API key
    pub api_key: String,
    /// Base URL
    pub base_url: Option<String>,
    /// Network access requested for this provider endpoint.
    #[serde(default)]
    pub endpoint_access: ProviderEndpointAccess,
    /// API version
    pub api_version: Option<String>,
    /// Organization ID
    pub organization: Option<String>,
    /// Project ID
    pub project: Option<String>,
    /// Provider weight for load balancing
    #[serde(default = "default_weight")]
    pub weight: f32,
    /// Maximum requests per minute
    #[serde(default = "default_rpm")]
    pub rpm: u32,
    /// Maximum tokens per minute
    #[serde(default = "default_tpm")]
    pub tpm: u32,
    /// Maximum concurrent requests
    #[serde(default = "default_max_connections")]
    pub max_concurrent_requests: u32,
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// Maximum retries
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Retry configuration
    #[serde(default)]
    pub retry: RetryConfig,
    /// Health check configuration
    #[serde(default)]
    pub health_check: ProviderHealthCheckConfig,
    /// Provider-specific settings
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,
    /// Supported models
    #[serde(default)]
    pub models: Vec<ProviderModelEntry>,
    /// Tags for grouping providers
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether provider is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl std::fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("name", &self.name)
            .field("provider_type", &self.provider_type)
            .field("api_key", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("endpoint_access", &self.endpoint_access)
            .field("api_version", &self.api_version)
            .field("organization", &self.organization)
            .field("project", &self.project)
            .field("weight", &self.weight)
            .field("rpm", &self.rpm)
            .field("tpm", &self.tpm)
            .field("max_concurrent_requests", &self.max_concurrent_requests)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("retry", &self.retry)
            .field("health_check", &self.health_check)
            .field("settings", &self.settings)
            .field("models", &self.models)
            .field("tags", &self.tags)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            provider_type: String::new(),
            api_key: String::new(),
            base_url: None,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            api_version: None,
            organization: None,
            project: None,
            weight: default_weight(),
            rpm: default_rpm(),
            tpm: default_tpm(),
            max_concurrent_requests: default_max_connections(),
            timeout: default_timeout(),
            max_retries: default_max_retries(),
            retry: RetryConfig::default(),
            health_check: ProviderHealthCheckConfig::default(),
            settings: HashMap::new(),
            models: Vec::new(),
            tags: Vec::new(),
            enabled: true,
        }
    }
}

impl ProviderConfig {
    /// User-facing model groups this provider serves.
    pub fn model_groups(&self) -> Vec<String> {
        self.models
            .iter()
            .map(|entry| entry.group().to_string())
            .collect()
    }

    /// Upstream model names this provider is addressed with.
    pub fn upstream_models(&self) -> Vec<String> {
        self.models
            .iter()
            .map(|entry| entry.upstream().to_string())
            .collect()
    }

    pub(crate) fn configured_endpoint(&self) -> Option<&str> {
        let provider_selector = if self.provider_type.trim().is_empty() {
            self.name.as_str()
        } else {
            self.provider_type.as_str()
        };
        self.base_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .or_else(|| {
                crate::core::providers::factory::endpoint_keys_for_selector(provider_selector)
                    .iter()
                    .find_map(|key| {
                        self.settings
                            .get(*key)
                            .and_then(serde_json::Value::as_str)
                            .filter(|url| !url.trim().is_empty())
                    })
            })
    }

    /// Resolve a configured custom health endpoint to an absolute URL.
    pub(crate) fn resolved_health_check_endpoint(&self) -> Result<Option<Url>, String> {
        let Some(endpoint) = self.health_check.endpoint.as_deref() else {
            return Ok(None);
        };
        let endpoint = endpoint.trim();
        if endpoint.is_empty() {
            return Err(format!(
                "Provider {} health check endpoint cannot be empty",
                self.name
            ));
        }

        match Url::parse(endpoint) {
            Ok(url) => Ok(Some(url)),
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                let base_url = self.configured_endpoint().ok_or_else(|| {
                    format!(
                        "Provider {} relative health check endpoint requires a configured endpoint",
                        self.name
                    )
                })?;
                let mut base = Url::parse(base_url).map_err(|error| {
                    format!(
                        "Provider {} base_url cannot resolve health check endpoint: {}",
                        self.name, error
                    )
                })?;
                if !base.path().ends_with('/') {
                    base.path_segments_mut()
                        .map_err(|()| {
                            format!(
                                "Provider {} base_url cannot resolve a relative health check endpoint",
                                self.name
                            )
                        })?
                        .push("");
                }
                base.join(endpoint).map(Some).map_err(|error| {
                    format!(
                        "Provider {} has invalid health check endpoint: {}",
                        self.name, error
                    )
                })
            }
            Err(error) => Err(format!(
                "Provider {} has invalid health check endpoint: {}",
                self.name, error
            )),
        }
    }
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryConfig {
    /// Base delay in milliseconds
    #[serde(default = "default_base_delay")]
    pub base_delay: u64,
    /// Maximum delay in milliseconds
    #[serde(default = "default_max_delay")]
    pub max_delay: u64,
    /// Backoff multiplier
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// Jitter factor (0.0 to 1.0)
    #[serde(default = "default_retry_jitter")]
    pub jitter: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            base_delay: default_base_delay(),
            max_delay: default_max_delay(),
            backoff_multiplier: default_backoff_multiplier(),
            jitter: default_retry_jitter(),
        }
    }
}

/// Health check configuration for provider-level health monitoring
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderHealthCheckConfig {
    /// Health check interval in seconds
    #[serde(default = "default_health_check_interval")]
    pub interval: u64,
    /// Failure threshold before marking unhealthy
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    /// Recovery timeout in seconds
    #[serde(default = "default_recovery_timeout")]
    pub recovery_timeout: u64,
    /// Health check endpoint path
    pub endpoint: Option<String>,
    /// Expected status codes for healthy response
    #[serde(default = "default_health_expected_codes")]
    pub expected_codes: Vec<u16>,
}

impl Default for ProviderHealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: default_health_check_interval(),
            failure_threshold: default_failure_threshold(),
            recovery_timeout: default_recovery_timeout(),
            endpoint: None,
            expected_codes: default_health_expected_codes(),
        }
    }
}

impl ProviderHealthCheckConfig {
    pub(crate) fn has_runtime_overrides(&self) -> bool {
        self != &Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== RetryConfig Tests ====================

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.base_delay, 100);
        assert_eq!(config.max_delay, 5000);
        assert!((config.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert!((config.jitter - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_retry_config_structure() {
        let config = RetryConfig {
            base_delay: 500,
            max_delay: 30000,
            backoff_multiplier: 1.5,
            jitter: 0.2,
        };
        assert_eq!(config.base_delay, 500);
        assert_eq!(config.max_delay, 30000);
    }

    #[test]
    fn test_retry_config_serialization() {
        let config = RetryConfig {
            base_delay: 2000,
            max_delay: 120000,
            backoff_multiplier: 3.0,
            jitter: 0.5,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["base_delay"], 2000);
        assert_eq!(json["max_delay"], 120000);
    }

    #[test]
    fn test_retry_config_deserialization() {
        let json =
            r#"{"base_delay": 1500, "max_delay": 45000, "backoff_multiplier": 2.5, "jitter": 0.3}"#;
        let config: RetryConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.base_delay, 1500);
        assert!((config.backoff_multiplier - 2.5).abs() < f64::EPSILON);
        assert!((config.jitter - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_partial_yaml_retry_config_preserves_jitter_default() {
        let config: RetryConfig = serde_yml::from_str("base_delay: 250\nmax_delay: 900\n")
            .expect("partial retry YAML should deserialize");

        assert_eq!(config.base_delay, 250);
        assert_eq!(config.max_delay, 900);
        assert!((config.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert!((config.jitter - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_retry_config_clone() {
        let config = RetryConfig::default();
        let cloned = config.clone();
        assert_eq!(config.base_delay, cloned.base_delay);
        assert_eq!(config.max_delay, cloned.max_delay);
    }

    // ==================== ProviderHealthCheckConfig Tests ====================

    #[test]
    fn test_health_check_config_default() {
        let config = ProviderHealthCheckConfig::default();
        assert_eq!(config.interval, 30);
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.recovery_timeout, 60);
        assert!(config.endpoint.is_none());
        assert_eq!(config.expected_codes, vec![200]);
    }

    #[test]
    fn test_health_check_config_structure() {
        let config = ProviderHealthCheckConfig {
            interval: 60,
            failure_threshold: 5,
            recovery_timeout: 120,
            endpoint: Some("/health".to_string()),
            expected_codes: vec![200, 201],
        };
        assert_eq!(config.interval, 60);
        assert_eq!(config.endpoint, Some("/health".to_string()));
        assert_eq!(config.expected_codes.len(), 2);
    }

    #[test]
    fn test_health_check_config_serialization() {
        let config = ProviderHealthCheckConfig {
            interval: 45,
            failure_threshold: 4,
            recovery_timeout: 90,
            endpoint: Some("/api/health".to_string()),
            expected_codes: vec![200],
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["interval"], 45);
        assert_eq!(json["endpoint"], "/api/health");
    }

    #[test]
    fn test_health_check_config_deserialization() {
        let json = r#"{"interval": 20, "failure_threshold": 2, "recovery_timeout": 30, "expected_codes": [200, 204]}"#;
        let config: ProviderHealthCheckConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.interval, 20);
        assert_eq!(config.failure_threshold, 2);
    }

    #[test]
    fn test_partial_health_check_yaml_preserves_expected_code_default() {
        let config: ProviderHealthCheckConfig =
            serde_yml::from_str("interval: 15\nfailure_threshold: 2\n")
                .expect("partial health check YAML should deserialize");

        assert_eq!(config.interval, 15);
        assert_eq!(config.failure_threshold, 2);
        assert_eq!(config.expected_codes, vec![200]);
    }

    #[test]
    fn test_health_check_config_clone() {
        let config = ProviderHealthCheckConfig::default();
        let cloned = config.clone();
        assert_eq!(config.interval, cloned.interval);
    }

    // ==================== ProviderConfig Tests ====================

    #[test]
    fn test_provider_config_default() {
        let config = ProviderConfig::default();
        assert!(config.name.is_empty());
        assert!(config.provider_type.is_empty());
        assert!(config.api_key.is_empty());
        assert!(config.base_url.is_none());
        assert_eq!(config.endpoint_access, ProviderEndpointAccess::PublicOnly);
        assert!((config.weight - 1.0).abs() < f32::EPSILON);
        assert_eq!(config.rpm, 1000);
        assert!(config.enabled);
    }

    #[test]
    fn test_provider_config_structure() {
        let config = ProviderConfig {
            name: "openai-main".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-xxx".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            api_version: Some("2024-01".to_string()),
            organization: Some("org-123".to_string()),
            project: None,
            weight: 2.0,
            rpm: 100,
            tpm: 100000,
            max_concurrent_requests: 20,
            timeout: 60,
            max_retries: 5,
            retry: RetryConfig::default(),
            health_check: ProviderHealthCheckConfig::default(),
            settings: HashMap::new(),
            models: vec!["gpt-4".into()],
            tags: vec!["production".to_string()],
            enabled: true,
        };
        assert_eq!(config.name, "openai-main");
        assert_eq!(config.provider_type, "openai");
        assert_eq!(config.models.len(), 1);
    }

    #[test]
    fn name_only_provider_uses_effective_selector_for_endpoint_aliases() {
        let mut config = ProviderConfig {
            name: "azure".to_string(),
            health_check: ProviderHealthCheckConfig {
                endpoint: Some("health".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        config.settings.insert(
            "azure_endpoint".to_string(),
            "http://127.0.0.1:18080/openai/deployments/test".into(),
        );

        assert_eq!(
            config
                .resolved_health_check_endpoint()
                .expect("name fallback must resolve relative health endpoint")
                .expect("health endpoint must be configured")
                .as_str(),
            "http://127.0.0.1:18080/openai/deployments/test/health"
        );
    }

    #[test]
    fn test_provider_config_with_settings() {
        let mut settings = HashMap::new();
        settings.insert("custom_param".to_string(), serde_json::json!("value"));
        settings.insert("max_tokens".to_string(), serde_json::json!(4096));

        let config = ProviderConfig {
            name: "custom".to_string(),
            provider_type: "custom".to_string(),
            api_key: "key".to_string(),
            base_url: None,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            api_version: None,
            organization: None,
            project: None,
            weight: 1.0,
            rpm: 60,
            tpm: 60000,
            max_concurrent_requests: 10,
            timeout: 30,
            max_retries: 3,
            retry: RetryConfig::default(),
            health_check: ProviderHealthCheckConfig::default(),
            settings,
            models: vec![],
            tags: vec![],
            enabled: true,
        };
        assert_eq!(config.settings.len(), 2);
    }

    #[test]
    fn test_provider_config_serialization() {
        let config = ProviderConfig {
            name: "test-provider".to_string(),
            provider_type: "anthropic".to_string(),
            api_key: "sk-ant-xxx".to_string(),
            base_url: None,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            api_version: None,
            organization: None,
            project: None,
            weight: 1.5,
            rpm: 50,
            tpm: 50000,
            max_concurrent_requests: 15,
            timeout: 45,
            max_retries: 4,
            retry: RetryConfig::default(),
            health_check: ProviderHealthCheckConfig::default(),
            settings: HashMap::new(),
            models: vec!["claude-3".into()],
            tags: vec!["backup".to_string()],
            enabled: true,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["name"], "test-provider");
        assert_eq!(json["provider_type"], "anthropic");
        assert_eq!(json["endpoint_access"], "public_only");
        assert_eq!(json["rpm"], 50);
    }

    #[test]
    fn test_provider_config_deserialization() {
        let json = r#"{
            "name": "gemini",
            "provider_type": "google",
            "api_key": "gcp-key",
            "weight": 0.5,
            "rpm": 30,
            "tpm": 30000,
            "max_concurrent_requests": 5,
            "timeout": 20,
            "max_retries": 2,
            "enabled": false
        }"#;
        let config: ProviderConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.name, "gemini");
        assert!(!config.enabled);
        assert!((config.weight - 0.5).abs() < f32::EPSILON);
        assert_eq!(config.endpoint_access, ProviderEndpointAccess::PublicOnly);
    }

    #[test]
    fn test_provider_endpoint_access_yaml_is_strict() {
        let private: ProviderConfig = serde_yml::from_str(
            "name: local\nprovider_type: openai_compatible\napi_key: ''\nendpoint_access: private_network\n",
        )
        .expect("private endpoint access should deserialize");
        assert_eq!(
            private.endpoint_access,
            ProviderEndpointAccess::PrivateNetwork
        );
        for invalid in ["", "private"] {
            let yaml = format!(
                "name: local\nprovider_type: openai\napi_key: key\nendpoint_access: {invalid}\n"
            );
            assert!(serde_yml::from_str::<ProviderConfig>(&yaml).is_err());
        }
    }

    #[test]
    fn test_provider_config_clone() {
        let config = ProviderConfig::default();
        let cloned = config.clone();
        assert_eq!(config.name, cloned.name);
        assert_eq!(config.weight, cloned.weight);
        assert_eq!(config.enabled, cloned.enabled);
    }

    #[test]
    fn test_provider_config_with_tags() {
        let config = ProviderConfig {
            tags: vec![
                "production".to_string(),
                "primary".to_string(),
                "fast".to_string(),
            ],
            ..ProviderConfig::default()
        };
        assert_eq!(config.tags.len(), 3);
        assert!(config.tags.contains(&"primary".to_string()));
    }

    #[test]
    fn test_provider_config_with_models() {
        let config = ProviderConfig {
            models: vec![
                "gpt-4".into(),
                "gpt-4-turbo".into(),
                "gpt-3.5-turbo".into(),
            ],
            ..ProviderConfig::default()
        };
        assert_eq!(config.models.len(), 3);
    }
}
