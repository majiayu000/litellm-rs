//! Tests for the thinking module
//!
//! Comprehensive tests for all thinking/reasoning provider implementations.

use super::providers::{
    anthropic_thinking, deepseek_thinking, gemini_thinking, openai_thinking, openrouter_thinking,
};
use super::trait_def::{NoThinkingSupport, ThinkingProvider};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::thinking::{
    ThinkingCapabilities, ThinkingConfig, ThinkingContent, ThinkingEffort, ThinkingUsage,
};
use serde_json::Value;

// ============================================================================
// NoThinkingSupport Tests
// ============================================================================

#[test]
fn test_no_thinking_support() {
    let no_support = NoThinkingSupport;
    assert!(!no_support.supports_thinking("any-model"));
    assert!(
        no_support
            .extract_thinking(&serde_json::json!({}))
            .is_none()
    );
}

#[test]
fn test_no_thinking_support_capabilities() {
    let no_support = NoThinkingSupport;
    let caps = no_support.thinking_capabilities("any-model");
    assert!(!caps.supports_thinking);
    assert!(!caps.supports_streaming_thinking);
    assert!(!caps.can_return_thinking);
}

#[test]
fn test_no_thinking_support_transform_config() {
    let no_support = NoThinkingSupport;
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(1000),
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };
    let result = no_support
        .transform_thinking_config(&config, "model")
        .unwrap();
    assert!(result.as_object().unwrap().is_empty());
}

#[test]
fn test_no_thinking_support_extract_usage() {
    let no_support = NoThinkingSupport;
    let response = serde_json::json!({
        "usage": {
            "thinking_tokens": 100
        }
    });
    assert!(no_support.extract_thinking_usage(&response).is_none());
}

// ============================================================================
// OpenAI Thinking Tests
// ============================================================================

#[test]
fn test_openai_thinking_detection() {
    assert!(openai_thinking::supports_thinking("o1"));
    assert!(openai_thinking::supports_thinking("o1-preview"));
    assert!(openai_thinking::supports_thinking("o3-mini"));
    assert!(openai_thinking::supports_thinking("O1-PREVIEW")); // Case insensitive
    assert!(openai_thinking::supports_thinking("o4"));
    assert!(openai_thinking::supports_thinking("openai/o1-preview")); // With prefix
    assert!(!openai_thinking::supports_thinking("gpt-4"));
    assert!(!openai_thinking::supports_thinking("gpt-4o"));
}

#[test]
fn test_openai_capabilities() {
    let caps = openai_thinking::capabilities("o1-preview");
    assert!(caps.supports_thinking);
    assert!(!caps.supports_streaming_thinking);
    assert_eq!(caps.max_thinking_tokens, Some(20_000));
    assert_eq!(caps.supported_efforts.len(), 3);
    assert!(caps.can_return_thinking);
    assert!(!caps.thinking_always_on);
}

#[test]
fn test_openai_capabilities_non_thinking_model() {
    let caps = openai_thinking::capabilities("gpt-4");
    assert!(!caps.supports_thinking);
}

#[test]
fn test_openai_config_transform() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(10000),
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = openai_thinking::transform_config(&config, "o1").unwrap();
    assert!(result.get("max_reasoning_tokens").is_some());
    assert_eq!(
        result.get("max_reasoning_tokens").unwrap().as_u64(),
        Some(10000)
    );
    assert!(result.get("include_reasoning").is_some());
    assert_eq!(
        result.get("reasoning_effort").unwrap().as_str(),
        Some("high")
    );
}

#[test]
fn test_openai_config_transform_budget_capping() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(50000), // Over the 20k limit
        effort: Some(ThinkingEffort::Medium),
        include_thinking: false,
        extra_params: Default::default(),
    };

    let result = openai_thinking::transform_config(&config, "o1").unwrap();
    assert_eq!(
        result.get("max_reasoning_tokens").unwrap().as_u64(),
        Some(20000)
    );
    assert_eq!(
        result.get("reasoning_effort").unwrap().as_str(),
        Some("medium")
    );
    assert!(result.get("include_reasoning").is_none());
}

