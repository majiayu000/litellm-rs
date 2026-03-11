//! Configuration for the Guardrails system

use crate::core::types::config::defaults::default_true;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::types::{GuardrailAction, ModerationCategory, PIIType};

/// Main configuration for the guardrails system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailConfig {
    /// Whether guardrails are enabled
    #[serde(default)]
    pub enabled: bool,

    /// OpenAI moderation configuration
    #[serde(default)]
    pub openai_moderation: Option<OpenAIModerationConfig>,

    /// PII detection configuration
    #[serde(default)]
    pub pii: Option<PIIConfig>,

    /// Prompt injection detection configuration
    #[serde(default)]
    pub prompt_injection: Option<PromptInjectionConfig>,

    /// Custom rules configuration
    #[serde(default)]
    pub custom_rules: Vec<CustomRuleConfig>,

    /// Default action when a guardrail is triggered
    #[serde(default)]
    pub default_action: GuardrailAction,

    /// Whether to check input (requests)
    #[serde(default = "default_true")]
    pub check_input: bool,

    /// Whether to check output (responses)
    #[serde(default = "default_true")]
    pub check_output: bool,

    /// Paths to exclude from guardrail checks
    #[serde(default)]
    pub exclude_paths: Vec<String>,

    /// Whether to fail open (allow on error)
    #[serde(default)]
    pub fail_open: bool,
}

impl Default for GuardrailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            openai_moderation: None,
            pii: None,
            prompt_injection: None,
            custom_rules: Vec::new(),
            default_action: GuardrailAction::Block,
            check_input: true,
            check_output: true,
            exclude_paths: Vec::new(),
            fail_open: false,
        }
    }
}

impl GuardrailConfig {
    /// Create a new guardrail config
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable guardrails
    pub fn enable(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Enable OpenAI moderation
    pub fn with_openai_moderation(mut self, config: OpenAIModerationConfig) -> Self {
        self.openai_moderation = Some(config);
        self
    }

    /// Enable PII detection
    pub fn with_pii(mut self, config: PIIConfig) -> Self {
        self.pii = Some(config);
        self
    }

    /// Enable prompt injection detection
    pub fn with_prompt_injection(mut self, config: PromptInjectionConfig) -> Self {
        self.prompt_injection = Some(config);
        self
    }

    /// Add a custom rule
    pub fn with_custom_rule(mut self, rule: CustomRuleConfig) -> Self {
        self.custom_rules.push(rule);
        self
    }

    /// Set default action
    pub fn with_default_action(mut self, action: GuardrailAction) -> Self {
        self.default_action = action;
        self
    }

    /// Set fail open behavior
    pub fn with_fail_open(mut self, fail_open: bool) -> Self {
        self.fail_open = fail_open;
        self
    }

    /// Add excluded path
    pub fn exclude_path(mut self, path: impl Into<String>) -> Self {
        self.exclude_paths.push(path.into());
        self
    }

    /// Check if a path should be excluded
    pub fn is_path_excluded(&self, path: &str) -> bool {
        self.exclude_paths.iter().any(|p| path.starts_with(p))
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref moderation) = self.openai_moderation
            && moderation.enabled
            && moderation.api_key.is_none()
        {
            return Err("OpenAI moderation enabled but no API key provided".to_string());
        }
        Ok(())
    }
}

/// OpenAI Moderation API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIModerationConfig {
    /// Whether OpenAI moderation is enabled
    #[serde(default)]
    pub enabled: bool,

    /// OpenAI API key (can use env var)
    #[serde(default)]
    pub api_key: Option<String>,

    /// API base URL
    #[serde(default = "default_openai_base_url")]
    pub base_url: String,

    /// Model to use for moderation
    #[serde(default = "default_moderation_model")]
    pub model: String,

    /// Categories to check (empty = all)
    #[serde(default)]
    pub categories: HashSet<ModerationCategory>,

    /// Threshold for flagging (0.0 - 1.0)
    #[serde(default = "default_threshold")]
    pub threshold: f64,

    /// Action to take when flagged
    #[serde(default)]
    pub action: GuardrailAction,

    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_openai_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_moderation_model() -> String {
    "omni-moderation-latest".to_string()
}

fn default_threshold() -> f64 {
    0.5
}

fn default_timeout() -> u64 {
    5000
}

impl Default for OpenAIModerationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: None,
            base_url: default_openai_base_url(),
            model: default_moderation_model(),
            categories: HashSet::new(),
            threshold: default_threshold(),
            action: GuardrailAction::Block,
            timeout_ms: default_timeout(),
        }
    }
}

