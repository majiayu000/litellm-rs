//! Provider-specific thinking implementations
//!
//! This module contains thinking/reasoning implementations for each supported provider:
//! - OpenAI (o1, o3, o4 series)
//! - Anthropic (Claude extended thinking)
//! - DeepSeek (R1/Reasoner)
//! - Gemini (Flash Thinking / Deep Think)
//! - OpenRouter (passthrough to underlying providers)

use serde_json::Value;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::thinking::{
    ThinkingCapabilities, ThinkingConfig, ThinkingContent, ThinkingEffort, ThinkingUsage,
};

/// OpenAI-specific thinking implementation
pub mod openai_thinking {
    use super::*;

    /// OpenAI thinking models (o1, o3, o4 series)
    const OPENAI_THINKING_MODELS: &[&str] = &[
        "o1",
        "o1-preview",
        "o1-mini",
        "o3",
        "o3-mini",
        "o4",
        "o4-mini",
    ];

    /// Check if an OpenAI model supports thinking
    pub fn supports_thinking(model: &str) -> bool {
        let model_lower = model.to_lowercase();
        OPENAI_THINKING_MODELS
            .iter()
            .any(|m| model_lower.starts_with(m) || model_lower.contains(&format!("/{}", m)))
    }

    /// Get thinking capabilities for OpenAI models
    pub fn capabilities(model: &str) -> ThinkingCapabilities {
        if supports_thinking(model) {
            ThinkingCapabilities {
                supports_thinking: true,
                supports_streaming_thinking: false, // OpenAI doesn't stream reasoning
                max_thinking_tokens: Some(20_000),
                supported_efforts: vec![
                    ThinkingEffort::Low,
                    ThinkingEffort::Medium,
                    ThinkingEffort::High,
                ],
                thinking_models: OPENAI_THINKING_MODELS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                can_return_thinking: true,
                thinking_always_on: false,
            }
        } else {
            ThinkingCapabilities::unsupported()
        }
    }

    /// Transform thinking config for OpenAI
    pub fn transform_config(config: &ThinkingConfig, _model: &str) -> Result<Value, ProviderError> {
        let mut params = serde_json::Map::new();

        if let Some(budget) = config.budget_tokens {
            // OpenAI max is 20,000
            let capped = budget.min(20_000);
            params.insert("max_reasoning_tokens".into(), capped.into());
        }

        if config.include_thinking {
            params.insert("include_reasoning".into(), true.into());
        }

        // Map effort to reasoning_effort
        if let Some(effort) = &config.effort {
            let effort_str = match effort {
                ThinkingEffort::Low => "low",
                ThinkingEffort::Medium => "medium",
                ThinkingEffort::High => "high",
            };
            params.insert("reasoning_effort".into(), effort_str.into());
        }

        Ok(Value::Object(params))
    }

    /// Extract thinking from OpenAI response
    pub fn extract_thinking(response: &Value) -> Option<ThinkingContent> {
        response
            .pointer("/choices/0/message/reasoning")
            .and_then(|v| v.as_str())
            .map(|text| ThinkingContent::Text {
                text: text.to_string(),
                signature: None,
            })
    }

    /// Extract thinking usage from OpenAI response
    pub fn extract_usage(response: &Value) -> Option<ThinkingUsage> {
        response
            .pointer("/usage/reasoning_tokens")
            .map(|tokens| ThinkingUsage {
                thinking_tokens: tokens.as_u64().map(|t| t as u32),
                budget_tokens: None,
                thinking_cost: None,
                provider: Some("openai".to_string()),
            })
    }
}

/// Anthropic-specific thinking implementation
pub mod anthropic_thinking {
    use super::*;

    /// Anthropic models with thinking support
    const ANTHROPIC_THINKING_MODELS: &[&str] = &[
        "claude-3-opus",
        "claude-3-sonnet",
        "claude-3-haiku",
        "claude-3-5-sonnet",
        "claude-3-5-opus",
        "claude-4",
    ];

    /// Check if an Anthropic model supports thinking
    pub fn supports_thinking(model: &str) -> bool {
        let model_lower = model.to_lowercase();
        ANTHROPIC_THINKING_MODELS
            .iter()
            .any(|m| model_lower.contains(m))
    }

