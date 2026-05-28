//! Unified Thinking/Reasoning Types
//!
//! This module provides a unified abstraction for thinking/reasoning features
//! across all AI providers (OpenAI o-series, Anthropic Claude, DeepSeek R1, Gemini).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unified thinking content - provider agnostic
///
/// Different providers return thinking/reasoning in different formats:
/// - OpenAI: `reasoning` field in message
/// - Anthropic: `thinking` blocks in content
/// - DeepSeek: `reasoning_content` field
/// - Gemini: `thoughts` field
///
/// This enum normalizes all formats into a single type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThinkingContent {
    /// Text-based thinking (OpenAI, DeepSeek, Gemini)
    Text {
        /// The thinking/reasoning text
        text: String,
        /// Optional signature for verification (Anthropic)
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    /// Structured thinking blocks (Anthropic style)
    Block {
        /// The thinking content
        thinking: String,
        /// Block type identifier
        #[serde(skip_serializing_if = "Option::is_none")]
        block_type: Option<String>,
    },
    /// Redacted thinking (when provider hides details)
    Redacted {
        /// Number of tokens used for thinking (if available)
        #[serde(skip_serializing_if = "Option::is_none")]
        token_count: Option<u32>,
    },
}

impl ThinkingContent {
    /// Create text-based thinking content
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            text: content.into(),
            signature: None,
        }
    }

    /// Create text-based thinking with signature
    pub fn text_with_signature(content: impl Into<String>, signature: impl Into<String>) -> Self {
        Self::Text {
            text: content.into(),
            signature: Some(signature.into()),
        }
    }

    /// Create block-based thinking content (Anthropic style)
    pub fn block(thinking: impl Into<String>) -> Self {
        Self::Block {
            thinking: thinking.into(),
            block_type: Some("thinking".to_string()),
        }
    }

    /// Create redacted thinking with token count
    pub fn redacted(token_count: Option<u32>) -> Self {
        Self::Redacted { token_count }
    }

    /// Get the thinking text content (if available)
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } => Some(text),
            Self::Block { thinking, .. } => Some(thinking),
            Self::Redacted { .. } => None,
        }
    }

    /// Check if thinking is redacted
    pub fn is_redacted(&self) -> bool {
        matches!(self, Self::Redacted { .. })
    }
}

/// Default value for include_thinking
fn default_include_thinking() -> bool {
    true
}

/// Unified thinking request configuration
///
/// This configuration is normalized across all providers:
/// - OpenAI: maps to `max_reasoning_tokens` and `include_reasoning`
/// - Anthropic: maps to `thinking.enabled` and `thinking.budget_tokens`
/// - DeepSeek: maps to `reasoning_effort`
/// - Gemini: maps to thinking parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Enable thinking mode
    #[serde(default)]
    pub enabled: bool,

    /// Maximum thinking tokens budget
    ///
    /// Provider-specific limits:
    /// - OpenAI: max 20,000
    /// - Anthropic: varies by model
    /// - DeepSeek: no explicit limit
    /// - Gemini: varies by model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,

    /// Thinking effort level (normalized across providers)
    ///
    /// Maps to provider-specific values:
    /// - DeepSeek: `reasoning_effort` (low/medium/high)
    /// - Others: budget scaling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ThinkingEffort>,

    /// Include thinking content in response
    ///
    /// When false, thinking is performed but not returned.
    /// Default: true
    #[serde(default = "default_include_thinking")]
    pub include_thinking: bool,

    /// Provider-specific extra parameters
    #[serde(flatten, skip_serializing_if = "HashMap::is_empty")]
    pub extra_params: HashMap<String, serde_json::Value>,
}

impl Default for ThinkingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            budget_tokens: None,
            effort: None,
            include_thinking: default_include_thinking(),
            extra_params: HashMap::new(),
        }
    }
}

impl ThinkingConfig {
    /// Create a new thinking config with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable thinking mode
    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Set thinking token budget
    pub fn with_budget(mut self, tokens: u32) -> Self {
        self.budget_tokens = Some(tokens);
        self
    }

    /// Set thinking effort level
    pub fn with_effort(mut self, effort: ThinkingEffort) -> Self {
        self.effort = Some(effort);
        self
    }

    /// Set whether to include thinking in response
    pub fn include_in_response(mut self, include: bool) -> Self {
        self.include_thinking = include;
        self
    }

    /// Add provider-specific parameter
    pub fn with_param(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.extra_params.insert(key.into(), value);
        self
    }

    /// Create config for high-effort thinking
    pub fn high_effort() -> Self {
        Self {
            enabled: true,
            effort: Some(ThinkingEffort::High),
            include_thinking: true,
            ..Default::default()
        }
    }

