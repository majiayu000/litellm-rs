use super::*;

// ============================================================================
// DeepSeek Thinking Tests
// ============================================================================

#[test]
fn test_deepseek_thinking_detection() {
    assert!(deepseek_thinking::supports_thinking("deepseek-flash"));
    assert!(!deepseek_thinking::capabilities("deepseek-flash").thinking_always_on);
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

#[test]
fn flash_thinking_toggle_is_explicit() {
    for model in ["deepseek-flash", "deepseek/deepseek-flash"] {
        for enabled in [false, true] {
            let config = ThinkingConfig {
                enabled,
                ..Default::default()
            };
            let params = deepseek_thinking::transform_config(&config, model).unwrap();
            assert_eq!(
                params["thinking"]["type"],
                if enabled { "enabled" } else { "disabled" }
            );
        }
    }
    let legacy =
        deepseek_thinking::transform_config(&ThinkingConfig::default(), "deepseek-r1").unwrap();
    assert!(legacy.get("thinking").is_none());
}