impl OpenAIModerationConfig {
    /// Create a new config with API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            enabled: true,
            api_key: Some(api_key.into()),
            ..Default::default()
        }
    }

    /// Create from environment variable
    pub fn from_env() -> Self {
        Self {
            enabled: true,
            api_key: std::env::var("OPENAI_API_KEY").ok(),
            ..Default::default()
        }
    }

    /// Set threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set categories to check
    pub fn with_categories(
        mut self,
        categories: impl IntoIterator<Item = ModerationCategory>,
    ) -> Self {
        self.categories = categories.into_iter().collect();
        self
    }

    /// Set action
    pub fn with_action(mut self, action: GuardrailAction) -> Self {
        self.action = action;
        self
    }
}

/// PII detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PIIConfig {
    /// Whether PII detection is enabled
    #[serde(default)]
    pub enabled: bool,

    /// PII types to detect (empty = all standard)
    #[serde(default)]
    pub types: HashSet<PIIType>,

    /// Action to take when PII is detected
    #[serde(default = "default_pii_action")]
    pub action: GuardrailAction,

    /// Mask character for redaction
    #[serde(default = "default_mask_char")]
    pub mask_char: char,

    /// Mask pattern (e.g., "[REDACTED]", "***")
    #[serde(default)]
    pub mask_pattern: Option<String>,

    /// Minimum confidence for detection (0.0 - 1.0)
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f64,

    /// Allow list of patterns to ignore
    #[serde(default)]
    pub allow_list: Vec<String>,
}

fn default_pii_action() -> GuardrailAction {
    GuardrailAction::Mask
}

fn default_mask_char() -> char {
    '*'
}

fn default_min_confidence() -> f64 {
    0.8
}

impl Default for PIIConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            types: HashSet::new(),
            action: default_pii_action(),
            mask_char: default_mask_char(),
            mask_pattern: None,
            min_confidence: default_min_confidence(),
            allow_list: Vec::new(),
        }
    }
}

impl PIIConfig {
    /// Create a new PII config
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    /// Set PII types to detect
    pub fn with_types(mut self, types: impl IntoIterator<Item = PIIType>) -> Self {
        self.types = types.into_iter().collect();
        self
    }

    /// Set action
    pub fn with_action(mut self, action: GuardrailAction) -> Self {
        self.action = action;
        self
    }

    /// Set mask pattern
    pub fn with_mask_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.mask_pattern = Some(pattern.into());
        self
    }

    /// Add to allow list
    pub fn allow(mut self, pattern: impl Into<String>) -> Self {
        self.allow_list.push(pattern.into());
        self
    }

    /// Get effective PII types (standard if empty)
    pub fn effective_types(&self) -> Vec<PIIType> {
        if self.types.is_empty() {
            PIIType::standard()
        } else {
            self.types.iter().cloned().collect()
        }
    }
}

/// Prompt injection detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptInjectionConfig {
    /// Whether prompt injection detection is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Action to take when injection is detected
    #[serde(default)]
    pub action: GuardrailAction,

    /// Detection sensitivity (0.0 - 1.0)
    #[serde(default = "default_sensitivity")]
    pub sensitivity: f64,

    /// Custom patterns to detect
    #[serde(default)]
    pub custom_patterns: Vec<String>,

    /// Patterns to ignore
    #[serde(default)]
    pub ignore_patterns: Vec<String>,

    /// Whether to use heuristic detection
    #[serde(default = "default_true")]
    pub use_heuristics: bool,
}

fn default_sensitivity() -> f64 {
    0.7
}

impl Default for PromptInjectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            action: GuardrailAction::Block,
            sensitivity: default_sensitivity(),
            custom_patterns: Vec::new(),
            ignore_patterns: Vec::new(),
            use_heuristics: true,
        }
    }
}

impl PromptInjectionConfig {
    /// Create a new config
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    /// Set sensitivity
    pub fn with_sensitivity(mut self, sensitivity: f64) -> Self {
        self.sensitivity = sensitivity.clamp(0.0, 1.0);
        self
    }

    /// Set action
    pub fn with_action(mut self, action: GuardrailAction) -> Self {
        self.action = action;
        self
    }

    /// Add custom pattern
    pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.custom_patterns.push(pattern.into());
        self
    }

    /// Add ignore pattern
    pub fn ignore(mut self, pattern: impl Into<String>) -> Self {
        self.ignore_patterns.push(pattern.into());
        self
    }
}