    /// Get thinking capabilities for Anthropic models
    pub fn capabilities(model: &str) -> ThinkingCapabilities {
        if supports_thinking(model) {
            ThinkingCapabilities {
                supports_thinking: true,
                supports_streaming_thinking: true, // Anthropic supports streaming thinking
                max_thinking_tokens: Some(100_000), // Anthropic allows larger budgets
                supported_efforts: vec![ThinkingEffort::Medium, ThinkingEffort::High],
                thinking_models: ANTHROPIC_THINKING_MODELS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                can_return_thinking: true,
                thinking_always_on: false,
            }
        } else {
            ThinkingCapabilities::unsupported()
        }
    }

    /// Transform thinking config for Anthropic
    pub fn transform_config(config: &ThinkingConfig, _model: &str) -> Result<Value, ProviderError> {
        let mut params = serde_json::Map::new();

        if config.enabled {
            let mut thinking = serde_json::Map::new();
            thinking.insert("type".into(), "enabled".into());

            if let Some(budget) = config.budget_tokens {
                thinking.insert("budget_tokens".into(), budget.into());
            }

            params.insert("thinking".into(), Value::Object(thinking));
        }

        Ok(Value::Object(params))
    }

    /// Extract thinking from Anthropic response
    pub fn extract_thinking(response: &Value) -> Option<ThinkingContent> {
        response
            .pointer("/content")
            .and_then(|v| v.as_array())
            .and_then(|blocks| {
                blocks.iter().find_map(|block| {
                    if block.get("type")?.as_str()? == "thinking" {
                        Some(ThinkingContent::Block {
                            thinking: block.get("thinking")?.as_str()?.to_string(),
                            block_type: Some("thinking".to_string()),
                        })
                    } else {
                        None
                    }
                })
            })
    }

    /// Extract thinking usage from Anthropic response
    pub fn extract_usage(response: &Value) -> Option<ThinkingUsage> {
        let thinking_tokens = response
            .pointer("/usage/thinking_tokens")
            .and_then(|v| v.as_u64())
            .map(|t| t as u32);

        if thinking_tokens.is_some() {
            Some(ThinkingUsage {
                thinking_tokens,
                budget_tokens: response
                    .pointer("/usage/thinking_budget_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|t| t as u32),
                thinking_cost: None,
                provider: Some("anthropic".to_string()),
            })
        } else {
            None
        }
    }
}

/// DeepSeek-specific thinking implementation
pub mod deepseek_thinking {
    use super::*;

    /// DeepSeek thinking models
    const DEEPSEEK_THINKING_MODELS: &[&str] = &["deepseek-r1", "deepseek-reasoner", "r1"];

    /// Check if a DeepSeek model supports thinking
    pub fn supports_thinking(model: &str) -> bool {
        let model_lower = model.to_lowercase();
        DEEPSEEK_THINKING_MODELS
            .iter()
            .any(|m| model_lower.contains(m))
    }

    /// Get thinking capabilities for DeepSeek models
    pub fn capabilities(model: &str) -> ThinkingCapabilities {
        if supports_thinking(model) {
            ThinkingCapabilities {
                supports_thinking: true,
                supports_streaming_thinking: true,
                max_thinking_tokens: None, // No documented limit
                supported_efforts: vec![
                    ThinkingEffort::Low,
                    ThinkingEffort::Medium,
                    ThinkingEffort::High,
                ],
                thinking_models: DEEPSEEK_THINKING_MODELS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                can_return_thinking: true,
                thinking_always_on: true, // DeepSeek R1 always thinks
            }
        } else {
            ThinkingCapabilities::unsupported()
        }
    }

    /// Transform thinking config for DeepSeek
    pub fn transform_config(config: &ThinkingConfig, _model: &str) -> Result<Value, ProviderError> {
        let mut params = serde_json::Map::new();

        // DeepSeek uses reasoning_effort
        if let Some(effort) = &config.effort {
            let effort_str = match effort {
                ThinkingEffort::Low => "low",
                ThinkingEffort::Medium => "medium",
                ThinkingEffort::High => "high",
            };
            params.insert("reasoning_effort".into(), effort_str.into());
        }

        Ok(Value::Object(params))
    }

    /// Extract thinking from DeepSeek response
    pub fn extract_thinking(response: &Value) -> Option<ThinkingContent> {
        response
            .pointer("/choices/0/message/reasoning_content")
            .and_then(|v| v.as_str())
            .map(|text| ThinkingContent::Text {
                text: text.to_string(),
                signature: None,
            })
    }

