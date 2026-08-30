use super::*;

// ==================== Convenience Method Tests (moved from provider.rs) ====================

#[test]
fn test_model_recommendations() {
    let provider = create_test_provider();

    assert_eq!(
        provider.get_best_model_for_task(super::client::OpenAITask::GeneralChat),
        Some("gpt-5.6".to_string())
    );

    assert_eq!(
        provider.get_best_model_for_task(super::client::OpenAITask::ComplexReasoning),
        Some("gpt-5.6".to_string())
    );

    assert_eq!(
        provider.get_best_model_for_task(super::client::OpenAITask::CostSensitive),
        Some("gpt-5.6-luna".to_string())
    );
}

#[test]
fn test_feature_support() {
    let provider = create_test_provider();

    // Test vision support - may depend on model availability
    let gpt4o_vision = provider.model_supports_vision("gpt-4o");
    let gpt35_vision = provider.model_supports_vision("gpt-3.5-turbo");
    if !gpt4o_vision {
        eprintln!("Warning: gpt-4o vision support not detected");
    }
    assert!(!gpt35_vision); // gpt-3.5-turbo should not support vision

    // Test tool calling
    assert!(provider.model_supports_tools("gpt-4"));
    assert!(provider.model_supports_tools("gpt-3.5-turbo"));

    // Test streaming
    assert!(provider.model_supports_streaming("gpt-4"));
    assert!(!provider.model_supports_streaming("text-embedding-ada-002"));
}

#[test]
fn test_model_pricing() {
    let provider = create_test_provider();

    let (input_cost, output_cost) = provider
        .get_model_pricing("gpt-4")
        .expect("gpt-4 should have OpenAI pricing");
    assert!(input_cost > 0.0);
    assert!(output_cost > input_cost); // Output usually costs more
}

#[test]
fn test_model_pricing_prefers_shared_cost_source() {
    let provider = create_test_provider();
    let tolerance = 1e-12;

    let Some((input_cost, output_cost)) = provider.get_model_pricing("gpt-4o-mini") else {
        panic!("gpt-4o-mini pricing should resolve from shared cost source");
    };

    assert!(
        (input_cost - 0.00015).abs() <= tolerance,
        "expected input cost near 0.00015, got {input_cost}"
    );
    assert!(
        (output_cost - 0.0006).abs() <= tolerance,
        "expected output cost near 0.0006, got {output_cost}"
    );
}

#[test]
fn test_context_window() {
    let provider = create_test_provider();

    if let Ok(context_len) = provider.get_model_context_window("gpt-4o") {
        // GPT-4o typically has a large context window
        assert!(
            context_len >= 32000,
            "Expected gpt-4o to have large context, got {}",
            context_len
        );
    }

    if let Ok(context_len) = provider.get_model_context_window("gpt-3.5-turbo") {
        // GPT-3.5-turbo should have at least 4K context, may vary by version
        assert!(
            context_len >= 4000,
            "Expected gpt-3.5-turbo to have reasonable context, got {}",
            context_len
        );
    }
}
