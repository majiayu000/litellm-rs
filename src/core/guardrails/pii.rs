//! PII (Personally Identifiable Information) detection and masking
//!
//! This module provides detection and masking of PII in text content.

use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;

use super::config::PIIConfig;
use super::traits::Guardrail;
use super::types::{
    CheckResult, GuardrailAction, GuardrailError, GuardrailResult, PIIMatch, PIIType, Violation,
    ViolationType,
};

/// PII detection and masking guardrail
pub struct PIIGuardrail {
    config: PIIConfig,
    patterns: HashMap<PIIType, Regex>,
}

impl PIIGuardrail {
    /// Create a new PII guardrail
    pub fn new(config: PIIConfig) -> GuardrailResult<Self> {
        let mut patterns = HashMap::new();

        // Build regex patterns for each PII type
        for pii_type in config.effective_types() {
            if let Some(pattern) = Self::get_pattern(&pii_type) {
                let regex = Regex::new(pattern).map_err(|e| {
                    GuardrailError::Config(format!("Invalid regex for {:?}: {}", pii_type, e))
                })?;
                patterns.insert(pii_type, regex);
            }
        }

        Ok(Self { config, patterns })
    }

    /// Get the regex pattern for a PII type
    fn get_pattern(pii_type: &PIIType) -> Option<&'static str> {
        match pii_type {
            PIIType::Email => Some(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b"),
            PIIType::Phone => Some(r"\b(?:\+?1[-.\s]?)?(?:\(?\d{3}\)?[-.\s]?)?\d{3}[-.\s]?\d{4}\b"),
            PIIType::CreditCard => Some(r"\b(?:\d{4}[-\s]?){3}\d{4}\b"),
            PIIType::SSN => Some(r"\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b"),
            PIIType::IpAddress => Some(
                r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b",
            ),
            PIIType::DateOfBirth => {
                Some(r"\b(?:0?[1-9]|1[0-2])[/\-](?:0?[1-9]|[12]\d|3[01])[/\-](?:19|20)\d{2}\b")
            }
            PIIType::Passport => Some(r"\b[A-Z]{1,2}\d{6,9}\b"),
            PIIType::DriversLicense => Some(r"\b[A-Z]{1,2}\d{5,8}\b"),
            PIIType::BankAccount => Some(r"\b\d{8,17}\b"),
            PIIType::Address => None, // Too complex for simple regex
            PIIType::Name => None,    // Requires NER
            PIIType::Custom(_) => None,
        }
    }

    /// Detect PII in text
    pub fn detect(&self, text: &str) -> Vec<PIIMatch> {
        let mut matches = Vec::new();

        for (pii_type, regex) in &self.patterns {
            for m in regex.find_iter(text) {
                let matched_text = m.as_str().to_string();

                // Check allow list
                if self.is_allowed(&matched_text) {
                    continue;
                }

                matches.push(PIIMatch::new(
                    pii_type.clone(),
                    m.start(),
                    m.end(),
                    matched_text,
                ));
            }
        }

        // Sort by position
        matches.sort_by_key(|m| m.start);
        matches
    }

    /// Check if a match is in the allow list
    fn is_allowed(&self, text: &str) -> bool {
        self.config
            .allow_list
            .iter()
            .any(|allowed| text.eq_ignore_ascii_case(allowed))
    }

    /// Mask PII in text
    pub fn mask(&self, text: &str, matches: &[PIIMatch]) -> String {
        if matches.is_empty() {
            return text.to_string();
        }

        let mut result = text.to_string();

        // Process matches in reverse order to preserve positions
        for m in matches.iter().rev() {
            let mask = self.get_mask(m);
            result.replace_range(m.start..m.end, &mask);
        }

        result
    }