    /// Extract thinking usage from DeepSeek response
    pub fn extract_usage(response: &Value) -> Option<ThinkingUsage> {
        response
            .pointer("/usage/reasoning_tokens")
            .map(|tokens| ThinkingUsage {
                thinking_tokens: tokens.as_u64().map(|t| t as u32),
                budget_tokens: None,
                thinking_cost: None,
                provider: Some("deepseek".to_string()),
            })
    }
}

/// Gemini-specific thinking implementation
pub mod gemini_thinking {
    use super::*;

    /// Gemini thinking models
    const GEMINI_THINKING_MODELS: &[&str] = &[
        "gemini-2.0-flash-thinking",
        "gemini-3.0-deep-think",
        "gemini-thinking",
    ];

    /// Check if a Gemini model supports thinking
    pub fn supports_thinking(model: &str) -> bool {
        let model_lower = model.to_lowercase();
        GEMINI_THINKING_MODELS
            .iter()
            .any(|m| model_lower.contains(m))
            || model_lower.contains("thinking")
            || model_lower.contains("deep-think")
    }

    /// Get thinking capabilities for Gemini models
    pub fn capabilities(model: &str) -> ThinkingCapabilities {
        if supports_thinking(model) {
            ThinkingCapabilities {
                supports_thinking: true,
                supports_streaming_thinking: true,
                max_thinking_tokens: Some(32_000),
                supported_efforts: vec![ThinkingEffort::Medium, ThinkingEffort::High],
                thinking_models: GEMINI_THINKING_MODELS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                can_return_thinking: true,
                thinking_always_on: false,
            }
        } else {
            ThinkingCapabilities::unsupported()
        }
    }

    /// Transform thinking config for Gemini
    pub fn transform_config(config: &ThinkingConfig, _model: &str) -> Result<Value, ProviderError> {
        let mut params = serde_json::Map::new();

        if config.enabled {
            params.insert("enableThinking".into(), true.into());

            if let Some(budget) = config.budget_tokens {
                params.insert("thinkingBudget".into(), budget.into());
            }
        }

        Ok(Value::Object(params))
    }

    /// Extract thinking from Gemini response
    pub fn extract_thinking(response: &Value) -> Option<ThinkingContent> {
        // Try thoughts field first
        response
            .pointer("/candidates/0/content/thoughts")
            .and_then(|v| v.as_str())
            .map(|text| ThinkingContent::Text {
                text: text.to_string(),
                signature: None,
            })
            // Also try thinking field
            .or_else(|| {
                response
                    .pointer("/candidates/0/content/thinking")
                    .and_then(|v| v.as_str())
                    .map(|text| ThinkingContent::Text {
                        text: text.to_string(),
                        signature: None,
                    })
            })
    }

    /// Extract thinking usage from Gemini response
    pub fn extract_usage(response: &Value) -> Option<ThinkingUsage> {
        response
            .pointer("/usageMetadata/thinkingTokenCount")
            .map(|tokens| ThinkingUsage {
                thinking_tokens: tokens.as_u64().map(|t| t as u32),
                budget_tokens: None,
                thinking_cost: None,
                provider: Some("gemini".to_string()),
            })
    }
}

/// OpenRouter passthrough implementation
///
/// OpenRouter routes to multiple providers, so we detect the underlying provider
/// and use the appropriate thinking extraction.
pub mod openrouter_thinking {
    use super::*;

    /// Check if a model supports thinking through OpenRouter
    pub fn supports_thinking(model: &str) -> bool {
        let model_lower = model.to_lowercase();

        // Check for OpenAI reasoning models
        if model_lower.contains("o1") || model_lower.contains("o3") || model_lower.contains("o4") {
            return true;
        }

        // Check for Anthropic models
        if model_lower.contains("claude") {
            return true;
        }

        // Check for DeepSeek reasoning models
        if model_lower.contains("deepseek-r1") || model_lower.contains("reasoner") {
            return true;
        }

        // Check for Gemini thinking models
        if model_lower.contains("gemini") && model_lower.contains("thinking") {
            return true;
        }

        false
    }

    /// Detect the underlying provider from model name
    pub fn detect_provider(model: &str) -> &'static str {
        let model_lower = model.to_lowercase();