/// Custom rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRuleConfig {
    /// Rule name
    pub name: String,

    /// Rule description
    #[serde(default)]
    pub description: Option<String>,

    /// Whether the rule is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Regex patterns to match
    pub patterns: Vec<String>,

    /// Action to take when matched
    #[serde(default)]
    pub action: GuardrailAction,

    /// Message to return when triggered
    #[serde(default)]
    pub message: Option<String>,
}

impl CustomRuleConfig {
    /// Create a new custom rule
    pub fn new(name: impl Into<String>, patterns: Vec<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            enabled: true,
            patterns,
            action: GuardrailAction::Block,
            message: None,
        }
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set action
    pub fn with_action(mut self, action: GuardrailAction) -> Self {
        self.action = action;
        self
    }

    /// Set message
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guardrail_config_default() {
        let config = GuardrailConfig::default();
        assert!(!config.enabled);
        assert!(config.check_input);
        assert!(config.check_output);
        assert!(!config.fail_open);
    }

    #[test]
    fn test_guardrail_config_builder() {
        let config = GuardrailConfig::new()
            .enable()
            .with_openai_moderation(OpenAIModerationConfig::new("test-key"))
            .with_pii(PIIConfig::new())
            .with_prompt_injection(PromptInjectionConfig::new())
            .with_default_action(GuardrailAction::Log)
            .with_fail_open(true)
            .exclude_path("/health");

        assert!(config.enabled);
        assert!(config.openai_moderation.is_some());
        assert!(config.pii.is_some());
        assert!(config.prompt_injection.is_some());
        assert_eq!(config.default_action, GuardrailAction::Log);
        assert!(config.fail_open);
        assert!(config.is_path_excluded("/health"));
        assert!(!config.is_path_excluded("/api"));
    }

    #[test]
    fn test_openai_moderation_config() {
        let config = OpenAIModerationConfig::new("test-key")
            .with_threshold(0.8)
            .with_categories([ModerationCategory::Hate, ModerationCategory::Violence])
            .with_action(GuardrailAction::Log);

        assert!(config.enabled);
        assert_eq!(config.api_key, Some("test-key".to_string()));
        assert_eq!(config.threshold, 0.8);
        assert!(config.categories.contains(&ModerationCategory::Hate));
        assert_eq!(config.action, GuardrailAction::Log);
    }

    #[test]
    fn test_pii_config() {
        let config = PIIConfig::new()
            .with_types([PIIType::Email, PIIType::Phone])
            .with_action(GuardrailAction::Mask)
            .with_mask_pattern("[REDACTED]")
            .allow("test@example.com");

        assert!(config.enabled);
        assert!(config.types.contains(&PIIType::Email));
        assert_eq!(config.action, GuardrailAction::Mask);
        assert_eq!(config.mask_pattern, Some("[REDACTED]".to_string()));
        assert_eq!(config.allow_list.len(), 1);
    }

    #[test]
    fn test_pii_config_effective_types() {
        let empty_config = PIIConfig::default();
        assert_eq!(
            empty_config.effective_types().len(),
            PIIType::standard().len()
        );

        let specific_config = PIIConfig::new().with_types([PIIType::Email]);
        assert_eq!(specific_config.effective_types().len(), 1);
    }

    #[test]
    fn test_prompt_injection_config() {
        let config = PromptInjectionConfig::new()
            .with_sensitivity(0.9)
            .with_action(GuardrailAction::Block)
            .with_pattern("ignore previous")
            .ignore("safe pattern");

        assert!(config.enabled);
        assert_eq!(config.sensitivity, 0.9);
        assert_eq!(config.custom_patterns.len(), 1);
        assert_eq!(config.ignore_patterns.len(), 1);
    }

    #[test]
    fn test_custom_rule_config() {
        let rule = CustomRuleConfig::new("no-secrets", vec![r"api[_-]?key".to_string()])
            .with_description("Block API keys in content")
            .with_action(GuardrailAction::Block)
            .with_message("API keys are not allowed");

        assert_eq!(rule.name, "no-secrets");
        assert!(rule.enabled);
        assert!(rule.description.is_some());
        assert!(rule.message.is_some());
    }

    #[test]
    fn test_config_validation() {
        let valid_config = GuardrailConfig::default();
        assert!(valid_config.validate().is_ok());

        let invalid_config =
            GuardrailConfig::new().with_openai_moderation(OpenAIModerationConfig {
                enabled: true,
                api_key: None,
                ..Default::default()
            });
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = GuardrailConfig::new().enable().with_pii(PIIConfig::new());

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: GuardrailConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.enabled, deserialized.enabled);
        assert!(deserialized.pii.is_some());
    }
}