#[test]
fn test_openai_config_transform_minimal() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: None,
        effort: None,
        include_thinking: false,
        extra_params: Default::default(),
    };

    let result = openai_thinking::transform_config(&config, "o1").unwrap();
    assert!(result.get("max_reasoning_tokens").is_none());
    assert!(result.get("reasoning_effort").is_none());
    assert!(result.get("include_reasoning").is_none());
}

#[test]
fn test_openai_config_transform_low_effort() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: None,
        effort: Some(ThinkingEffort::Low),
        include_thinking: false,
        extra_params: Default::default(),
    };

    let result = openai_thinking::transform_config(&config, "o1").unwrap();
    assert_eq!(
        result.get("reasoning_effort").unwrap().as_str(),
        Some("low")
    );
}

#[test]
fn test_openai_thinking_extraction() {
    let openai_response = serde_json::json!({
        "choices": [{
            "message": {
                "content": "The answer is 42.",
                "reasoning": "Let me think about this step by step..."
            }
        }],
        "usage": {
            "reasoning_tokens": 150
        }
    });

    let thinking = openai_thinking::extract_thinking(&openai_response);
    assert!(thinking.is_some());
    if let Some(ThinkingContent::Text { text, .. }) = thinking {
        assert!(text.contains("step by step"));
    }
}

#[test]
fn test_openai_thinking_extraction_missing() {
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "content": "The answer is 42."
            }
        }]
    });

    let thinking = openai_thinking::extract_thinking(&response);
    assert!(thinking.is_none());
}

#[test]
fn test_openai_usage_extraction() {
    let openai_response = serde_json::json!({
        "usage": {
            "reasoning_tokens": 150
        }
    });

    let usage = openai_thinking::extract_usage(&openai_response);
    assert!(usage.is_some());
    let usage = usage.unwrap();
    assert_eq!(usage.thinking_tokens, Some(150));
    assert_eq!(usage.provider, Some("openai".to_string()));
}

#[test]
fn test_openai_usage_extraction_missing() {
    let response = serde_json::json!({
        "usage": {
            "total_tokens": 100
        }
    });

    let usage = openai_thinking::extract_usage(&response);
    assert!(usage.is_none());
}

// ============================================================================
// Anthropic Thinking Tests
// ============================================================================

#[test]
fn test_anthropic_thinking_detection() {
    assert!(anthropic_thinking::supports_thinking("claude-3-opus"));
    assert!(anthropic_thinking::supports_thinking(
        "claude-3-5-sonnet-20241022"
    ));
    assert!(anthropic_thinking::supports_thinking("Claude-3-Opus")); // Case insensitive
    assert!(anthropic_thinking::supports_thinking("claude-4"));
    assert!(!anthropic_thinking::supports_thinking("claude-2"));
}

#[test]
fn test_anthropic_capabilities() {
    let caps = anthropic_thinking::capabilities("claude-3-opus");
    assert!(caps.supports_thinking);
    assert!(caps.supports_streaming_thinking);
    assert_eq!(caps.max_thinking_tokens, Some(100_000));
    assert_eq!(caps.supported_efforts.len(), 2);
    assert!(caps.can_return_thinking);
    assert!(!caps.thinking_always_on);
}

#[test]
fn test_anthropic_capabilities_non_thinking_model() {
    let caps = anthropic_thinking::capabilities("claude-2");
    assert!(!caps.supports_thinking);
}

#[test]
fn test_anthropic_config_transform_enabled() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(50000),
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = anthropic_thinking::transform_config(&config, "claude-3-opus").unwrap();
    let thinking_obj = result.get("thinking").unwrap();
    assert_eq!(thinking_obj.get("type").unwrap().as_str(), Some("enabled"));
    assert_eq!(
        thinking_obj.get("budget_tokens").unwrap().as_u64(),
        Some(50000)
    );
}

