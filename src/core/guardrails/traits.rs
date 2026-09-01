//! Guardrail trait definition
//!
//! This module defines the core trait that all guardrails must implement.

use async_trait::async_trait;

use super::types::{CheckResult, GuardrailResult};

/// Core trait for all guardrails
///
/// Implement this trait to create custom guardrails that can be
/// plugged into the guardrail engine.
#[async_trait]
pub trait Guardrail: Send + Sync {
    /// Get the name of this guardrail
    fn name(&self) -> &str;

    /// Get a description of what this guardrail does
    fn description(&self) -> &str {
        "No description provided"
    }

    /// Check if this guardrail is enabled
    fn is_enabled(&self) -> bool {
        true
    }

    /// Check input content (request)
    ///
    /// Returns a `CheckResult` indicating whether the content passed
    /// and any violations detected.
    async fn check_input(&self, content: &str) -> GuardrailResult<CheckResult>;

    /// Check output content (response)
    ///
    /// By default, this calls `check_input`. Override if output checking
    /// should behave differently.
    async fn check_output(&self, content: &str) -> GuardrailResult<CheckResult> {
        self.check_input(content).await
    }

    /// Apply this guardrail's configured mask without running unrelated checks.
    fn mask_content(&self, _content: &str) -> GuardrailResult<Option<String>> {
        Ok(None)
    }

    /// Get the priority of this guardrail (lower = higher priority)
    ///
    /// Guardrails are executed in priority order. Default is 100.
    fn priority(&self) -> u32 {
        100
    }
}

/// A boxed guardrail for dynamic dispatch
pub type BoxedGuardrail = Box<dyn Guardrail>;

/// Extension trait for guardrail collections
pub trait GuardrailExt {
    /// Sort guardrails by priority
    fn sort_by_priority(&mut self);
}

impl GuardrailExt for Vec<BoxedGuardrail> {
    fn sort_by_priority(&mut self) {
        self.sort_by_key(|g| g.priority());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::guardrails::types::CheckResult;

    struct TestGuardrail {
        name: String,
        enabled: bool,
        priority: u32,
    }

    #[async_trait]
    impl Guardrail for TestGuardrail {
        fn name(&self) -> &str {
            &self.name
        }

        fn is_enabled(&self) -> bool {
            self.enabled
        }

        fn priority(&self) -> u32 {
            self.priority
        }

        async fn check_input(&self, _content: &str) -> GuardrailResult<CheckResult> {
            Ok(CheckResult::pass())
        }
    }

    #[test]
    fn test_guardrail_trait() {
        let guardrail = TestGuardrail {
            name: "test".to_string(),
            enabled: true,
            priority: 50,
        };

        assert_eq!(guardrail.name(), "test");
        assert!(guardrail.is_enabled());
        assert_eq!(guardrail.priority(), 50);
        assert_eq!(guardrail.description(), "No description provided");
    }

    #[test]
    fn test_guardrail_sort_by_priority() {
        let g1 = TestGuardrail {
            name: "low".to_string(),
            enabled: true,
            priority: 100,
        };
        let g2 = TestGuardrail {
            name: "high".to_string(),
            enabled: true,
            priority: 10,
        };
        let g3 = TestGuardrail {
            name: "medium".to_string(),
            enabled: true,
            priority: 50,
        };

        let mut guardrails: Vec<BoxedGuardrail> = vec![Box::new(g1), Box::new(g2), Box::new(g3)];

        guardrails.sort_by_priority();

        assert_eq!(guardrails[0].name(), "high");
        assert_eq!(guardrails[1].name(), "medium");
        assert_eq!(guardrails[2].name(), "low");
    }

    #[tokio::test]
    async fn test_guardrail_check() {
        let guardrail = TestGuardrail {
            name: "test".to_string(),
            enabled: true,
            priority: 100,
        };

        let result = guardrail.check_input("test content").await.unwrap();
        assert!(result.passed);

        // check_output defaults to check_input
        let result = guardrail.check_output("test content").await.unwrap();
        assert!(result.passed);
    }
}