    /// Create config for medium-effort thinking (default)
    pub fn medium_effort() -> Self {
        Self {
            enabled: true,
            effort: Some(ThinkingEffort::Medium),
            include_thinking: true,
            ..Default::default()
        }
    }

    /// Create config for low-effort thinking (fast)
    pub fn low_effort() -> Self {
        Self {
            enabled: true,
            effort: Some(ThinkingEffort::Low),
            include_thinking: true,
            ..Default::default()
        }
    }
}

/// Thinking effort levels (provider-agnostic)
///
/// These levels are normalized across providers:
/// - Low: Minimal thinking, fast responses
/// - Medium: Balanced thinking (default for most models)
/// - High: Deep thinking, thorough reasoning
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingEffort {
    /// Minimal thinking - fast responses
    Low,
    /// Balanced thinking (default)
    #[default]
    Medium,
    /// Deep thinking - thorough reasoning
    High,
}

impl ThinkingEffort {
    /// Convert to provider-specific string (e.g., DeepSeek)
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// Get suggested token budget for this effort level
    pub fn suggested_budget(&self) -> u32 {
        match self {
            Self::Low => 2000,
            Self::Medium => 8000,
            Self::High => 16000,
        }
    }
}

impl std::fmt::Display for ThinkingEffort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Thinking usage statistics
///
/// Tracks token usage and costs specifically for thinking/reasoning.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ThinkingUsage {
    /// Tokens used for thinking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<u32>,

    /// Budget that was allocated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,

    /// Cost for thinking (USD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_cost: Option<f64>,

    /// Provider that generated the thinking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

impl ThinkingUsage {
    /// Create new thinking usage with token count
    pub fn new(thinking_tokens: u32) -> Self {
        Self {
            thinking_tokens: Some(thinking_tokens),
            ..Default::default()
        }
    }

    /// Set the budget that was allocated
    pub fn with_budget(mut self, budget: u32) -> Self {
        self.budget_tokens = Some(budget);
        self
    }

    /// Set the thinking cost
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.thinking_cost = Some(cost);
        self
    }

    /// Set the provider
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }
}

/// Provider-specific thinking capabilities
///
/// Describes what thinking features a provider/model supports.
#[derive(Debug, Clone, Default)]
pub struct ThinkingCapabilities {
    /// Whether the model supports thinking mode
    pub supports_thinking: bool,

    /// Whether thinking can be streamed
    pub supports_streaming_thinking: bool,

    /// Maximum thinking tokens allowed
    pub max_thinking_tokens: Option<u32>,

    /// Supported effort levels
    pub supported_efforts: Vec<ThinkingEffort>,

    /// List of models that support thinking
    pub thinking_models: Vec<String>,

    /// Whether thinking content can be returned
    pub can_return_thinking: bool,

    /// Whether thinking is always performed (can't be disabled)
    pub thinking_always_on: bool,
}

impl ThinkingCapabilities {
    /// Create capabilities for a provider that supports thinking
    pub fn supported() -> Self {
        Self {
            supports_thinking: true,
            supports_streaming_thinking: false,
            max_thinking_tokens: None,
            supported_efforts: vec![
                ThinkingEffort::Low,
                ThinkingEffort::Medium,
                ThinkingEffort::High,
            ],
            thinking_models: Vec::new(),
            can_return_thinking: true,
            thinking_always_on: false,
        }
    }

    /// Create capabilities for a provider that doesn't support thinking
    pub fn unsupported() -> Self {
        Self::default()
    }

    /// Set maximum thinking tokens
    pub fn with_max_tokens(mut self, max: u32) -> Self {
        self.max_thinking_tokens = Some(max);
        self
    }

    /// Enable streaming thinking support
    pub fn with_streaming(mut self) -> Self {
        self.supports_streaming_thinking = true;
        self
    }

    /// Add thinking models
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.thinking_models = models;
        self
    }
}

/// Thinking delta for streaming responses
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ThinkingDelta {
    /// Incremental thinking content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Provider signature for verifying thinking content integrity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    /// Whether this is the start of thinking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_start: Option<bool>,

    /// Whether thinking is complete
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_complete: Option<bool>,
}

