//! Fallback configuration and execution result types
//!
//! This module defines fallback configuration for error handling
//! and execution result metadata.

use super::deployment::DeploymentId;
use std::collections::HashMap;
use std::sync::RwLock;

/// Fallback type enumeration
///
/// Defines different types of fallback scenarios that can trigger alternative model selection.
/// Each type corresponds to a specific error condition and has its own fallback mapping.
///
/// ## Fallback Priority
///
/// When determining fallback models, the router checks in this order:
/// 1. Specific fallback type (ContextWindow, ContentPolicy, RateLimit)
/// 2. General fallback (if no specific type matches)
/// 3. Empty list (no fallback available)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackType {
    /// General fallback for any error
    General,
    /// Context window exceeded - model cannot handle the input size
    ContextWindow,
    /// Content policy violation - content was filtered/rejected
    ContentPolicy,
    /// Rate limit exceeded - too many requests
    RateLimit,
}

/// Execution result with metadata
///
/// Contains the result of a successful execution along with metadata about
/// the execution such as which deployment was used, how many attempts were made,
/// and whether fallback was used.
///
/// # Type Parameters
///
/// * `T` - The type of the result value
#[derive(Debug, Clone)]
pub struct ExecutionResult<T> {
    /// The successful result value
    pub result: T,
    /// The deployment ID that successfully handled the request
    pub deployment_id: DeploymentId,
    /// Total number of attempts across all retries and fallbacks
    pub attempts: u32,
    /// The actual model that was used (may differ from requested if fallback occurred)
    pub model_used: String,
    /// Whether a fallback model was used (true if not the original model)
    pub used_fallback: bool,
    /// Total execution latency in microseconds (including retries)
    pub latency_us: u64,
}

/// Fallback configuration
///
/// Manages fallback mappings for different error types. Each model can have different
/// fallback models configured for different scenarios.
///
/// ## Thread Safety
///
/// Uses `RwLock` to allow concurrent reads and exclusive writes.
#[derive(Debug, Default)]
pub struct FallbackConfig {
    /// General fallback: model_name -> fallback model_names
    general: RwLock<HashMap<String, Vec<String>>>,

    /// Context window exceeded fallback
    context_window: RwLock<HashMap<String, Vec<String>>>,

    /// Content policy violation fallback
    content_policy: RwLock<HashMap<String, Vec<String>>>,

    /// Rate limit exceeded fallback
    rate_limit: RwLock<HashMap<String, Vec<String>>>,
}

impl FallbackConfig {
    /// Create a new empty fallback configuration
    pub fn new() -> Self {
        Self {
            general: RwLock::new(HashMap::new()),
            context_window: RwLock::new(HashMap::new()),
            content_policy: RwLock::new(HashMap::new()),
            rate_limit: RwLock::new(HashMap::new()),
        }
    }

    /// Add general fallback models for a model (builder pattern)
    ///
    /// General fallbacks are used when no specific fallback type matches the error.
    ///
    pub fn add_general(self, model: &str, fallbacks: Vec<String>) -> Self {
        self.general
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(model.to_string(), fallbacks);
        self
    }

    /// Add context window fallback models for a model (builder pattern)
    ///
    /// Context window fallbacks are used when the input exceeds the model's maximum context length.
    ///
    pub fn add_context_window(self, model: &str, fallbacks: Vec<String>) -> Self {
        self.context_window
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(model.to_string(), fallbacks);
        self
    }

    /// Add content policy fallback models for a model (builder pattern)
    ///
    /// Content policy fallbacks are used when content is filtered or rejected by safety systems.
    ///
    pub fn add_content_policy(self, model: &str, fallbacks: Vec<String>) -> Self {
        self.content_policy
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(model.to_string(), fallbacks);
        self
    }

    /// Add rate limit fallback models for a model (builder pattern)
    ///
    /// Rate limit fallbacks are used when the model's rate limit is exceeded.
    ///
    pub fn add_rate_limit(self, model: &str, fallbacks: Vec<String>) -> Self {
        self.rate_limit
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(model.to_string(), fallbacks);
        self
    }

    /// Validate fallback configuration for cycles
    ///
    /// Runs DFS on every fallback map and returns a list of cycle descriptions.
    /// An empty `Ok(())` means no cycles were found.
    #[allow(clippy::type_complexity)]
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        let maps: [(&str, &RwLock<HashMap<String, Vec<String>>>); 4] = [
            ("general", &self.general),
            ("context_window", &self.context_window),
            ("content_policy", &self.content_policy),
            ("rate_limit", &self.rate_limit),
        ];

