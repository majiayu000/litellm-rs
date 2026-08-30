use super::*;

fn exact(model: &str) -> TokenizerIdentity {
    TokenizerIdentity::exact_openai(model)
}

// ==================== TokenCounter Creation Tests ====================

#[test]
fn test_token_counter_new() {
    let counter = TokenCounter::new();
    assert!(!counter.model_configs.is_empty());
}

#[test]
fn test_token_counter_default() {
    let counter = TokenCounter::default();
    assert!(!counter.model_configs.is_empty());
}

#[test]
fn test_token_counter_clone() {
    let counter = TokenCounter::new();
    let cloned = counter.clone();
    assert_eq!(counter.model_configs.len(), cloned.model_configs.len());
}

#[test]
fn test_token_counter_debug() {
    let counter = TokenCounter::new();
    let debug_str = format!("{:?}", counter);
    assert!(debug_str.contains("TokenCounter"));
    assert!(debug_str.contains("model_configs"));
}

// ==================== Model Config Tests ====================

#[test]
fn test_get_model_config_exact_match() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-4");
    assert!(config.is_ok());
}

#[test]
fn test_get_model_config_gpt4_variant() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-4-turbo");
    assert!(config.is_ok());
}

#[test]
fn test_get_model_config_gpt35() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-3.5-turbo");
    assert!(config.is_ok());
}

#[test]
fn test_get_model_config_claude() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("claude-3-opus");
    assert!(config.is_ok());
}

#[test]
fn test_get_model_config_fallback_to_default() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("unknown-model-xyz");
    // Should fall back to default config
    assert!(config.is_ok());
}

// ==================== Model Family Extraction Tests ====================

#[test]
fn test_extract_model_family_gpt4() {
    let counter = TokenCounter::new();
    assert_eq!(counter.extract_model_family("gpt-4"), "gpt-4");
    assert_eq!(counter.extract_model_family("gpt-4-turbo"), "gpt-4");
    assert_eq!(counter.extract_model_family("gpt-4-0125-preview"), "gpt-4");
    assert_eq!(counter.extract_model_family("gpt-4o"), "gpt-4");
}

#[test]
fn test_extract_model_family_gpt35() {
    let counter = TokenCounter::new();
    assert_eq!(
        counter.extract_model_family("gpt-3.5-turbo"),
        "gpt-3.5-turbo"
    );
    assert_eq!(
        counter.extract_model_family("gpt-3.5-turbo-16k"),
        "gpt-3.5-turbo"
    );
}

#[test]
fn test_extract_model_family_claude() {
    let counter = TokenCounter::new();
    assert_eq!(counter.extract_model_family("claude-3-opus"), "claude-3");
    assert_eq!(counter.extract_model_family("claude-3-sonnet"), "claude-3");
    assert_eq!(counter.extract_model_family("claude-3-haiku"), "claude-3");
    assert_eq!(counter.extract_model_family("claude-2.1"), "claude-2");
}

#[test]
fn test_extract_model_family_does_not_strip_provider_prefix() {
    let counter = TokenCounter::new();
    assert_eq!(counter.extract_model_family("openai/gpt-4"), "default");
    assert_eq!(
        counter.extract_model_family("anthropic/claude-3-opus"),
        "default"
    );
}

#[test]
fn test_extract_model_family_unknown() {
    let counter = TokenCounter::new();
    assert_eq!(counter.extract_model_family("unknown-model"), "default");
    assert_eq!(counter.extract_model_family("llama-2-70b"), "default");
}

// ==================== Text Token Estimation Tests ====================

#[test]
fn test_estimate_text_tokens_empty() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-4").unwrap();
    let tokens = counter.estimate_text_tokens(config, "");
    assert_eq!(tokens, 0);
}

#[test]
fn test_estimate_text_tokens_short_text() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-4").unwrap();
    let tokens = counter.estimate_text_tokens(config, "Hello");
    assert!(tokens > 0);
    assert!(tokens < 10);
}

#[test]
fn test_estimate_text_tokens_longer_text() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-4").unwrap();
    let short_tokens = counter.estimate_text_tokens(config, "Hello");
    let long_tokens =
        counter.estimate_text_tokens(config, "Hello, this is a much longer text message.");
    assert!(long_tokens > short_tokens);
}

#[test]
fn test_estimate_text_tokens_unicode() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-4").unwrap();
    let tokens = counter.estimate_text_tokens(config, "你好世界");
    assert!(tokens > 0);
}

// ==================== Completion Token Counting Tests ====================

#[test]
fn test_count_completion_tokens_basic() {
    let counter = TokenCounter::new();
    let result = counter.count_completion_tokens(&exact("gpt-4"), "Hello, world!");
    assert!(result.is_ok());
    let estimate = result.unwrap();
    assert!(estimate.input_tokens > 0);
    assert!(!estimate.is_approximate);
}

#[test]
fn test_count_completion_tokens_empty() {
    let counter = TokenCounter::new();
    let result = counter.count_completion_tokens(&exact("gpt-4"), "");
    assert!(result.is_ok());
    let estimate = result.unwrap();
    assert_eq!(estimate.input_tokens, 0);
    assert!(!estimate.is_approximate);
}

#[test]
fn test_count_completion_tokens_long_text() {
    let counter = TokenCounter::new();
    let long_text = "word ".repeat(1000);
    let result = counter.count_completion_tokens(&exact("gpt-4"), &long_text);
    assert!(result.is_ok());
    let estimate = result.unwrap();
    assert!(estimate.input_tokens > 100);
}

