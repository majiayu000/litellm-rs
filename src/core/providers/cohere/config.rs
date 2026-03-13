//! Cohere Provider Configuration
//!
//! Configuration for Cohere API integration

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::core::traits::provider::ProviderConfig;

/// Cohere API version to use
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CohereApiVersion {
    /// Cohere v1 API (legacy)
    V1,
    /// Cohere v2 API (current, OpenAI-compatible)
    #[default]
    V2,
}

impl CohereApiVersion {
    /// Get the API version path component
    pub fn as_path(&self) -> &'static str {
        match self {
            CohereApiVersion::V1 => "v1",
            CohereApiVersion::V2 => "v2",
        }
    }
}

/// Cohere provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohereConfig {
    /// API key for authentication
    pub api_key: String,

    /// API base URL (defaults to <https://api.cohere.ai>)
    pub api_base: String,

    /// API version to use (v1 or v2)
    pub api_version: CohereApiVersion,

    /// Request timeout in seconds
    pub timeout_seconds: u64,

    /// Maximum retry attempts
    pub max_retries: u32,

    /// Default input type for embeddings (search_document, search_query, classification, clustering)
    pub default_embedding_input_type: String,
}

impl Default for CohereConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.cohere.ai".to_string(),
            api_version: CohereApiVersion::V2,
            timeout_seconds: 60,
            max_retries: 3,
            default_embedding_input_type: "search_document".to_string(),
        }
    }
}

impl CohereConfig {
    /// Create a new config with the given API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            ..Default::default()
        }
    }

    /// Set the API version
    pub fn with_api_version(mut self, version: CohereApiVersion) -> Self {
        self.api_version = version;
        self
    }

    /// Set the API base URL
    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    /// Set the timeout
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// Get the chat endpoint URL
    pub fn chat_endpoint(&self) -> String {
        format!(
            "{}/{}/chat",
            self.api_base.trim_end_matches('/'),
            self.api_version.as_path()
        )
    }

    /// Get the embed endpoint URL
    pub fn embed_endpoint(&self) -> String {
        // Embeddings use v2 API
        format!("{}/v2/embed", self.api_base.trim_end_matches('/'))
    }

    /// Get the rerank endpoint URL
    pub fn rerank_endpoint(&self) -> String {
        // Rerank uses v1 API
        format!("{}/v1/rerank", self.api_base.trim_end_matches('/'))
    }

    /// Get the models endpoint URL
    pub fn models_endpoint(&self) -> String {
        format!("{}/v1/models", self.api_base.trim_end_matches('/'))
    }

    /// Create default headers for Cohere API requests
    pub fn create_headers(&self) -> std::collections::HashMap<String, String> {
        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", self.api_key),
        );
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Accept".to_string(), "application/json".to_string());
        headers.insert("Request-Source".to_string(), "litellm-rs".to_string());
        headers
    }
}

impl ProviderConfig for CohereConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate_standard("Cohere")
    }

    fn api_key(&self) -> Option<&str> {
        Some(&self.api_key)
    }

    fn api_base(&self) -> Option<&str> {
        Some(&self.api_base)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CohereConfig::default();
        assert_eq!(config.api_base, "https://api.cohere.ai");
        assert_eq!(config.api_version, CohereApiVersion::V2);
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_config_with_api_key() {
        let config = CohereConfig::new("test-key");
        assert_eq!(config.api_key, "test-key");
    }

    #[test]
    fn test_config_validation() {
        let config = CohereConfig::default();
        assert!(config.validate().is_err()); // No API key

        let config = CohereConfig::new("test-key");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_endpoints() {
        let config = CohereConfig::new("test-key");

        assert_eq!(config.chat_endpoint(), "https://api.cohere.ai/v2/chat");
        assert_eq!(config.embed_endpoint(), "https://api.cohere.ai/v2/embed");
        assert_eq!(config.rerank_endpoint(), "https://api.cohere.ai/v1/rerank");
    }

    #[test]
    fn test_v1_chat_endpoint() {
        let config = CohereConfig::new("test-key").with_api_version(CohereApiVersion::V1);
        assert_eq!(config.chat_endpoint(), "https://api.cohere.ai/v1/chat");
    }

    #[test]
    fn test_create_headers() {
        let config = CohereConfig::new("test-key");
        let headers = config.create_headers();

        assert_eq!(headers.get("Authorization").unwrap(), "Bearer test-key");
        assert_eq!(headers.get("Content-Type").unwrap(), "application/json");
    }
}