        for (label, lock) in &maps {
            let map = lock.read().unwrap_or_else(|e| e.into_inner());
            for start in map.keys() {
                let mut visited = std::collections::HashSet::new();
                visited.insert(start.clone());
                let mut stack = vec![start.clone()];

                while let Some(node) = stack.pop() {
                    if let Some(targets) = map.get(&node) {
                        for target in targets {
                            if !visited.insert(target.clone()) {
                                errors.push(format!(
                                    "{label}: cycle involving '{start}' -> '{target}'"
                                ));
                            } else {
                                stack.push(target.clone());
                            }
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Get fallback models for a specific type
    ///
    /// Returns a cloned vector of fallback model names. Returns empty vector if no fallbacks
    /// are configured for the given model and type.
    ///
    pub fn get_fallbacks_for_type(
        &self,
        model_name: &str,
        fallback_type: FallbackType,
    ) -> Vec<String> {
        let lock = match fallback_type {
            FallbackType::General => &self.general,
            FallbackType::ContextWindow => &self.context_window,
            FallbackType::ContentPolicy => &self.content_policy,
            FallbackType::RateLimit => &self.rate_limit,
        };

        lock.read()
            .unwrap_or_else(|e| e.into_inner())
            .get(model_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Replace every fallback map with the maps from `other`.
    pub fn overwrite(&self, other: Self) {
        replace_fallback_map(&self.general, other.general);
        replace_fallback_map(&self.context_window, other.context_window);
        replace_fallback_map(&self.content_policy, other.content_policy);
        replace_fallback_map(&self.rate_limit, other.rate_limit);
    }
}

fn replace_fallback_map(
    dst: &RwLock<HashMap<String, Vec<String>>>,
    src: RwLock<HashMap<String, Vec<String>>>,
) {
    *dst.write().unwrap_or_else(|e| e.into_inner()) =
        src.into_inner().unwrap_or_else(|e| e.into_inner());
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== FallbackType Tests ====================

    #[test]
    fn test_fallback_type_debug() {
        assert!(format!("{:?}", FallbackType::General).contains("General"));
        assert!(format!("{:?}", FallbackType::ContextWindow).contains("ContextWindow"));
        assert!(format!("{:?}", FallbackType::ContentPolicy).contains("ContentPolicy"));
        assert!(format!("{:?}", FallbackType::RateLimit).contains("RateLimit"));
    }

    #[test]
    fn test_fallback_type_clone() {
        let t = FallbackType::ContextWindow;
        let cloned = t;
        assert_eq!(cloned, FallbackType::ContextWindow);
    }

    #[test]
    fn test_fallback_type_copy() {
        let t = FallbackType::RateLimit;
        let copied = t;
        assert_eq!(t, copied);
    }

    #[test]
    fn test_fallback_type_eq() {
        assert_eq!(FallbackType::General, FallbackType::General);
        assert_ne!(FallbackType::General, FallbackType::ContextWindow);
        assert_ne!(FallbackType::ContextWindow, FallbackType::ContentPolicy);
        assert_ne!(FallbackType::ContentPolicy, FallbackType::RateLimit);
    }

    #[test]
    fn overwrite_replaces_every_fallback_map() {
        let config = FallbackConfig::new().add_general("gpt-4o", vec!["old".to_string()]);
        config.overwrite(
            FallbackConfig::new().add_content_policy("gpt-4o", vec!["safe".to_string()]),
        );
        assert!(
            config
                .get_fallbacks_for_type("gpt-4o", FallbackType::General)
                .is_empty()
        );
        assert_eq!(
            config.get_fallbacks_for_type("gpt-4o", FallbackType::ContentPolicy),
            vec!["safe".to_string()]
        );
    }

    // ==================== ExecutionResult Tests ====================

    #[test]
    fn test_execution_result_creation() {
        let result = ExecutionResult {
            result: "success".to_string(),
            deployment_id: "openai-gpt-4".to_string(),
            attempts: 1,
            model_used: "gpt-4".to_string(),
            used_fallback: false,
            latency_us: 1500,
        };

        assert_eq!(result.result, "success");
        assert_eq!(result.attempts, 1);
        assert_eq!(result.model_used, "gpt-4");
        assert!(!result.used_fallback);
        assert_eq!(result.latency_us, 1500);
    }

    #[test]
    fn test_execution_result_with_fallback() {
        let result = ExecutionResult {
            result: 42,
            deployment_id: "anthropic-claude-3".to_string(),
            attempts: 3,
            model_used: "claude-3".to_string(),
            used_fallback: true,
            latency_us: 5000,
        };

        assert_eq!(result.result, 42);
        assert_eq!(result.attempts, 3);
        assert!(result.used_fallback);
    }

    #[test]
    fn test_execution_result_debug() {
        let result = ExecutionResult {
            result: "test",
            deployment_id: "openai-gpt-4".to_string(),
            attempts: 1,
            model_used: "gpt-4".to_string(),
            used_fallback: false,
            latency_us: 100,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("ExecutionResult"));
        assert!(debug.contains("attempts"));
    }

    #[test]
    fn test_execution_result_clone() {
        let result = ExecutionResult {
            result: vec![1, 2, 3],
            deployment_id: "openai-gpt-4".to_string(),
            attempts: 2,
            model_used: "gpt-4".to_string(),
            used_fallback: true,
            latency_us: 2000,
        };
        let cloned = result.clone();
        assert_eq!(cloned.result, vec![1, 2, 3]);
        assert_eq!(cloned.attempts, 2);
        assert!(cloned.used_fallback);
    }

    // ==================== FallbackConfig Tests ====================

    #[test]
    fn test_fallback_config_new() {
        let config = FallbackConfig::new();
        assert!(
            config
                .get_fallbacks_for_type("gpt-4", FallbackType::General)
                .is_empty()
        );
    }

    #[test]
    fn test_fallback_config_default() {
        let config = FallbackConfig::default();
        assert!(
            config
                .get_fallbacks_for_type("gpt-4", FallbackType::General)
                .is_empty()
        );
    }

    #[test]
    fn test_fallback_config_debug() {
        let config = FallbackConfig::new();
        let debug = format!("{:?}", config);
        assert!(debug.contains("FallbackConfig"));
    }

    #[test]
    fn test_add_general_fallback() {
        let config = FallbackConfig::new().add_general(
            "gpt-4",
            vec!["gpt-3.5-turbo".to_string(), "claude-3".to_string()],
        );

        let fallbacks = config.get_fallbacks_for_type("gpt-4", FallbackType::General);
        assert_eq!(fallbacks.len(), 2);
        assert!(fallbacks.contains(&"gpt-3.5-turbo".to_string()));
        assert!(fallbacks.contains(&"claude-3".to_string()));
    }

    #[test]
    fn test_add_context_window_fallback() {
        let config =
            FallbackConfig::new().add_context_window("gpt-4", vec!["gpt-4-32k".to_string()]);

        let fallbacks = config.get_fallbacks_for_type("gpt-4", FallbackType::ContextWindow);
        assert_eq!(fallbacks.len(), 1);
        assert_eq!(fallbacks[0], "gpt-4-32k");
    }

    #[test]
    fn test_add_content_policy_fallback() {
        let config =
            FallbackConfig::new().add_content_policy("gpt-4", vec!["claude-3".to_string()]);

        let fallbacks = config.get_fallbacks_for_type("gpt-4", FallbackType::ContentPolicy);
        assert_eq!(fallbacks.len(), 1);
        assert_eq!(fallbacks[0], "claude-3");
    }

    #[test]
    fn test_add_rate_limit_fallback() {
        let config = FallbackConfig::new().add_rate_limit(
            "gpt-4",
            vec!["gpt-3.5-turbo".to_string(), "gpt-4-turbo".to_string()],
        );

        let fallbacks = config.get_fallbacks_for_type("gpt-4", FallbackType::RateLimit);
        assert_eq!(fallbacks.len(), 2);
    }

    #[test]
    fn test_builder_pattern_chaining() {
        let config = FallbackConfig::new()
            .add_general("gpt-4", vec!["gpt-3.5-turbo".to_string()])
            .add_context_window("gpt-4", vec!["gpt-4-32k".to_string()])
            .add_content_policy("gpt-4", vec!["claude-3".to_string()])
            .add_rate_limit("gpt-4", vec!["gemini".to_string()]);

        assert_eq!(
            config
                .get_fallbacks_for_type("gpt-4", FallbackType::General)
                .len(),
            1
        );
        assert_eq!(
            config
                .get_fallbacks_for_type("gpt-4", FallbackType::ContextWindow)
                .len(),
            1
        );
        assert_eq!(
            config
                .get_fallbacks_for_type("gpt-4", FallbackType::ContentPolicy)
                .len(),
            1
        );
        assert_eq!(
            config
                .get_fallbacks_for_type("gpt-4", FallbackType::RateLimit)
                .len(),
            1
        );
    }

    #[test]
    fn test_multiple_models() {
        let config = FallbackConfig::new()
            .add_general("gpt-4", vec!["gpt-3.5-turbo".to_string()])
            .add_general("claude-3", vec!["gemini".to_string()]);

        let gpt4_fallbacks = config.get_fallbacks_for_type("gpt-4", FallbackType::General);
        let claude_fallbacks = config.get_fallbacks_for_type("claude-3", FallbackType::General);

        assert_eq!(gpt4_fallbacks, vec!["gpt-3.5-turbo".to_string()]);
        assert_eq!(claude_fallbacks, vec!["gemini".to_string()]);
    }

    #[test]
    fn test_get_fallbacks_nonexistent_model() {
        let config = FallbackConfig::new().add_general("gpt-4", vec!["gpt-3.5-turbo".to_string()]);

        let fallbacks = config.get_fallbacks_for_type("nonexistent", FallbackType::General);
        assert!(fallbacks.is_empty());
    }

    #[test]
    fn test_get_fallbacks_wrong_type() {
        let config = FallbackConfig::new().add_general("gpt-4", vec!["gpt-3.5-turbo".to_string()]);

        // No context window fallback configured
        let fallbacks = config.get_fallbacks_for_type("gpt-4", FallbackType::ContextWindow);
        assert!(fallbacks.is_empty());
    }

    #[test]
    fn test_empty_fallback_list() {
        let config = FallbackConfig::new().add_general("gpt-4", vec![]);

        let fallbacks = config.get_fallbacks_for_type("gpt-4", FallbackType::General);
        assert!(fallbacks.is_empty());
    }

    #[test]
    fn test_override_fallback() {
        let config = FallbackConfig::new()
            .add_general("gpt-4", vec!["first".to_string()])
            .add_general("gpt-4", vec!["second".to_string()]);

        let fallbacks = config.get_fallbacks_for_type("gpt-4", FallbackType::General);
        assert_eq!(fallbacks, vec!["second".to_string()]);
    }

    // ==================== Thread Safety Tests ====================

    #[test]
    fn test_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;

        let config =
            Arc::new(FallbackConfig::new().add_general("gpt-4", vec!["gpt-3.5".to_string()]));

        let mut handles = vec![];

        for _ in 0..10 {
            let c = config.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let _ = c.get_fallbacks_for_type("gpt-4", FallbackType::General);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_special_characters_in_model_name() {
        let config =
            FallbackConfig::new().add_general("model/v2.0:latest", vec!["backup".to_string()]);

        let fallbacks = config.get_fallbacks_for_type("model/v2.0:latest", FallbackType::General);
        assert_eq!(fallbacks.len(), 1);
    }

    #[test]
    fn test_unicode_in_model_name() {
        let config = FallbackConfig::new().add_general("模型", vec!["备份".to_string()]);

        let fallbacks = config.get_fallbacks_for_type("模型", FallbackType::General);
        assert_eq!(fallbacks, vec!["备份".to_string()]);
    }

    #[test]
    fn test_empty_model_name() {
        let config = FallbackConfig::new().add_general("", vec!["fallback".to_string()]);

        let fallbacks = config.get_fallbacks_for_type("", FallbackType::General);
        assert_eq!(fallbacks, vec!["fallback".to_string()]);
    }

    #[test]
    fn test_many_fallbacks() {
        let fallbacks: Vec<String> = (0..100).map(|i| format!("model_{}", i)).collect();
        let config = FallbackConfig::new().add_general("primary", fallbacks.clone());

        let result = config.get_fallbacks_for_type("primary", FallbackType::General);
        assert_eq!(result.len(), 100);
        assert_eq!(result[0], "model_0");
        assert_eq!(result[99], "model_99");
    }

    #[test]
    fn test_fallback_type_all_variants() {
        let config = FallbackConfig::new()
            .add_general("model", vec!["g".to_string()])
            .add_context_window("model", vec!["cw".to_string()])
            .add_content_policy("model", vec!["cp".to_string()])
            .add_rate_limit("model", vec!["rl".to_string()]);

        assert_eq!(
            config.get_fallbacks_for_type("model", FallbackType::General),
            vec!["g"]
        );
        assert_eq!(
            config.get_fallbacks_for_type("model", FallbackType::ContextWindow),
            vec!["cw"]
        );
        assert_eq!(
            config.get_fallbacks_for_type("model", FallbackType::ContentPolicy),
            vec!["cp"]
        );
        assert_eq!(
            config.get_fallbacks_for_type("model", FallbackType::RateLimit),
            vec!["rl"]
        );
    }
}