#[test]
fn test_anthropic_config_transform_disabled() {
    let config = ThinkingConfig {
        enabled: false,
        budget_tokens: Some(50000),
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = anthropic_thinking::transform_config(&config, "claude-3-opus").unwrap();
    assert!(result.get("thinking").is_none());
}

#[test]
fn test_anthropic_config_transform_no_budget() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: None,
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = anthropic_thinking::transform_config(&config, "claude-3-opus").unwrap();
    let thinking_obj = result.get("thinking").unwrap();
    assert_eq!(thinking_obj.get("type").unwrap().as_str(), Some("enabled"));
    assert!(thinking_obj.get("budget_tokens").is_none());
}

#[test]
fn test_anthropic_thinking_extraction() {
    let response = serde_json::json!({
        "content": [
            {
                "type": "thinking",
                "thinking": "Let me analyze this carefully..."
            },
            {
                "type": "text",
                "text": "The answer is 42."
            }
        ]
    });

    let thinking = anthropic_thinking::extract_thinking(&response);
    assert!(thinking.is_some());
    if let Some(ThinkingContent::Block { thinking, .. }) = thinking {
        assert!(thinking.contains("analyze"));
    }
}

#[test]
fn test_anthropic_thinking_extraction_no_thinking_block() {
    let response = serde_json::json!({
        "content": [
            {
                "type": "text",
                "text": "The answer is 42."
            }
        ]
    });

    let thinking = anthropic_thinking::extract_thinking(&response);
    assert!(thinking.is_none());
}

#[test]
fn test_anthropic_usage_extraction() {
    let response = serde_json::json!({
        "usage": {
            "thinking_tokens": 500,
            "thinking_budget_tokens": 100000
        }
    });

    let usage = anthropic_thinking::extract_usage(&response);
    assert!(usage.is_some());
    let usage = usage.unwrap();
    assert_eq!(usage.thinking_tokens, Some(500));
    assert_eq!(usage.budget_tokens, Some(100000));
    assert_eq!(usage.provider, Some("anthropic".to_string()));
}

#[test]
fn test_anthropic_usage_extraction_partial() {
    let response = serde_json::json!({
        "usage": {
            "thinking_tokens": 500
        }
    });

    let usage = anthropic_thinking::extract_usage(&response);
    assert!(usage.is_some());
    let usage = usage.unwrap();
    assert_eq!(usage.thinking_tokens, Some(500));
    assert!(usage.budget_tokens.is_none());
}

#[test]
fn test_anthropic_usage_extraction_missing() {
    let response = serde_json::json!({
        "usage": {
            "total_tokens": 1000
        }
    });

    let usage = anthropic_thinking::extract_usage(&response);
    assert!(usage.is_none());
}

// ============================================================================
// DeepSeek Thinking Tests
// ============================================================================

#[test]
fn test_deepseek_thinking_detection() {
    assert!(deepseek_thinking::supports_thinking("deepseek-r1"));
    assert!(deepseek_thinking::supports_thinking("deepseek-reasoner"));
    assert!(deepseek_thinking::supports_thinking("r1"));
    assert!(deepseek_thinking::supports_thinking("DeepSeek-R1")); // Case insensitive
    assert!(!deepseek_thinking::supports_thinking("deepseek-chat"));
}

#[test]
fn test_deepseek_capabilities() {
    let caps = deepseek_thinking::capabilities("deepseek-r1");
    assert!(caps.supports_thinking);
    assert!(caps.supports_streaming_thinking);
    assert!(caps.max_thinking_tokens.is_none());
    assert_eq!(caps.supported_efforts.len(), 3);
    assert!(caps.can_return_thinking);
    assert!(caps.thinking_always_on);
}

#[test]
fn test_deepseek_capabilities_non_thinking_model() {
    let caps = deepseek_thinking::capabilities("deepseek-chat");
    assert!(!caps.supports_thinking);
}

#[test]
fn test_deepseek_config_transform_all_efforts() {
    for (effort, expected) in [
        (ThinkingEffort::Low, "low"),
        (ThinkingEffort::Medium, "medium"),
        (ThinkingEffort::High, "high"),
    ] {
        let config = ThinkingConfig {
            enabled: true,
            budget_tokens: None,
            effort: Some(effort),
            include_thinking: true,
            extra_params: Default::default(),
        };

        let result = deepseek_thinking::transform_config(&config, "deepseek-r1").unwrap();
        assert_eq!(
            result.get("reasoning_effort").unwrap().as_str(),
            Some(expected)
        );
    }
}

