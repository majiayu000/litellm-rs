//! LangGraph Configuration
//!
//! Configuration for connecting to LangGraph Cloud

use crate::core::providers::base::config::BaseConfig;
use crate::core::traits::provider::ProviderConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// LangGraph Cloud configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LangGraphConfig {
    /// Base configuration (api_key, api_base, timeout, etc.)
    #[serde(flatten)]
    pub base: BaseConfig,

    /// Specific graph ID to use (optional - can be specified per request)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_id: Option<String>,

    /// Default assistant ID for the graph
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_id: Option<String>,

    /// Thread ID for stateful conversations (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,

    /// Enable checkpointing for state persistence
    #[serde(default = "default_checkpointing")]
    pub enable_checkpointing: bool,

    /// Maximum number of iterations for graph execution
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
}

fn default_checkpointing() -> bool {
    true
}

fn default_max_iterations() -> u32 {
    25
}

impl Default for LangGraphConfig {
    fn default() -> Self {
        Self {
            base: BaseConfig {
                api_key: None,
                api_base: Some("https://api.smith.langchain.com".to_string()),
                timeout: 120, // LangGraph may have longer execution times
                max_retries: 3,
                headers: std::collections::HashMap::new(),
                organization: None,
                api_version: None,
            },
            graph_id: None,
            assistant_id: None,
            thread_id: None,
            enable_checkpointing: default_checkpointing(),
            max_iterations: default_max_iterations(),
        }
    }
}

impl LangGraphConfig {
    /// Create configuration from environment variables
    ///
    /// Looks for:
    /// - `LANGGRAPH_API_KEY` or `LANGSMITH_API_KEY`
    /// - `LANGGRAPH_API_BASE` (defaults to LangGraph Cloud)
    /// - `LANGGRAPH_GRAPH_ID`
    /// - `LANGGRAPH_ASSISTANT_ID`
    pub fn from_env() -> Self {
        let api_key = std::env::var("LANGGRAPH_API_KEY")
            .or_else(|_| std::env::var("LANGSMITH_API_KEY"))
            .ok();

        let api_base = std::env::var("LANGGRAPH_API_BASE")
            .ok()
            .or_else(|| Some("https://api.smith.langchain.com".to_string()));

        let timeout = std::env::var("LANGGRAPH_TIMEOUT")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(120);

        let graph_id = std::env::var("LANGGRAPH_GRAPH_ID").ok();
        let assistant_id = std::env::var("LANGGRAPH_ASSISTANT_ID").ok();

        Self {
            base: BaseConfig {
                api_key,
                api_base,
                timeout,
                max_retries: 3,
                headers: std::collections::HashMap::new(),
                organization: None,
                api_version: None,
            },
            graph_id,
            assistant_id,
            thread_id: None,
            enable_checkpointing: default_checkpointing(),
            max_iterations: default_max_iterations(),
        }
    }

    /// Create a new configuration with the specified API key
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.base.api_key = Some(api_key.into());
        config
    }

    /// Set the graph ID
    pub fn with_graph_id(mut self, graph_id: impl Into<String>) -> Self {
        self.graph_id = Some(graph_id.into());
        self
    }

    /// Set the assistant ID
    pub fn with_assistant_id(mut self, assistant_id: impl Into<String>) -> Self {
        self.assistant_id = Some(assistant_id.into());
        self
    }

    /// Set the API base URL
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.base.api_base = Some(api_base.into());
        self
    }

    /// Get the effective API base URL
    pub fn get_api_base(&self) -> String {
        self.base
            .api_base
            .clone()
            .unwrap_or_else(|| "https://api.smith.langchain.com".to_string())
    }

    /// Get the API key
    pub fn get_api_key(&self) -> Option<&str> {
        self.base.api_key.as_deref()
    }
}

impl ProviderConfig for LangGraphConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate_standard("LangGraph")
    }

    fn api_key(&self) -> Option<&str> {
        self.base.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        self.base.api_base.as_deref()
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(self.base.timeout)
    }

    fn max_retries(&self) -> u32 {
        self.base.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LangGraphConfig::default();
        assert!(config.base.api_key.is_none());
        assert_eq!(
            config.base.api_base,
            Some("https://api.smith.langchain.com".to_string())
        );
        assert_eq!(config.base.timeout, 120);
        assert!(config.enable_checkpointing);
        assert_eq!(config.max_iterations, 25);
    }

    #[test]
    fn test_with_api_key() {
        let config = LangGraphConfig::with_api_key("lsv2_test_key");
        assert_eq!(config.base.api_key, Some("lsv2_test_key".to_string()));
    }

    #[test]
    fn test_builder_pattern() {
        let config = LangGraphConfig::with_api_key("test-key")
            .with_graph_id("my-graph")
            .with_assistant_id("my-assistant")
            .with_api_base("https://custom.langchain.com");

        assert_eq!(config.base.api_key, Some("test-key".to_string()));
        assert_eq!(config.graph_id, Some("my-graph".to_string()));
        assert_eq!(config.assistant_id, Some("my-assistant".to_string()));
        assert_eq!(
            config.base.api_base,
            Some("https://custom.langchain.com".to_string())
        );
    }

    #[test]
    fn test_validate_missing_api_key() {
        let config = LangGraphConfig::default();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key"));
    }

    #[test]
    fn test_validate_success() {
        let config = LangGraphConfig::with_api_key("lsv2_test_key");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_provider_config_trait() {
        let config = LangGraphConfig::with_api_key("test-key");

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("https://api.smith.langchain.com"));
        assert_eq!(config.timeout(), Duration::from_secs(120));
        assert_eq!(config.max_retries(), 3);
    }
}