    /// Get the mask string for a PII match
    fn get_mask(&self, pii_match: &PIIMatch) -> String {
        if let Some(ref pattern) = self.config.mask_pattern {
            pattern.clone()
        } else {
            // Create mask of same length
            let len = pii_match.end - pii_match.start;
            self.config.mask_char.to_string().repeat(len)
        }
    }

    /// Create violations from PII matches
    fn create_violations(&self, matches: &[PIIMatch]) -> Vec<Violation> {
        matches
            .iter()
            .map(|m| {
                Violation::new(
                    ViolationType::PII(m.pii_type.clone()),
                    format!("{:?} detected", m.pii_type),
                )
                .with_location(m.start, m.end)
                .with_severity(m.confidence)
            })
            .collect()
    }
}

#[async_trait]
impl Guardrail for PIIGuardrail {
    fn name(&self) -> &str {
        "pii_detection"
    }

    fn description(&self) -> &str {
        "Detect and mask personally identifiable information"
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    fn priority(&self) -> u32 {
        20 // Run after moderation
    }

    fn mask_content(&self, content: &str) -> GuardrailResult<Option<String>> {
        if !self.is_enabled() || self.config.action != GuardrailAction::Mask {
            return Ok(None);
        }
        let matches = self.detect(content);
        Ok((!matches.is_empty()).then(|| self.mask(content, &matches)))
    }

    async fn check_input(&self, content: &str) -> GuardrailResult<CheckResult> {
        if !self.is_enabled() {
            return Ok(CheckResult::pass());
        }

        let matches = self.detect(content);

        if matches.is_empty() {
            return Ok(CheckResult::pass());
        }

        let violations = self.create_violations(&matches);

        match self.config.action {
            GuardrailAction::Block => Ok(CheckResult::block(violations)),
            GuardrailAction::Mask => {
                let masked = self.mask(content, &matches);
                Ok(CheckResult::mask(masked, violations))
            }
            GuardrailAction::Log => {
                let mut result = CheckResult::pass();
                result.violations = violations;
                result.action = GuardrailAction::Log;
                Ok(result)
            }
            GuardrailAction::Allow => Ok(CheckResult::pass()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_guardrail() -> PIIGuardrail {
        let config = PIIConfig {
            enabled: true,
            action: GuardrailAction::Mask,
            ..Default::default()
        };
        PIIGuardrail::new(config).unwrap()
    }

    #[test]
    fn test_guardrail_creation() {
        let guardrail = create_test_guardrail();
        assert_eq!(guardrail.name(), "pii_detection");
        assert!(guardrail.is_enabled());
    }

    #[test]
    fn test_detect_email() {
        let guardrail = create_test_guardrail();
        let text = "Contact me at test@example.com for more info.";
        let matches = guardrail.detect(text);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PIIType::Email);
        assert_eq!(matches[0].text, "test@example.com");
    }

    #[test]
    fn test_detect_phone() {
        let guardrail = create_test_guardrail();
        let text = "Call me at 555-123-4567 or (555) 987-6543.";
        let matches = guardrail.detect(text);

        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|m| m.pii_type == PIIType::Phone));
    }

    #[test]
    fn test_detect_credit_card() {
        let guardrail = create_test_guardrail();
        let text = "My card number is 4111-1111-1111-1111.";
        let matches = guardrail.detect(text);

        // Should detect credit card (may also match other patterns)
        assert!(matches.iter().any(|m| m.pii_type == PIIType::CreditCard));
    }

    #[test]
    fn test_detect_ssn() {
        let guardrail = create_test_guardrail();
        let text = "SSN: 123-45-6789";
        let matches = guardrail.detect(text);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PIIType::SSN);
    }

