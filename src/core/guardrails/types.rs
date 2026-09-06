//! Core types for the Guardrails system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Error types for guardrails operations
#[derive(Debug, Error)]
pub enum GuardrailError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// API error (e.g., OpenAI moderation API)
    #[error("API error: {0}")]
    Api(String),

    /// Network error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for guardrail operations
pub type GuardrailResult<T> = Result<T, GuardrailError>;

/// Action to take when a guardrail is triggered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailAction {
    /// Block the request/response
    #[default]
    Block,
    /// Allow but log the violation
    Log,
    /// Mask the violating content
    Mask,
    /// Allow without any action
    Allow,
}

/// Type of violation detected
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationType {
    /// Content moderation violation
    Moderation(ModerationCategory),
    /// PII detected
    PII(PIIType),
    /// Prompt injection detected
    PromptInjection,
    /// Custom rule violation
    CustomRule(String),
}

/// OpenAI moderation categories
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationCategory {
    Hate,
    HateThreatening,
    Harassment,
    HarassmentThreatening,
    SelfHarm,
    SelfHarmIntent,
    SelfHarmInstructions,
    Sexual,
    SexualMinors,
    Violence,
    ViolenceGraphic,
}

impl ModerationCategory {
    /// Get all moderation categories
    pub fn all() -> Vec<Self> {
        vec![
            Self::Hate,
            Self::HateThreatening,
            Self::Harassment,
            Self::HarassmentThreatening,
            Self::SelfHarm,
            Self::SelfHarmIntent,
            Self::SelfHarmInstructions,
            Self::Sexual,
            Self::SexualMinors,
            Self::Violence,
            Self::ViolenceGraphic,
        ]
    }

    /// Convert from OpenAI API category name
    pub fn from_api_name(name: &str) -> Option<Self> {
        match name {
            "hate" => Some(Self::Hate),
            "hate/threatening" => Some(Self::HateThreatening),
            "harassment" => Some(Self::Harassment),
            "harassment/threatening" => Some(Self::HarassmentThreatening),
            "self-harm" => Some(Self::SelfHarm),
            "self-harm/intent" => Some(Self::SelfHarmIntent),
            "self-harm/instructions" => Some(Self::SelfHarmInstructions),
            "sexual" => Some(Self::Sexual),
            "sexual/minors" => Some(Self::SexualMinors),
            "violence" => Some(Self::Violence),
            "violence/graphic" => Some(Self::ViolenceGraphic),
            _ => None,
        }
    }

    /// Convert to OpenAI API category name
    pub fn to_api_name(&self) -> &'static str {
        match self {
            Self::Hate => "hate",
            Self::HateThreatening => "hate/threatening",
            Self::Harassment => "harassment",
            Self::HarassmentThreatening => "harassment/threatening",
            Self::SelfHarm => "self-harm",
            Self::SelfHarmIntent => "self-harm/intent",
            Self::SelfHarmInstructions => "self-harm/instructions",
            Self::Sexual => "sexual",
            Self::SexualMinors => "sexual/minors",
            Self::Violence => "violence",
            Self::ViolenceGraphic => "violence/graphic",
        }
    }
}

/// PII types that can be detected
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PIIType {
    /// Email address
    Email,
    /// Phone number
    Phone,
    /// Credit card number
    CreditCard,
    /// Social Security Number
    SSN,
    /// IP address
    IpAddress,
    /// Date of birth
    DateOfBirth,
    /// Physical address
    Address,
    /// Name (person)
    Name,
    /// Passport number
    Passport,
    /// Driver's license
    DriversLicense,
    /// Bank account number
    BankAccount,
    /// Custom PII type
    Custom(String),
}

impl PIIType {
    /// Get all standard PII types
    pub fn standard() -> Vec<Self> {
        vec![
            Self::Email,
            Self::Phone,
            Self::CreditCard,
            Self::SSN,
            Self::IpAddress,
        ]
    }
}

/// A match of PII in text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PIIMatch {
    /// Type of PII detected
    pub pii_type: PIIType,
    /// Start position in text
    pub start: usize,
    /// End position in text
    pub end: usize,
    /// The matched text (may be redacted)
    pub text: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
}

