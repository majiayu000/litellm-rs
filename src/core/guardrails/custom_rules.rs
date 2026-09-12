//! Custom regex rules
//!
//! Compiles configured `custom_rules` with the same `regex` crate used by PII
//! and prompt-injection, then evaluates them through the shared `Guardrail`
//! trait. Mask is not implemented; gateway validation rejects it.
//!
//! Operator-supplied patterns are untrusted: compile with length, nest, and
//! compiled-size caps so a single pathological rule cannot blow up heap or
//! stall the fleet at config apply time. Match-time budgets are deferred;
//! `regex` matching is linear in the haystack once compilation succeeds.

use async_trait::async_trait;
use regex::{Regex, RegexBuilder};
use tracing::debug;

use super::config::CustomRuleConfig;
use super::traits::Guardrail;
use super::types::{
    CheckResult, GuardrailAction, GuardrailError, GuardrailResult, Violation, ViolationType,
};

/// Cap on pattern string length (bytes). Keeps AST/heap proportional to input.
const MAX_CUSTOM_RULE_PATTERN_LEN: usize = 512;
/// Cap on compiled regex heap (~bytes). Default regex crate limit is ~10 MiB.
const CUSTOM_RULE_REGEX_SIZE_LIMIT: usize = 100 * 1024;
/// Cap on AST nesting depth. Default regex nest limit is 250.
const CUSTOM_RULE_REGEX_NEST_LIMIT: u32 = 32;

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

fn compile_bounded_pattern(pattern: &str) -> Result<Regex, String> {
    if pattern.len() > MAX_CUSTOM_RULE_PATTERN_LEN {
        return Err(format!(
            "pattern exceeds maximum length of {MAX_CUSTOM_RULE_PATTERN_LEN} bytes (got {})",
            pattern.len()
        ));
    }
    RegexBuilder::new(pattern)
        .size_limit(CUSTOM_RULE_REGEX_SIZE_LIMIT)
        .nest_limit(CUSTOM_RULE_REGEX_NEST_LIMIT)
        .build()
        .map_err(|error| error.to_string())
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
            let regex = compile_bounded_pattern(pattern).map_err(|error| {
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
    fn rejects_oversized_pattern_length() {
        let pattern = "a".repeat(MAX_CUSTOM_RULE_PATTERN_LEN + 1);
        let error = match CustomRulesGuardrail::try_from_config(&[rule(
            "too-long",
            &pattern,
            GuardrailAction::Block,
        )]) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("oversized pattern must fail closed"),
        };
        assert!(error.contains("too-long"), "{error}");
        assert!(error.contains("maximum length"), "{error}");
    }

    #[test]
    fn rejects_pathological_compile_size_blowup() {
        // Counted unicode class expands into a large compiled automaton.
        let error = match CustomRulesGuardrail::try_from_config(&[rule(
            "blowup",
            r"\p{L}{1000}",
            GuardrailAction::Block,
        )]) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("size-blowup pattern must fail closed"),
        };
        assert!(error.contains("blowup"), "{error}");
        assert!(
            error.contains("size limit") || error.contains("Compiled regex exceeds"),
            "{error}"
        );
    }

    #[test]
    fn rejects_excessive_nesting() {
        let pattern = format!("{}a{}", "(".repeat(33), ")".repeat(33));
        let error = match CustomRulesGuardrail::try_from_config(&[rule(
            "nested",
            &pattern,
            GuardrailAction::Block,
        )]) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("deeply nested pattern must fail closed"),
        };
        assert!(error.contains("nested"), "{error}");
        assert!(
            error.contains("nested parentheses") || error.contains("nest"),
            "{error}"
        );
    }

    #[test]
    fn validate_custom_rule_patterns_fail_closed_on_pathological() {
        let err = validate_custom_rule_patterns(&[rule(
            "blowup",
            r"\p{L}{1000}",
            GuardrailAction::Block,
        )])
        .expect_err("pathological pattern must fail validation");
        assert!(err.contains("blowup"), "{err}");
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