    #[test]
    fn test_detect_ip_address() {
        let guardrail = create_test_guardrail();
        let text = "Server IP: 192.168.1.100";
        let matches = guardrail.detect(text);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PIIType::IpAddress);
    }

    #[test]
    fn test_detect_multiple_pii() {
        let guardrail = create_test_guardrail();
        let text = "Email: user@test.com, Phone: 555-123-4567, SSN: 123-45-6789";
        let matches = guardrail.detect(text);

        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_mask_with_char() {
        let config = PIIConfig {
            enabled: true,
            action: GuardrailAction::Mask,
            mask_char: '*',
            mask_pattern: None,
            ..Default::default()
        };
        let guardrail = PIIGuardrail::new(config).unwrap();

        let text = "Email: test@example.com";
        let matches = guardrail.detect(text);
        let masked = guardrail.mask(text, &matches);

        assert!(masked.contains("****************"));
        assert!(!masked.contains("test@example.com"));
    }

    #[test]
    fn test_mask_with_pattern() {
        let config = PIIConfig {
            enabled: true,
            action: GuardrailAction::Mask,
            mask_pattern: Some("[REDACTED]".to_string()),
            ..Default::default()
        };
        let guardrail = PIIGuardrail::new(config).unwrap();

        let text = "Email: test@example.com";
        let matches = guardrail.detect(text);
        let masked = guardrail.mask(text, &matches);

        assert!(masked.contains("[REDACTED]"));
        assert!(!masked.contains("test@example.com"));
    }

    #[test]
    fn test_allow_list() {
        let config = PIIConfig {
            enabled: true,
            action: GuardrailAction::Mask,
            allow_list: vec!["allowed@example.com".to_string()],
            ..Default::default()
        };
        let guardrail = PIIGuardrail::new(config).unwrap();

        let text = "Contact: allowed@example.com or other@example.com";
        let matches = guardrail.detect(text);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].text, "other@example.com");
    }

    #[test]
    fn test_no_pii() {
        let guardrail = create_test_guardrail();
        let text = "This is a normal message without any PII.";
        let matches = guardrail.detect(text);

        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn test_check_input_block() {
        let config = PIIConfig {
            enabled: true,
            action: GuardrailAction::Block,
            ..Default::default()
        };
        let guardrail = PIIGuardrail::new(config).unwrap();

        let result = guardrail
            .check_input("Email: test@example.com")
            .await
            .unwrap();

        assert!(result.is_blocked());
        assert_eq!(result.violations.len(), 1);
    }

    #[tokio::test]
    async fn test_check_input_mask() {
        let config = PIIConfig {
            enabled: true,
            action: GuardrailAction::Mask,
            mask_pattern: Some("[EMAIL]".to_string()),
            ..Default::default()
        };
        let guardrail = PIIGuardrail::new(config).unwrap();

        let result = guardrail
            .check_input("Email: test@example.com")
            .await
            .unwrap();

        assert!(!result.is_blocked());
        assert!(result.is_modified());
        assert!(result.modified_content.unwrap().contains("[EMAIL]"));
    }

    #[tokio::test]
    async fn test_check_input_log() {
        let config = PIIConfig {
            enabled: true,
            action: GuardrailAction::Log,
            ..Default::default()
        };
        let guardrail = PIIGuardrail::new(config).unwrap();

        let result = guardrail
            .check_input("Email: test@example.com")
            .await
            .unwrap();

        assert!(result.passed);
        assert!(!result.is_blocked());
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.action, GuardrailAction::Log);
    }

    #[tokio::test]
    async fn test_check_input_disabled() {
        let config = PIIConfig {
            enabled: false,
            ..Default::default()
        };
        let guardrail = PIIGuardrail::new(config).unwrap();

        let result = guardrail
            .check_input("Email: test@example.com")
            .await
            .unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_specific_pii_types() {
        let config = PIIConfig {
            enabled: true,
            types: [PIIType::Email].into_iter().collect(),
            ..Default::default()
        };
        let guardrail = PIIGuardrail::new(config).unwrap();

        let text = "Email: test@example.com, Phone: 555-123-4567";
        let matches = guardrail.detect(text);

        // Should only detect email, not phone
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PIIType::Email);
    }
}