impl PIIMatch {
    /// Create a new PII match
    pub fn new(pii_type: PIIType, start: usize, end: usize, text: String) -> Self {
        Self {
            pii_type,
            start,
            end,
            text,
            confidence: 1.0,
        }
    }

    /// Set confidence score
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }
}

/// Result of content moderation check
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModerationResult {
    /// Whether the content was flagged
    pub flagged: bool,
    /// Categories that were flagged
    pub categories: HashMap<ModerationCategory, bool>,
    /// Category scores (0.0 - 1.0)
    pub category_scores: HashMap<ModerationCategory, f64>,
}

impl ModerationResult {
    /// Create a new moderation result
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any category is flagged
    pub fn is_flagged(&self) -> bool {
        self.flagged
    }

    /// Get flagged categories
    pub fn flagged_categories(&self) -> Vec<&ModerationCategory> {
        self.categories
            .iter()
            .filter(|&(_, flagged)| *flagged)
            .map(|(cat, _)| cat)
            .collect()
    }

    /// Get score for a category
    pub fn score(&self, category: &ModerationCategory) -> f64 {
        self.category_scores.get(category).copied().unwrap_or(0.0)
    }
}

/// Result of a guardrail check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Whether the content passed all checks
    pub passed: bool,
    /// Action to take
    pub action: GuardrailAction,
    /// Violations detected
    pub violations: Vec<Violation>,
    /// Modified content (if masking was applied)
    pub modified_content: Option<String>,
    /// Metadata about the check
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for CheckResult {
    fn default() -> Self {
        Self {
            passed: true,
            action: GuardrailAction::Allow,
            violations: Vec::new(),
            modified_content: None,
            metadata: HashMap::new(),
        }
    }
}

impl CheckResult {
    /// Create a passing result
    pub fn pass() -> Self {
        Self::default()
    }