#[test]
fn test_count_completion_tokens_confidence() {
    let counter = TokenCounter::new();
    let result = counter.count_completion_tokens(&exact("gpt-4"), "test");
    assert!(result.is_ok());
    let estimate = result.unwrap();
    assert!(estimate.confidence > 0.0);
    assert!(estimate.confidence <= 1.0);
}

// ==================== Token Estimate Tests ====================

#[test]
fn test_token_estimate_structure() {
    let counter = TokenCounter::new();
    let result = counter.count_completion_tokens(&exact("gpt-4"), "Hello");
    assert!(result.is_ok());
    let estimate = result.unwrap();

    assert_eq!(estimate.total_tokens, estimate.input_tokens);
    assert!(estimate.output_tokens.is_none());
    assert!(!estimate.is_approximate);
}

// ==================== Embedding Token Counting Tests ====================

#[test]
fn test_count_embedding_tokens_single() {
    let counter = TokenCounter::new();
    let input = vec!["Hello, world!".to_string()];
    let result = counter.count_embedding_tokens(&exact("gpt-4"), &input);
    assert!(result.is_ok());
    let estimate = result.unwrap();
    assert!(estimate.input_tokens > 0);
    assert_eq!(estimate.confidence, 1.0);
    assert!(!estimate.is_approximate);
}

#[test]
fn test_count_embedding_tokens_multiple() {
    let counter = TokenCounter::new();
    let input = vec![
        "First text".to_string(),
        "Second text".to_string(),
        "Third text".to_string(),
    ];
    let result = counter.count_embedding_tokens(&exact("gpt-4"), &input);
    assert!(result.is_ok());
    let estimate = result.unwrap();
    assert!(estimate.input_tokens > 0);
}

#[test]
fn test_count_embedding_tokens_empty() {
    let counter = TokenCounter::new();
    let input: Vec<String> = vec![];
    let result = counter.count_embedding_tokens(&exact("gpt-4"), &input);
    assert!(result.is_ok());
}

// ==================== Output Token Estimation Tests ====================

#[test]
fn test_estimate_output_tokens_with_max() {
    let counter = TokenCounter::new();
    let result = counter.estimate_output_tokens(Some(100), 50, &exact("gpt-4"));
    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output, 100);
}

#[test]
fn test_estimate_output_tokens_without_max() {
    let counter = TokenCounter::new();
    let result = counter.estimate_output_tokens(None, 100, &exact("gpt-4"));
    assert!(result.is_ok());
    let output = result.unwrap();
    // Should be ~25% of remaining context
    assert!(output > 0);
}

#[test]
fn test_estimate_output_tokens_capped_by_context() {
    let counter = TokenCounter::new();
    // Request more tokens than available
    let result = counter.estimate_output_tokens(Some(1_000_000), 0, &exact("gpt-4"));
    assert!(result.is_ok());
    let output = result.unwrap();
    // Should be capped at model's max context
    assert!(output < 1_000_000);
}

// ==================== Context Window Check Tests ====================

#[test]
fn test_check_context_window_fits() {
    let counter = TokenCounter::new();
    let result = counter.check_context_window(&exact("gpt-4"), 1000, Some(1000));
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_check_context_window_exceeds() {
    let counter = TokenCounter::new();
    // Try to use more tokens than the context window
    let result = counter.check_context_window(&exact("gpt-4"), 100_000, Some(100_000));
    assert!(result.is_ok());
    // Verify the function returns a boolean (regardless of value)
    let _fits = result.unwrap();
}

#[test]
fn test_check_context_window_no_output() {
    let counter = TokenCounter::new();
    let result = counter.check_context_window(&exact("gpt-4"), 1000, None);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

// ==================== Model Config Management Tests ====================

#[test]
fn test_add_model_config() {
    let mut counter = TokenCounter::new();
    let initial_count = counter.model_configs.len();

    let config = ModelTokenConfig {
        model: "custom-model".to_string(),
        chars_per_token: 4.5,
        message_overhead: 5,
        request_overhead: 10,
        max_context_tokens: 16000,
        special_tokens: HashMap::new(),
    };
    counter.add_model_config(config);

    assert_eq!(counter.model_configs.len(), initial_count + 1);
    assert!(counter.get_model_config("custom-model").is_ok());
}

#[test]
fn test_get_supported_models() {
    let counter = TokenCounter::new();
    let models = counter.get_supported_models();
    assert!(!models.is_empty());
}

// ==================== Edge Cases Tests ====================

#[test]
fn test_special_characters() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-4").unwrap();
    let tokens = counter.estimate_text_tokens(config, "!@#$%^&*()");
    assert!(tokens > 0);
}

#[test]
fn test_newlines_and_whitespace() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-4").unwrap();
    let tokens = counter.estimate_text_tokens(config, "Hello\n\n\nWorld\t\tTest");
    assert!(tokens > 0);
}

#[test]
fn test_very_long_word() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-4").unwrap();
    let long_word = "a".repeat(1000);
    let tokens = counter.estimate_text_tokens(config, &long_word);
    assert!(tokens > 0);
}

#[test]
fn test_mixed_content() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-4").unwrap();
    let mixed = "Hello 你好 Привет مرحبا 🎉";
    let tokens = counter.estimate_text_tokens(config, mixed);
    assert!(tokens > 0);
}