#[test]
fn test_deepseek_config_transform_no_effort() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: None,
        effort: None,
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = deepseek_thinking::transform_config(&config, "deepseek-r1").unwrap();
    assert!(result.get("reasoning_effort").is_none());
}

#[test]
fn test_deepseek_thinking_extraction() {
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "content": "The answer is 42.",
                "reasoning_content": "Step 1: Analyze the question..."
            }
        }]
    });

    let thinking = deepseek_thinking::extract_thinking(&response);
    assert!(thinking.is_some());
    if let Some(ThinkingContent::Text { text, .. }) = thinking {
        assert!(text.contains("Step 1"));
    }
}

#[test]
fn test_deepseek_thinking_extraction_missing() {
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "content": "The answer is 42."
            }
        }]
    });

    let thinking = deepseek_thinking::extract_thinking(&response);
    assert!(thinking.is_none());
}

#[test]
fn test_deepseek_usage_extraction() {
    let response = serde_json::json!({
        "usage": {
            "reasoning_tokens": 800
        }
    });

    let usage = deepseek_thinking::extract_usage(&response);
    assert!(usage.is_some());
    let usage = usage.unwrap();
    assert_eq!(usage.thinking_tokens, Some(800));
    assert_eq!(usage.provider, Some("deepseek".to_string()));
}

#[test]
fn test_deepseek_usage_extraction_missing() {
    let response = serde_json::json!({
        "usage": {
            "total_tokens": 1000
        }
    });

    let usage = deepseek_thinking::extract_usage(&response);
    assert!(usage.is_none());
}

// ============================================================================
// Gemini Thinking Tests
// ============================================================================

#[test]
fn test_gemini_thinking_detection() {
    assert!(gemini_thinking::supports_thinking(
        "gemini-2.0-flash-thinking-exp"
    ));
    assert!(gemini_thinking::supports_thinking("gemini-thinking"));
    assert!(gemini_thinking::supports_thinking("gemini-3.0-deep-think"));
    assert!(gemini_thinking::supports_thinking("Gemini-Thinking")); // Case insensitive
    assert!(gemini_thinking::supports_thinking("gemini-deep-think"));
    assert!(!gemini_thinking::supports_thinking("gemini-pro"));
}

#[test]
fn test_gemini_capabilities() {
    let caps = gemini_thinking::capabilities("gemini-2.0-flash-thinking");
    assert!(caps.supports_thinking);
    assert!(caps.supports_streaming_thinking);
    assert_eq!(caps.max_thinking_tokens, Some(32_000));
    assert_eq!(caps.supported_efforts.len(), 2);
    assert!(caps.can_return_thinking);
    assert!(!caps.thinking_always_on);
}

#[test]
fn test_gemini_capabilities_non_thinking_model() {
    let caps = gemini_thinking::capabilities("gemini-pro");
    assert!(!caps.supports_thinking);
}

#[test]
fn test_gemini_config_transform_enabled() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(10000),
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = gemini_thinking::transform_config(&config, "gemini-thinking").unwrap();
    assert_eq!(result.get("enableThinking").unwrap().as_bool(), Some(true));
    assert_eq!(result.get("thinkingBudget").unwrap().as_u64(), Some(10000));
}

#[test]
fn test_gemini_config_transform_disabled() {
    let config = ThinkingConfig {
        enabled: false,
        budget_tokens: Some(10000),
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = gemini_thinking::transform_config(&config, "gemini-thinking").unwrap();
    assert!(result.get("enableThinking").is_none());
}

#[test]
fn test_gemini_config_transform_no_budget() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: None,
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = gemini_thinking::transform_config(&config, "gemini-thinking").unwrap();
    assert_eq!(result.get("enableThinking").unwrap().as_bool(), Some(true));
    assert!(result.get("thinkingBudget").is_none());
}

