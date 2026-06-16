//! Provider test utilities
//!
//! Utilities for testing AI providers without mocking.
//! Uses real provider implementations with optional API key checks.

use std::env;

/// Configuration for provider tests
#[derive(Debug, Clone)]
pub struct ProviderTestConfig {
    /// Whether to skip tests that require live API calls
    pub skip_live_tests: bool,
    /// Default timeout for API calls in seconds
    pub timeout_secs: u64,
}

impl Default for ProviderTestConfig {
    fn default() -> Self {
        Self {
            skip_live_tests: env::var("SKIP_LIVE_TESTS").is_ok() || env::var("CI").is_ok(),
            timeout_secs: 30,
        }
    }
}

impl ProviderTestConfig {
    /// Check if live tests should run
    pub fn should_run_live_tests(&self) -> bool {
        !self.skip_live_tests
    }
}

/// Get API key for a provider from environment
pub fn get_api_key(provider: &str) -> Option<String> {
    let key_vars: &[&str] = match provider.to_lowercase().as_str() {
        "openai" => &["OPENAI_API_KEY"],
        "anthropic" | "claude" => &["ANTHROPIC_API_KEY"],
        "groq" => &["GROQ_API_KEY"],
        "gemini" | "google" => &["GOOGLE_API_KEY"],
        "azure" | "azure_openai" => &["AZURE_OPENAI_API_KEY"],
        "cohere" => &["COHERE_API_KEY"],
        "mistral" => &["MISTRAL_API_KEY"],
        "deepseek" => &["DEEPSEEK_API_KEY"],
        "xiaomi_mimo" | "mimo" => &["MIMO_API_KEY", "XIAOMI_API_KEY"],
        "together" => &["TOGETHER_API_KEY"],
        "openrouter" => &["OPENROUTER_API_KEY"],
        "deepinfra" => &["DEEPINFRA_API_KEY"],
        _ => return None,
    };

    key_vars.iter().find_map(|key_var| env::var(key_var).ok())
}

/// Check if API key is available for a provider
pub fn has_api_key(provider: &str) -> bool {
    get_api_key(provider).is_some()
}

/// Get list of available providers (those with API keys set)
pub fn available_providers() -> Vec<String> {
    let providers = vec![
        "openai",
        "anthropic",
        "groq",
        "gemini",
        "azure",
        "cohere",
        "mistral",
        "deepseek",
        "xiaomi_mimo",
        "together",
        "openrouter",
        "deepinfra",
    ];

    providers
        .into_iter()
        .filter(|p| has_api_key(p))
        .map(|s| s.to_string())
        .collect()
}

/// Test models for each provider
pub fn test_models() -> std::collections::HashMap<&'static str, &'static str> {
    let mut models = std::collections::HashMap::new();
    models.insert("openai", "gpt-3.5-turbo");
    models.insert("anthropic", "claude-3-haiku-20240307");
    models.insert("groq", "llama-3.1-8b-instant");
    models.insert("gemini", "gemini-1.5-flash");
    models.insert("mistral", "mistral-small-latest");
    models.insert("deepseek", "deepseek-v4-flash");
    models.insert("xiaomi_mimo", "mimo-v2.5");
    models.insert("together", "meta-llama/Llama-3.2-3B-Instruct-Turbo");
    models
}

/// Get a test model for a provider
pub fn get_test_model(provider: &str) -> Option<&'static str> {
    test_models().get(provider).copied()
}

/// Provider test builder for fluent API
pub struct ProviderTestBuilder {
    provider: String,
    model: Option<String>,
    timeout: u64,
}

impl ProviderTestBuilder {
    /// Create a new test builder for a provider
    pub fn new(provider: &str) -> Self {
        Self {
            provider: provider.to_string(),
            model: None,
            timeout: 30,
        }
    }

    /// Set the model to test
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    /// Set the timeout
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout = seconds;
        self
    }

    /// Check if this test can run (API key available)
    pub fn can_run(&self) -> bool {
        has_api_key(&self.provider)
    }

    /// Get the API key
    pub fn api_key(&self) -> Option<String> {
        get_api_key(&self.provider)
    }

    /// Get the model to use
    pub fn model(&self) -> String {
        self.model
            .clone()
            .or_else(|| get_test_model(&self.provider).map(|s| s.to_string()))
            .unwrap_or_else(|| "gpt-3.5-turbo".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_default() {
        let config = ProviderTestConfig::default();
        assert!(config.timeout_secs > 0);
    }

    #[test]
    fn test_get_api_key_mapping() {
        // These tests don't require actual keys
        // Just verify the mapping logic
        assert!(get_api_key("unknown_provider").is_none());
    }

    #[test]
    fn test_provider_test_builder() {
        let builder = ProviderTestBuilder::new("openai")
            .with_model("gpt-4")
            .with_timeout(60);

        assert_eq!(builder.model(), "gpt-4");
        assert_eq!(builder.timeout, 60);
    }

    #[test]
    fn test_test_models() {
        let models = test_models();
        assert!(models.contains_key("openai"));
        assert!(models.contains_key("groq"));
    }
}