    /// Create a blocking result
    pub fn block(violations: Vec<Violation>) -> Self {
        Self {
            passed: false,
            action: GuardrailAction::Block,
            violations,
            modified_content: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a result with masked content
    pub fn mask(modified_content: String, violations: Vec<Violation>) -> Self {
        Self {
            passed: true,
            action: GuardrailAction::Mask,
            violations,
            modified_content: Some(modified_content),
            metadata: HashMap::new(),
        }
    }

    /// Check if the content is blocked
    pub fn is_blocked(&self) -> bool {
        self.action == GuardrailAction::Block
    }

    /// Check if content was modified
    pub fn is_modified(&self) -> bool {
        self.modified_content.is_some()
    }

    /// Get the content to use (modified or original)
    pub fn get_content<'a>(&'a self, original: &'a str) -> &'a str {
        self.modified_content.as_deref().unwrap_or(original)
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Merge with another result (combines violations)
    pub fn merge(mut self, other: Self) -> Self {
        // If either is blocked, result is blocked
        if other.action == GuardrailAction::Block {
            self.action = GuardrailAction::Block;
            self.passed = false;
        } else if self.action != GuardrailAction::Block && other.action == GuardrailAction::Mask {
            self.action = GuardrailAction::Mask;
        } else if !matches!(self.action, GuardrailAction::Block | GuardrailAction::Mask)
            && other.action == GuardrailAction::Log
        {
            self.action = GuardrailAction::Log;
        }

        // Merge violations
        self.violations.extend(other.violations);

        // Use modified content from other if present
        if other.modified_content.is_some() {
            self.modified_content = other.modified_content;
        }

        // Merge metadata
        self.metadata.extend(other.metadata);

        self
    }
}

/// A specific violation detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    /// Type of violation
    pub violation_type: ViolationType,
    /// Human-readable message
    pub message: String,
    /// Severity (0.0 - 1.0)
    pub severity: f64,
    /// Location in text (if applicable)
    pub location: Option<TextLocation>,
    /// Additional details
    pub details: HashMap<String, serde_json::Value>,
}

impl Violation {
    /// Create a new violation
    pub fn new(violation_type: ViolationType, message: impl Into<String>) -> Self {
        Self {
            violation_type,
            message: message.into(),
            severity: 1.0,
            location: None,
            details: HashMap::new(),
        }
    }

    /// Set severity
    pub fn with_severity(mut self, severity: f64) -> Self {
        self.severity = severity;
        self
    }

    /// Set location
    pub fn with_location(mut self, start: usize, end: usize) -> Self {
        self.location = Some(TextLocation { start, end });
        self
    }

    /// Add detail
    pub fn with_detail(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.details.insert(key.into(), value);
        self
    }
}

/// Location in text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLocation {
    /// Start position
    pub start: usize,
    /// End position
    pub end: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guardrail_action_default() {
        let action = GuardrailAction::default();
        assert_eq!(action, GuardrailAction::Block);
    }

    #[test]
    fn test_moderation_category_from_api_name() {
        assert_eq!(
            ModerationCategory::from_api_name("hate"),
            Some(ModerationCategory::Hate)
        );
        assert_eq!(
            ModerationCategory::from_api_name("hate/threatening"),
            Some(ModerationCategory::HateThreatening)
        );
        assert_eq!(
            ModerationCategory::from_api_name("self-harm/intent"),
            Some(ModerationCategory::SelfHarmIntent)
        );
        assert_eq!(ModerationCategory::from_api_name("unknown"), None);
    }

    #[test]
    fn test_moderation_category_to_api_name() {
        assert_eq!(ModerationCategory::Hate.to_api_name(), "hate");
        assert_eq!(
            ModerationCategory::HateThreatening.to_api_name(),
            "hate/threatening"
        );
    }

    #[test]
    fn test_pii_type_standard() {
        let standard = PIIType::standard();
        assert!(standard.contains(&PIIType::Email));
        assert!(standard.contains(&PIIType::Phone));
        assert!(standard.contains(&PIIType::CreditCard));
    }

    #[test]
    fn test_pii_match() {
        let m = PIIMatch::new(PIIType::Email, 0, 15, "test@example.com".to_string())
            .with_confidence(0.95);
        assert_eq!(m.pii_type, PIIType::Email);
        assert_eq!(m.confidence, 0.95);
    }

    #[test]
    fn test_moderation_result() {
        let mut result = ModerationResult::new();
        result.flagged = true;
        result.categories.insert(ModerationCategory::Hate, true);
        result
            .categories
            .insert(ModerationCategory::Violence, false);
        result.category_scores.insert(ModerationCategory::Hate, 0.9);

        assert!(result.is_flagged());
        assert_eq!(result.flagged_categories().len(), 1);
        assert_eq!(result.score(&ModerationCategory::Hate), 0.9);
        assert_eq!(result.score(&ModerationCategory::Sexual), 0.0);
    }

    #[test]
    fn test_check_result_pass() {
        let result = CheckResult::pass();
        assert!(result.passed);
        assert!(!result.is_blocked());
        assert!(!result.is_modified());
    }

    #[test]
    fn test_check_result_block() {
        let violations = vec![Violation::new(
            ViolationType::PromptInjection,
            "Prompt injection detected",
        )];
        let result = CheckResult::block(violations);
        assert!(!result.passed);
        assert!(result.is_blocked());
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn test_check_result_mask() {
        let violations = vec![Violation::new(
            ViolationType::PII(PIIType::Email),
            "Email detected",
        )];
        let result = CheckResult::mask("[REDACTED]".to_string(), violations);
        assert!(result.passed);
        assert!(!result.is_blocked());
        assert!(result.is_modified());
        assert_eq!(result.get_content("original"), "[REDACTED]");
    }

    #[test]
    fn test_check_result_merge() {
        let r1 = CheckResult::pass();
        let violations = vec![Violation::new(ViolationType::PromptInjection, "Injection")];
        let r2 = CheckResult::block(violations);

        let merged = r1.merge(r2);
        assert!(merged.is_blocked());
        assert_eq!(merged.violations.len(), 1);
    }

    #[test]
    fn test_violation() {
        let v = Violation::new(ViolationType::PromptInjection, "Test violation")
            .with_severity(0.8)
            .with_location(10, 20)
            .with_detail("pattern", serde_json::json!("ignore previous"));

        assert_eq!(v.severity, 0.8);
        assert!(v.location.is_some());
        assert!(v.details.contains_key("pattern"));
    }
}