#[test]
fn test_gemini_thinking_extraction_thoughts() {
    let response = serde_json::json!({
        "candidates": [{
            "content": {
                "thoughts": "Let me think through this problem..."
            }
        }]
    });

    let thinking = gemini_thinking::extract_thinking(&response);
    assert!(thinking.is_some());
    if let Some(ThinkingContent::Text { text, .. }) = thinking {
        assert!(text.contains("think through"));
    }
}

#[test]
fn test_gemini_thinking_extraction_thinking() {
    let response = serde_json::json!({
        "candidates": [{
            "content": {
                "thinking": "Analyzing the data..."
            }
        }]
    });

    let thinking = gemini_thinking::extract_thinking(&response);
    assert!(thinking.is_some());
    if let Some(ThinkingContent::Text { text, .. }) = thinking {
        assert!(text.contains("Analyzing"));
    }
}

#[test]
fn test_gemini_thinking_extraction_missing() {
    let response = serde_json::json!({
        "candidates": [{
            "content": {
                "text": "The answer is 42."
            }
        }]
    });

    let thinking = gemini_thinking::extract_thinking(&response);
    assert!(thinking.is_none());
}

#[test]
fn test_gemini_usage_extraction() {
    let response = serde_json::json!({
        "usageMetadata": {
            "thinkingTokenCount": 1200
        }
    });

    let usage = gemini_thinking::extract_usage(&response);
    assert!(usage.is_some());
    let usage = usage.unwrap();
    assert_eq!(usage.thinking_tokens, Some(1200));
    assert_eq!(usage.provider, Some("gemini".to_string()));
}

#[test]
fn test_gemini_usage_extraction_missing() {
    let response = serde_json::json!({
        "usageMetadata": {
            "totalTokenCount": 2000
        }
    });

    let usage = gemini_thinking::extract_usage(&response);
    assert!(usage.is_none());
}

// ============================================================================
// OpenRouter Thinking Tests
// ============================================================================

#[test]
fn test_openrouter_thinking_detection() {
    assert!(openrouter_thinking::supports_thinking("openai/o1-preview"));
    assert!(openrouter_thinking::supports_thinking("o1-mini"));
    assert!(openrouter_thinking::supports_thinking(
        "anthropic/claude-3-opus"
    ));
    assert!(openrouter_thinking::supports_thinking("claude-3-sonnet"));
    assert!(openrouter_thinking::supports_thinking(
        "deepseek/deepseek-r1"
    ));
    assert!(openrouter_thinking::supports_thinking("deepseek-reasoner"));
    assert!(openrouter_thinking::supports_thinking(
        "google/gemini-thinking"
    ));
    assert!(openrouter_thinking::supports_thinking(
        "gemini-2.0-flash-thinking"
    ));
    assert!(!openrouter_thinking::supports_thinking("gpt-4"));
}

#[test]
fn test_openrouter_provider_detection() {
    assert_eq!(
        openrouter_thinking::detect_provider("openai/o1-preview"),
        "openai"
    );
    assert_eq!(openrouter_thinking::detect_provider("o1-mini"), "openai");
    assert_eq!(openrouter_thinking::detect_provider("o3-mini"), "openai");
    assert_eq!(
        openrouter_thinking::detect_provider("anthropic/claude-3-opus"),
        "anthropic"
    );
    assert_eq!(
        openrouter_thinking::detect_provider("claude-3-5-sonnet"),
        "anthropic"
    );
    assert_eq!(
        openrouter_thinking::detect_provider("deepseek/deepseek-r1"),
        "deepseek"
    );
    assert_eq!(
        openrouter_thinking::detect_provider("google/gemini-thinking"),
        "gemini"
    );
    assert_eq!(openrouter_thinking::detect_provider("gemini-pro"), "gemini");
    assert_eq!(
        openrouter_thinking::detect_provider("unknown-model"),
        "unknown"
    );
}

#[test]
fn test_openrouter_capabilities_openai() {
    let caps = openrouter_thinking::capabilities("openai/o1-preview");
    assert!(caps.supports_thinking);
    assert!(!caps.supports_streaming_thinking);
}

