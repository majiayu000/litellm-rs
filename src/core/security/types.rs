//! Security type definitions
//!
//! Core types used throughout the security module.

use regex::Regex;
use std::collections::HashMap;

/// PII (Personally Identifiable Information) pattern
#[derive(Debug, Clone)]
pub struct PIIPattern {
    /// Pattern name
    pub name: String,
    /// Regex pattern
    pub pattern: Regex,
    /// Replacement strategy
    pub replacement: PIIReplacement,
    /// Confidence level
    pub confidence: f64,
}

/// PII replacement strategies
#[derive(Debug, Clone)]
pub enum PIIReplacement {
    /// Redact with asterisks
    Redact,
    /// Replace with placeholder
    Placeholder(String),
    /// Hash the value
    Hash,
    /// Remove entirely
    Remove,
    /// Mask partially (keep first/last N characters)
    PartialMask {
        /// Number of characters to keep at start
        keep_start: usize,
        /// Number of characters to keep at end
        keep_end: usize,
    },
}

/// Content moderation rule
#[derive(Debug, Clone)]
pub struct ModerationRule {
    /// Rule name
    pub name: String,
    /// Rule type
    pub rule_type: ModerationType,
    /// Action to take
    pub action: ModerationAction,
    /// Severity level
    pub severity: ModerationSeverity,
}

/// Types of content moderation
#[derive(Debug, Clone)]
pub enum ModerationType {
    /// Hate speech detection
    HateSpeech,
    /// Violence detection
    Violence,
    /// Sexual content detection
    Sexual,
    /// Self-harm detection
    SelfHarm,
    /// Harassment detection
    Harassment,
    /// Illegal activity detection
    IllegalActivity,
    /// Custom category
    Custom(String),
}

/// Actions to take when content is flagged
#[derive(Debug, Clone)]
pub enum ModerationAction {
    /// Block the request
    Block,
    /// Warn but allow
    Warn,
    /// Log for review
    Log,
    /// Modify content
    Modify,
    /// Require human review
    HumanReview,
}

/// Severity levels for moderation
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum ModerationSeverity {
    /// Low severity
    Low,
    /// Medium severity
    Medium,
    /// High severity
    High,
    /// Critical severity
    Critical,
}

/// Custom content filter
#[derive(Debug, Clone)]
pub struct CustomFilter {
    /// Filter name
    pub name: String,
    /// Filter pattern
    pub pattern: Regex,
    /// Filter action
    pub action: ModerationAction,
}

/// Content filtering result
#[derive(Debug, Clone)]
pub struct FilterResult {
    /// Whether content should be blocked
    pub blocked: bool,
    /// Detected issues
    pub issues: Vec<ContentIssue>,
    /// Modified content (if applicable)
    pub modified_content: Option<String>,
    /// Confidence score
    pub confidence: f64,
}

/// Detected content issue
#[derive(Debug, Clone)]
pub struct ContentIssue {
    /// Issue type
    pub issue_type: String,
    /// Issue description
    pub description: String,
    /// Severity level
    pub severity: ModerationSeverity,
    /// Location in text (start, end)
    pub location: Option<(usize, usize)>,
    /// Confidence score
    pub confidence: f64,
}

/// Data retention policy
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    /// Data type
    pub data_type: String,
    /// Retention period in days
    pub retention_days: u32,
    /// Auto-deletion enabled
    pub auto_delete: bool,
    /// Anonymization rules
    pub anonymization: Option<AnonymizationRule>,
}

/// Consent management
#[derive(Debug, Clone)]
pub struct ConsentManager {
    /// User consents
    pub(crate) consents: HashMap<String, UserConsent>,
}

/// User consent information
#[derive(Debug, Clone)]
pub struct UserConsent {
    /// User ID
    pub user_id: String,
    /// Consent given
    pub consented: bool,
    /// Consent timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Consent version
    pub version: String,
    /// Specific permissions
    pub permissions: Vec<String>,
}

/// Data export tools
#[derive(Debug, Clone)]
pub struct DataExportTools {
    /// Supported export formats
    pub(crate) formats: Vec<ExportFormat>,
}

/// Export formats
#[derive(Debug, Clone)]
pub enum ExportFormat {
    /// JSON format
    Json,
    /// CSV format
    Csv,
    /// XML format
    Xml,
    /// PDF format
    Pdf,
}

/// Anonymization rule
#[derive(Debug, Clone)]
pub struct AnonymizationRule {
    /// Fields to anonymize
    pub fields: Vec<String>,
    /// Anonymization method
    pub method: AnonymizationMethod,
}

/// Anonymization methods
#[derive(Debug, Clone)]
pub enum AnonymizationMethod {
    /// Replace with random data
    Randomize,
    /// Hash the data
    Hash,
    /// Remove the data
    Remove,
    /// Generalize the data
    Generalize,
}

// ====================================================================================
// TESTS
// ====================================================================================

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