impl ThinkingDelta {
    /// Create a new thinking delta with content
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
            signature: None,
            is_start: None,
            is_complete: None,
        }
    }

    /// Create a start marker
    pub fn start() -> Self {
        Self {
            content: None,
            signature: None,
            is_start: Some(true),
            is_complete: None,
        }
    }

    /// Create an end marker
    pub fn complete() -> Self {
        Self {
            content: None,
            signature: None,
            is_start: None,
            is_complete: Some(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_content_text() {
        let content = ThinkingContent::text("Let me think about this...");
        assert_eq!(content.as_text(), Some("Let me think about this..."));
        assert!(!content.is_redacted());
    }

    #[test]
    fn test_thinking_content_block() {
        let content = ThinkingContent::block("Step 1: Analyze the problem");
        assert_eq!(content.as_text(), Some("Step 1: Analyze the problem"));
    }

    #[test]
    fn test_thinking_content_redacted() {
        let content = ThinkingContent::redacted(Some(500));
        assert!(content.is_redacted());
        assert_eq!(content.as_text(), None);
    }

    #[test]
    fn test_thinking_config_builder() {
        let config = ThinkingConfig::new()
            .enabled()
            .with_budget(10000)
            .with_effort(ThinkingEffort::High)
            .include_in_response(true);

        assert!(config.enabled);
        assert_eq!(config.budget_tokens, Some(10000));
        assert_eq!(config.effort, Some(ThinkingEffort::High));
        assert!(config.include_thinking);
    }

    #[test]
    fn test_thinking_effort_presets() {
        let high = ThinkingConfig::high_effort();
        assert!(high.enabled);
        assert_eq!(high.effort, Some(ThinkingEffort::High));

        let low = ThinkingConfig::low_effort();
        assert_eq!(low.effort, Some(ThinkingEffort::Low));
    }

    #[test]
    fn test_thinking_effort_suggested_budget() {
        assert_eq!(ThinkingEffort::Low.suggested_budget(), 2000);
        assert_eq!(ThinkingEffort::Medium.suggested_budget(), 8000);
        assert_eq!(ThinkingEffort::High.suggested_budget(), 16000);
    }

    #[test]
    fn test_thinking_usage() {
        let usage = ThinkingUsage::new(5000)
            .with_budget(10000)
            .with_cost(0.05)
            .with_provider("openai");

        assert_eq!(usage.thinking_tokens, Some(5000));
        assert_eq!(usage.budget_tokens, Some(10000));
        assert_eq!(usage.thinking_cost, Some(0.05));
        assert_eq!(usage.provider, Some("openai".to_string()));
    }

    #[test]
    fn test_thinking_capabilities() {
        let caps = ThinkingCapabilities::supported()
            .with_max_tokens(20000)
            .with_streaming()
            .with_models(vec!["o1-preview".to_string()]);

        assert!(caps.supports_thinking);
        assert!(caps.supports_streaming_thinking);
        assert_eq!(caps.max_thinking_tokens, Some(20000));
        assert_eq!(caps.thinking_models, vec!["o1-preview"]);
    }

    #[test]
    fn test_thinking_delta() {
        let start = ThinkingDelta::start();
        assert_eq!(start.is_start, Some(true));

        let content = ThinkingDelta::new("thinking...");
        assert_eq!(content.content, Some("thinking...".to_string()));

        let complete = ThinkingDelta::complete();
        assert_eq!(complete.is_complete, Some(true));
    }

    #[test]
    fn test_thinking_content_serialization() {
        let content = ThinkingContent::text("Hello");
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Hello\""));

        let parsed: ThinkingContent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, content);
    }

    #[test]
    fn test_thinking_content_text_with_signature() {
        let content = ThinkingContent::text_with_signature("reasoning content", "sig123");
        match content {
            ThinkingContent::Text { text, signature } => {
                assert_eq!(text, "reasoning content");
                assert_eq!(signature, Some("sig123".to_string()));
            }
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_thinking_config_with_param() {
        let config =
            ThinkingConfig::new().with_param("custom_key", serde_json::json!("custom_value"));
        assert!(config.extra_params.contains_key("custom_key"));
    }

    #[test]
    fn test_thinking_config_medium_effort() {
        let config = ThinkingConfig::medium_effort();
        assert!(config.enabled);
        assert_eq!(config.effort, Some(ThinkingEffort::Medium));
        assert!(config.include_thinking);
    }

    #[test]
    fn test_thinking_effort_as_str() {
        assert_eq!(ThinkingEffort::Low.as_str(), "low");
        assert_eq!(ThinkingEffort::Medium.as_str(), "medium");
        assert_eq!(ThinkingEffort::High.as_str(), "high");
    }

    #[test]
    fn test_thinking_effort_display() {
        assert_eq!(format!("{}", ThinkingEffort::Low), "low");
        assert_eq!(format!("{}", ThinkingEffort::Medium), "medium");
        assert_eq!(format!("{}", ThinkingEffort::High), "high");
    }

    #[test]
    fn test_thinking_capabilities_unsupported() {
        let caps = ThinkingCapabilities::unsupported();
        assert!(!caps.supports_thinking);
        assert!(!caps.supports_streaming_thinking);
        assert_eq!(caps.max_thinking_tokens, None);
    }

    #[test]
    fn test_default_include_thinking() {
        // Test the default function is called during deserialization
        let json = r#"{"enabled": true}"#;
        let config: ThinkingConfig = serde_json::from_str(json).unwrap();
        assert!(config.include_thinking); // default should be true
    }
}