#[test]
fn test_openrouter_capabilities_anthropic() {
    let caps = openrouter_thinking::capabilities("anthropic/claude-3-opus");
    assert!(caps.supports_thinking);
    assert!(caps.supports_streaming_thinking);
}

#[test]
fn test_openrouter_capabilities_deepseek() {
    let caps = openrouter_thinking::capabilities("deepseek/deepseek-r1");
    assert!(caps.supports_thinking);
    assert!(caps.thinking_always_on);
}

#[test]
fn test_openrouter_capabilities_gemini() {
    let caps = openrouter_thinking::capabilities("google/gemini-thinking");
    assert!(caps.supports_thinking);
}

#[test]
fn test_openrouter_capabilities_unknown() {
    let caps = openrouter_thinking::capabilities("unknown-model");
    assert!(!caps.supports_thinking);
}

#[test]
fn test_openrouter_transform_config_openai() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(10000),
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = openrouter_thinking::transform_config(&config, "openai/o1-preview").unwrap();
    assert!(result.get("max_reasoning_tokens").is_some());
    assert!(result.get("reasoning_effort").is_some());
}

#[test]
fn test_openrouter_transform_config_anthropic() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(50000),
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = openrouter_thinking::transform_config(&config, "anthropic/claude-3-opus").unwrap();
    assert!(result.get("thinking").is_some());
}

#[test]
fn test_openrouter_transform_config_deepseek() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(5000),
        effort: Some(ThinkingEffort::Medium),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = openrouter_thinking::transform_config(&config, "deepseek/deepseek-r1").unwrap();
    assert!(result.get("reasoning_effort").is_some());
}

#[test]
fn test_openrouter_transform_config_gemini() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(15000),
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    let result = openrouter_thinking::transform_config(&config, "google/gemini-thinking").unwrap();
    assert!(result.get("enableThinking").is_some());
}

#[test]
fn test_openrouter_transform_config_unknown() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: Some(10000),
        effort: Some(ThinkingEffort::High),
        include_thinking: true,
        extra_params: Default::default(),
    };

    // Unknown models use the OpenRouter-native reasoning.effort format
    let Ok(result) = openrouter_thinking::transform_config(&config, "unknown-model") else {
        panic!("transform_config must succeed for unknown-model");
    };
    assert_eq!(
        result
            .get("reasoning")
            .and_then(|r| r.get("effort"))
            .and_then(|v| v.as_str()),
        Some("high"),
        "reasoning.effort should be 'high' for High effort"
    );
    assert_eq!(
        result
            .get("reasoning")
            .and_then(|r| r.get("max_tokens"))
            .and_then(|v| v.as_u64()),
        Some(10000),
        "reasoning.max_tokens should equal budget_tokens"
    );
}

#[test]
fn test_openrouter_transform_config_unknown_no_effort() {
    let config = ThinkingConfig {
        enabled: true,
        budget_tokens: None,
        effort: None,
        include_thinking: true,
        extra_params: Default::default(),
    };

    // No effort specified: no reasoning object emitted
    let Ok(result) = openrouter_thinking::transform_config(&config, "unknown-model") else {
        panic!("transform_config must succeed for unknown-model");
    };
    assert!(
        result.as_object().is_some_and(|m| m.is_empty()),
        "no reasoning key when effort is None"
    );
}

#[test]
fn test_openrouter_extract_thinking_openai() {
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "reasoning": "OpenAI reasoning content"
            }
        }]
    });

    let thinking = openrouter_thinking::extract_thinking(&response);
    assert!(thinking.is_some());
}

#[test]
fn test_openrouter_extract_thinking_deepseek() {
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "reasoning_content": "DeepSeek reasoning content"
            }
        }]
    });

    let thinking = openrouter_thinking::extract_thinking(&response);
    assert!(thinking.is_some());
}

#[test]
fn test_openrouter_extract_thinking_anthropic() {
    let response = serde_json::json!({
        "content": [
            {
                "type": "thinking",
                "thinking": "Anthropic thinking content"
            }
        ]
    });

    let thinking = openrouter_thinking::extract_thinking(&response);
    assert!(thinking.is_some());
}

