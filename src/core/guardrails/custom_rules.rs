//! Custom regex rules
//!
//! Compiles configured `custom_rules` with the same `regex` crate used by PII
//! and prompt-injection, then evaluates them through the shared `Guardrail`
//! trait. Mask is not implemented; gateway validation rejects it.

use async_trait::async_trait;
use regex::Regex;
use tracing::debug;

use super::config::CustomRuleConfig;
use super::traits::Guardrail;
use super::types::{
    CheckResult, GuardrailAction, GuardrailError, GuardrailResult, Violation, ViolationType,
};

struct CompiledCustomRule {
    name: String,
    action: GuardrailAction,
    message: Option<String>,
    patterns: Vec<Regex>,
}

/// One guardrail that owns every enabled custom rule.
pub(crate) struct CustomRulesGuardrail {
    rules: Vec<CompiledCustomRule>,
}

/// Compile enabled custom rules. Invalid patterns name the rule and pattern.
fn compile_custom_rule_patterns(
    rules: &[CustomRuleConfig],
) -> GuardrailResult<Vec<CompiledCustomRule>> {
    let mut compiled = Vec::new();
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        let mut patterns = Vec::with_capacity(rule.patterns.len());
        for pattern in &rule.patterns {
            let regex = Regex::new(pattern).map_err(|error| {
                GuardrailError::Config(format!(
                    "Invalid custom rule '{}' pattern '{}': {error}",
                    rule.name, pattern
                ))
            })?;
            patterns.push(regex);
        }
        compiled.push(CompiledCustomRule {
            name: rule.name.clone(),
            action: rule.action,
            message: rule.message.clone(),
            patterns,
        });
    }
    Ok(compiled)
}

/// Fail closed on invalid enabled custom-rule patterns, naming the rule.
pub(crate) fn validate_custom_rule_patterns(rules: &[CustomRuleConfig]) -> Result<(), String> {
    compile_custom_rule_patterns(rules).map_err(|error| match error {
        GuardrailError::Config(message) => message,
        other => other.to_string(),
    })?;
    Ok(())
}

impl CustomRulesGuardrail {
    /// Build a guardrail when at least one enabled rule compiles.
    pub(crate) fn try_from_config(rules: &[CustomRuleConfig]) -> GuardrailResult<Option<Self>> {
        let rules = compile_custom_rule_patterns(rules)?;
        if rules.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Self { rules }))
        }
    }

    fn evaluate(&self, content: &str) -> CheckResult {
        let mut violations = Vec::new();
        let mut block = false;
        let mut log = false;

        for rule in &self.rules {
            if !rule
                .patterns
                .iter()
                .any(|pattern| pattern.is_match(content))
            {
                continue;
            }

            match rule.action {
                GuardrailAction::Allow | GuardrailAction::Mask => {}
                GuardrailAction::Block | GuardrailAction::Log => {
                    debug!(
                        rule = %rule.name,
                        action = ?rule.action,
                        "Custom guardrail rule matched"
                    );
                    if rule.action == GuardrailAction::Block {
                        block = true;
                    } else {
                        log = true;
                    }
                    let message = rule
                        .message
                        .clone()
                        .unwrap_or_else(|| format!("Custom rule '{}' matched", rule.name));
                    violations.push(Violation::new(
                        ViolationType::CustomRule(rule.name.clone()),
                        message,
                    ));
                }
            }
        }

        if block {
            CheckResult::block(violations)
        } else if log {
            let mut result = CheckResult::pass();
            result.violations = violations;
            result.action = GuardrailAction::Log;
            result
        } else {
            CheckResult::pass()
        }
    }
}

#[async_trait]
impl Guardrail for CustomRulesGuardrail {
    fn name(&self) -> &str {
        "custom_rules"
    }

    fn description(&self) -> &str {
        "Evaluate configured custom regex rules"
    }

    fn is_enabled(&self) -> bool {
        !self.rules.is_empty()
    }

    fn priority(&self) -> u32 {
        20
    }

    async fn check_input(&self, content: &str) -> GuardrailResult<CheckResult> {
        Ok(self.evaluate(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name: &str, pattern: &str, action: GuardrailAction) -> CustomRuleConfig {
        CustomRuleConfig {
            name: name.to_string(),
            description: None,
            enabled: true,
            patterns: vec![pattern.to_string()],
            action,
            message: None,
        }
    }

    fn guardrail(rules: Vec<CustomRuleConfig>) -> CustomRulesGuardrail {
        CustomRulesGuardrail::try_from_config(&rules)
            .expect("test rules must compile")
            .expect("test rules must produce a guardrail")
    }

    #[test]
    fn invalid_pattern_names_the_rule() {
        let error = match CustomRulesGuardrail::try_from_config(&[rule(
            "no-secrets",
            "[",
            GuardrailAction::Block,
        )]) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("unterminated character class must fail"),
        };
        assert!(error.contains("no-secrets"), "{error}");
        assert!(error.contains("pattern '['"), "{error}");
    }

    #[test]
    fn disabled_rules_are_skipped() {
        let mut disabled = rule("no-secrets", "[", GuardrailAction::Block);
        disabled.enabled = false;
        assert!(
            CustomRulesGuardrail::try_from_config(&[disabled])
                .expect("disabled invalid rules are skipped")
                .is_none()
        );
    }

    #[tokio::test]
    async fn block_log_and_allow_map_like_prompt_injection() {
        let content = "hello forbidden-token-xyz";

        let blocked = guardrail(vec![rule(
            "deny-token",
            "forbidden-token-xyz",
            GuardrailAction::Block,
        )])
        .check_input(content)
        .await
        .unwrap();
        assert!(blocked.is_blocked());
        assert_eq!(
            blocked.violations[0].violation_type,
            ViolationType::CustomRule("deny-token".to_string())
        );
        assert!(!blocked.violations[0].details.contains_key("matched_text"));
        assert!(
            !blocked.violations[0]
                .message
                .contains("forbidden-token-xyz")
        );

        let logged = guardrail(vec![rule(
            "log-token",
            "forbidden-token-xyz",
            GuardrailAction::Log,
        )])
        .check_input(content)
        .await
        .unwrap();
        assert!(logged.passed);
        assert!(!logged.is_blocked());
        assert_eq!(logged.action, GuardrailAction::Log);
        assert_eq!(logged.violations.len(), 1);

        let allowed = guardrail(vec![rule(
            "allow-token",
            "forbidden-token-xyz",
            GuardrailAction::Allow,
        )])
        .check_input(content)
        .await
        .unwrap();
        assert!(allowed.passed);
        assert!(!allowed.is_blocked());
        assert!(allowed.violations.is_empty());
    }

    #[tokio::test]
    async fn unmatched_content_passes() {
        let result = guardrail(vec![rule(
            "deny-token",
            "forbidden-token-xyz",
            GuardrailAction::Block,
        )])
        .check_input("hello")
        .await
        .unwrap();
        assert!(result.passed);
        assert!(result.violations.is_empty());
    }
}