        if model_lower.contains("openai")
            || model_lower.starts_with("o1")
            || model_lower.starts_with("o3")
            || model_lower.starts_with("o4")
        {
            "openai"
        } else if model_lower.contains("anthropic") || model_lower.contains("claude") {
            "anthropic"
        } else if model_lower.contains("deepseek") {
            "deepseek"
        } else if model_lower.contains("gemini") || model_lower.contains("google") {
            "gemini"
        } else {
            "unknown"
        }
    }

    /// Get thinking capabilities for OpenRouter models
    pub fn capabilities(model: &str) -> ThinkingCapabilities {
        match detect_provider(model) {
            "openai" => openai_thinking::capabilities(model),
            "anthropic" => anthropic_thinking::capabilities(model),
            "deepseek" => deepseek_thinking::capabilities(model),
            "gemini" => gemini_thinking::capabilities(model),
            _ => ThinkingCapabilities::unsupported(),
        }
    }

    /// Transform thinking config for OpenRouter
    ///
    /// For models with a recognized provider prefix (openai/, anthropic/, etc.) the
    /// provider-specific format is used so OpenRouter can forward it correctly.
    /// For all other models the OpenRouter-native `reasoning.effort` object is emitted,
    /// which OpenRouter accepts universally across its model catalog.
    pub fn transform_config(config: &ThinkingConfig, model: &str) -> Result<Value, ProviderError> {
        match detect_provider(model) {
            "openai" => openai_thinking::transform_config(config, model),
            "anthropic" => anthropic_thinking::transform_config(config, model),
            "deepseek" => deepseek_thinking::transform_config(config, model),
            "gemini" => gemini_thinking::transform_config(config, model),
            _ => {
                // Use OpenRouter's native reasoning object for generic/unknown models.
                // `effort` and `max_tokens` are alternative controls on OpenRouter;
                // sending both causes validation failures on stricter backends.
                let mut params = serde_json::Map::new();
                if let Some(effort) = &config.effort {
                    let effort_str = match effort {
                        ThinkingEffort::Low => "low",
                        ThinkingEffort::Medium => "medium",
                        ThinkingEffort::High => "high",
                    };
                    let mut reasoning = serde_json::Map::new();
                    reasoning.insert("effort".into(), effort_str.into());
                    // Do not set max_tokens when effort is present; they are mutually exclusive.
                    params.insert("reasoning".into(), Value::Object(reasoning));
                } else if let Some(budget) = config.budget_tokens {
                    let mut reasoning = serde_json::Map::new();
                    reasoning.insert("max_tokens".into(), budget.into());
                    params.insert("reasoning".into(), Value::Object(reasoning));
                }
                Ok(Value::Object(params))
            }
        }
    }

    /// Extract thinking from OpenRouter response
    ///
    /// Tries multiple extraction patterns since the response format
    /// depends on the underlying provider.
    pub fn extract_thinking(response: &Value) -> Option<ThinkingContent> {
        // Try OpenAI style
        if let Some(thinking) = openai_thinking::extract_thinking(response) {
            return Some(thinking);
        }

        // Try DeepSeek style
        if let Some(thinking) = deepseek_thinking::extract_thinking(response) {
            return Some(thinking);
        }

        // Try Anthropic style
        if let Some(thinking) = anthropic_thinking::extract_thinking(response) {
            return Some(thinking);
        }

        // Try Gemini style
        if let Some(thinking) = gemini_thinking::extract_thinking(response) {
            return Some(thinking);
        }

        None
    }

    /// Extract thinking usage from OpenRouter response
    pub fn extract_usage(response: &Value) -> Option<ThinkingUsage> {
        // Try OpenAI style
        if let Some(mut usage) = openai_thinking::extract_usage(response) {
            usage.provider = Some("openrouter".to_string());
            return Some(usage);
        }

        // Try DeepSeek style
        if let Some(mut usage) = deepseek_thinking::extract_usage(response) {
            usage.provider = Some("openrouter".to_string());
            return Some(usage);
        }

        // Try Anthropic style
        if let Some(mut usage) = anthropic_thinking::extract_usage(response) {
            usage.provider = Some("openrouter".to_string());
            return Some(usage);
        }

        // Try Gemini style
        if let Some(mut usage) = gemini_thinking::extract_usage(response) {
            usage.provider = Some("openrouter".to_string());
            return Some(usage);
        }

        None
    }
}