#[test]
fn test_openrouter_extract_thinking_gemini() {
    let response = serde_json::json!({
        "candidates": [{
            "content": {
                "thoughts": "Gemini thoughts content"
            }
        }]
    });

    let thinking = openrouter_thinking::extract_thinking(&response);
    assert!(thinking.is_some());
}

#[test]
fn test_openrouter_extract_thinking_none() {
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "content": "Regular response"
            }
        }]
    });

    let thinking = openrouter_thinking::extract_thinking(&response);
    assert!(thinking.is_none());
}

#[test]
fn test_openrouter_extract_usage_openai() {
    let response = serde_json::json!({
        "usage": {
            "reasoning_tokens": 500
        }
    });

    let usage = openrouter_thinking::extract_usage(&response);
    assert!(usage.is_some());
    let usage = usage.unwrap();
    assert_eq!(usage.thinking_tokens, Some(500));
    assert_eq!(usage.provider, Some("openrouter".to_string()));
}

#[test]
fn test_openrouter_extract_usage_deepseek() {
    let response = serde_json::json!({
        "usage": {
            "reasoning_tokens": 800
        }
    });

    let usage = openrouter_thinking::extract_usage(&response);
    assert!(usage.is_some());
    assert_eq!(usage.unwrap().provider, Some("openrouter".to_string()));
}

#[test]
fn test_openrouter_extract_usage_anthropic() {
    let response = serde_json::json!({
        "usage": {
            "thinking_tokens": 600
        }
    });

    let usage = openrouter_thinking::extract_usage(&response);
    assert!(usage.is_some());
    assert_eq!(usage.unwrap().provider, Some("openrouter".to_string()));
}

#[test]
fn test_openrouter_extract_usage_gemini() {
    let response = serde_json::json!({
        "usageMetadata": {
            "thinkingTokenCount": 1000
        }
    });

    let usage = openrouter_thinking::extract_usage(&response);
    assert!(usage.is_some());
    assert_eq!(usage.unwrap().provider, Some("openrouter".to_string()));
}

#[test]
fn test_openrouter_extract_usage_none() {
    let response = serde_json::json!({
        "usage": {
            "total_tokens": 100
        }
    });

    let usage = openrouter_thinking::extract_usage(&response);
    assert!(usage.is_none());
}

// ============================================================================
// ThinkingProvider Trait Default Methods Tests
// ============================================================================

struct MockThinkingProvider;

impl ThinkingProvider for MockThinkingProvider {
    fn supports_thinking(&self, _model: &str) -> bool {
        true
    }

    fn thinking_capabilities(&self, _model: &str) -> ThinkingCapabilities {
        ThinkingCapabilities {
            supports_thinking: true,
            supports_streaming_thinking: true,
            max_thinking_tokens: Some(5000),
            supported_efforts: vec![ThinkingEffort::Medium, ThinkingEffort::High],
            thinking_models: vec!["test-model".to_string()],
            can_return_thinking: true,
            thinking_always_on: false,
        }
    }

    fn transform_thinking_config(
        &self,
        _config: &ThinkingConfig,
        _model: &str,
    ) -> Result<Value, ProviderError> {
        Ok(serde_json::json!({}))
    }

    fn extract_thinking(&self, _response: &Value) -> Option<ThinkingContent> {
        None
    }

    fn extract_thinking_usage(&self, _response: &Value) -> Option<ThinkingUsage> {
        None
    }
}

#[test]
fn test_thinking_provider_default_effort() {
    let provider = MockThinkingProvider;
    assert_eq!(provider.default_thinking_effort(), ThinkingEffort::Medium);
}

#[test]
fn test_thinking_provider_max_thinking_tokens() {
    let provider = MockThinkingProvider;
    assert_eq!(provider.max_thinking_tokens("test-model"), Some(5000));
}

#[test]
fn test_thinking_provider_supports_streaming_thinking() {
    let provider = MockThinkingProvider;
    assert!(provider.supports_streaming_thinking("test-model"));
}
