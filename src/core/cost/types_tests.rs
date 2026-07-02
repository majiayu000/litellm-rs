use super::*;

// ==================== Usage → UsageTokens Conversion Tests ====================

#[test]
fn test_usage_to_usage_tokens_basic() {
    use crate::core::types::responses::Usage;
    let usage = Usage::new(100, 50);
    let tokens: UsageTokens = usage.into();
    assert_eq!(tokens.prompt_tokens, 100);
    assert_eq!(tokens.completion_tokens, 50);
    assert_eq!(tokens.total_tokens, 150);
    assert!(tokens.cached_tokens.is_none());
    assert!(tokens.reasoning_tokens.is_none());
}

#[test]
fn test_usage_to_usage_tokens_with_details() {
    use crate::core::types::responses::{CompletionTokensDetails, PromptTokensDetails, Usage};
    let usage = Usage {
        prompt_tokens: 200,
        completion_tokens: 100,
        total_tokens: 300,
        prompt_tokens_details: Some(PromptTokensDetails {
            cached_tokens: Some(50),
            cache_creation_tokens: None,
            cache_read_tokens: Some(50),
            audio_tokens: Some(10),
        }),
        completion_tokens_details: Some(CompletionTokensDetails {
            reasoning_tokens: Some(30),
            audio_tokens: None,
        }),
        thinking_usage: None,
    };
    let tokens: UsageTokens = usage.into();
    assert_eq!(tokens.cached_tokens, Some(50));
    assert_eq!(tokens.audio_tokens, Some(10));
    assert_eq!(tokens.reasoning_tokens, Some(30));
    assert!(tokens.image_tokens.is_none());
}

// ==================== UsageTokens Tests ====================

#[test]
fn test_usage_tokens_new() {
    let usage = UsageTokens::new(100, 50);
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);
    assert!(usage.cached_tokens.is_none());
    assert!(usage.audio_tokens.is_none());
    assert!(usage.image_tokens.is_none());
    assert!(usage.reasoning_tokens.is_none());
}

#[test]
fn test_usage_tokens_zero() {
    let usage = UsageTokens::new(0, 0);
    assert_eq!(usage.total_tokens, 0);
}

#[test]
fn test_usage_tokens_clone() {
    let usage = UsageTokens::new(100, 50);
    let cloned = usage.clone();
    assert_eq!(usage.prompt_tokens, cloned.prompt_tokens);
    assert_eq!(usage.completion_tokens, cloned.completion_tokens);
}

// ==================== ModelPricing Tests ====================

#[test]
fn test_model_pricing_default() {
    let pricing = ModelPricing::default();
    assert!(pricing.model.is_empty());
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.0);
    assert_eq!(pricing.currency, "USD");
    assert!(pricing.cache_read_input_token_cost.is_none());
}

#[test]
fn test_model_pricing_serialization() {
    let pricing = ModelPricing {
        model: "gpt-4".to_string(),
        input_cost_per_1k_tokens: 0.03,
        output_cost_per_1k_tokens: 0.06,
        ..Default::default()
    };

    let json = serde_json::to_value(&pricing).unwrap();
    assert_eq!(json["model"], "gpt-4");
    assert_eq!(json["input_cost_per_1k_tokens"], 0.03);
    assert_eq!(json["output_cost_per_1k_tokens"], 0.06);
}

// ==================== CostEstimate Tests ====================

#[test]
fn test_cost_estimate_structure() {
    let estimate = CostEstimate {
        min_cost: 0.01,
        max_cost: 0.05,
        input_cost: 0.01,
        estimated_output_cost: 0.04,
        currency: "USD".to_string(),
    };

    assert_eq!(estimate.min_cost, 0.01);
    assert_eq!(estimate.max_cost, 0.05);
    assert_eq!(estimate.currency, "USD");
}

#[test]
fn test_cost_estimate_serialization() {
    let estimate = CostEstimate {
        min_cost: 0.01,
        max_cost: 0.05,
        input_cost: 0.01,
        estimated_output_cost: 0.04,
        currency: "USD".to_string(),
    };

    let json = serde_json::to_string(&estimate).unwrap();
    assert!(json.contains("min_cost"));
    assert!(json.contains("max_cost"));
}

// ==================== CostBreakdown Tests ====================

#[test]
fn test_cost_breakdown_new() {
    let usage = UsageTokens::new(100, 50);
    let breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);

    assert_eq!(breakdown.model, "gpt-4");
    assert_eq!(breakdown.provider, "openai");
    assert_eq!(breakdown.total_cost, 0.0);
    assert_eq!(breakdown.input_cost, 0.0);
    assert_eq!(breakdown.output_cost, 0.0);
    assert_eq!(breakdown.currency, "USD");
}

#[test]
fn test_cost_breakdown_calculate_total() {
    let usage = UsageTokens::new(100, 50);
    let mut breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);

    breakdown.input_cost = 0.01;
    breakdown.output_cost = 0.02;
    breakdown.cache_cost = 0.005;
    breakdown.audio_cost = 0.003;
    breakdown.image_cost = 0.002;
    breakdown.reasoning_cost = 0.001;

    breakdown.calculate_total();

    let expected = 0.01 + 0.02 + 0.005 + 0.003 + 0.002 + 0.001;
    assert!((breakdown.total_cost - expected).abs() < 1e-10);
}

#[test]
fn test_cost_breakdown_serialization() {
    let usage = UsageTokens::new(100, 50);
    let breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);

    let json = serde_json::to_value(&breakdown).unwrap();
    assert_eq!(json["model"], "gpt-4");
    assert_eq!(json["provider"], "openai");
}

// ==================== CostTracker Tests ====================

#[test]
fn test_cost_tracker_new() {
    let tracker = CostTracker::new();
    assert_eq!(tracker.total_cost(), 0.0);
    assert_eq!(tracker.request_count(), 0);
}

#[test]
fn test_cost_tracker_add_request_cost() {
    let mut tracker = CostTracker::new();

    let usage = UsageTokens::new(100, 50);
    let mut breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);
    breakdown.total_cost = 0.05;

    tracker.add_request_cost(breakdown);

    assert_eq!(tracker.total_cost(), 0.05);
    assert_eq!(tracker.request_count(), 1);
}

#[test]
fn test_cost_tracker_multiple_requests() {
    let mut tracker = CostTracker::new();

    for i in 0..5 {
        let usage = UsageTokens::new(100, 50);
        let mut breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);
        breakdown.total_cost = 0.01 * (i + 1) as f64;
        tracker.add_request_cost(breakdown);
    }

    assert_eq!(tracker.request_count(), 5);
    // 0.01 + 0.02 + 0.03 + 0.04 + 0.05 = 0.15
    assert!((tracker.total_cost() - 0.15).abs() < 1e-10);
}

#[test]
fn test_cost_tracker_average_cost_per_request() {
    let mut tracker = CostTracker::new();

    for _ in 0..4 {
        let usage = UsageTokens::new(100, 50);
        let mut breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);
        breakdown.total_cost = 0.02;
        tracker.add_request_cost(breakdown);
    }

    assert!((tracker.average_cost_per_request() - 0.02).abs() < 1e-10);
}

#[test]
fn test_cost_tracker_average_cost_empty() {
    let tracker = CostTracker::new();
    assert_eq!(tracker.average_cost_per_request(), 0.0);
}

#[test]
fn test_cost_tracker_cost_by_provider() {
    let mut tracker = CostTracker::new();

    let usage1 = UsageTokens::new(100, 50);
    let mut breakdown1 = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage1);
    breakdown1.total_cost = 0.05;
    tracker.add_request_cost(breakdown1);

    let usage2 = UsageTokens::new(100, 50);
    let mut breakdown2 =
        CostBreakdown::new("claude-3".to_string(), "anthropic".to_string(), usage2);
    breakdown2.total_cost = 0.03;
    tracker.add_request_cost(breakdown2);

    assert!((tracker.cost_by_provider("openai") - 0.05).abs() < 1e-10);
    assert!((tracker.cost_by_provider("anthropic") - 0.03).abs() < 1e-10);
    assert_eq!(tracker.cost_by_provider("unknown"), 0.0);
}

#[test]
fn test_cost_tracker_cost_by_model() {
    let mut tracker = CostTracker::new();

    let usage1 = UsageTokens::new(100, 50);
    let mut breakdown1 = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage1);
    breakdown1.total_cost = 0.05;
    tracker.add_request_cost(breakdown1);

    let usage2 = UsageTokens::new(100, 50);
    let mut breakdown2 = CostBreakdown::new("gpt-3.5".to_string(), "openai".to_string(), usage2);
    breakdown2.total_cost = 0.01;
    tracker.add_request_cost(breakdown2);

    assert!((tracker.cost_by_model("gpt-4") - 0.05).abs() < 1e-10);
    assert!((tracker.cost_by_model("gpt-3.5") - 0.01).abs() < 1e-10);
    assert_eq!(tracker.cost_by_model("unknown"), 0.0);
}

#[test]
fn test_cost_tracker_most_expensive_request() {
    let mut tracker = CostTracker::new();

    let costs = [0.01, 0.05, 0.02, 0.03];
    for cost in costs {
        let usage = UsageTokens::new(100, 50);
        let mut breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);
        breakdown.total_cost = cost;
        tracker.add_request_cost(breakdown);
    }

    let most_expensive = tracker.most_expensive_request().unwrap();
    assert!((most_expensive.total_cost - 0.05).abs() < 1e-10);
}

#[test]
fn test_cost_tracker_cheapest_request() {
    let mut tracker = CostTracker::new();

    let costs = [0.01, 0.05, 0.02, 0.03];
    for cost in costs {
        let usage = UsageTokens::new(100, 50);
        let mut breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);
        breakdown.total_cost = cost;
        tracker.add_request_cost(breakdown);
    }

    let cheapest = tracker.cheapest_request().unwrap();
    assert!((cheapest.total_cost - 0.01).abs() < 1e-10);
}

#[test]
fn test_cost_tracker_most_expensive_empty() {
    let tracker = CostTracker::new();
    assert!(tracker.most_expensive_request().is_none());
}

#[test]
fn test_cost_tracker_get_summary() {
    let mut tracker = CostTracker::new();

    let usage1 = UsageTokens::new(100, 50);
    let mut breakdown1 = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage1);
    breakdown1.total_cost = 0.05;
    breakdown1.input_cost = 0.03;
    breakdown1.output_cost = 0.02;
    tracker.add_request_cost(breakdown1);

    let usage2 = UsageTokens::new(200, 100);
    let mut breakdown2 = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage2);
    breakdown2.total_cost = 0.10;
    breakdown2.input_cost = 0.06;
    breakdown2.output_cost = 0.04;
    tracker.add_request_cost(breakdown2);

    let summary = tracker.get_summary();

    assert_eq!(summary.total_requests, 2);
    assert!((summary.total_cost - 0.15).abs() < 1e-10);
    assert_eq!(summary.total_input_tokens, 300);
    assert_eq!(summary.total_output_tokens, 150);
    assert_eq!(summary.total_tokens, 450);
    assert!((summary.total_input_cost - 0.09).abs() < 1e-10);
    assert!((summary.total_output_cost - 0.06).abs() < 1e-10);
    assert_eq!(summary.currency, "USD");
}

// ==================== CostResult Tests ====================

#[test]
fn test_cost_result_new() {
    let result = CostResult::new(0.05, 0.10);
    assert_eq!(result.input_cost, 0.05);
    assert_eq!(result.output_cost, 0.10);
    assert!((result.total_cost - 0.15).abs() < 1e-10);
    assert!(result.additional_costs.is_empty());
}

#[test]
fn test_cost_result_with_additional_cost() {
    let result = CostResult::new(0.05, 0.10)
        .with_additional_cost("cache".to_string(), 0.02)
        .with_additional_cost("audio".to_string(), 0.01);

    assert!((result.total_cost - 0.18).abs() < 1e-10);
    assert_eq!(result.additional_costs.len(), 2);
    assert_eq!(result.additional_costs.get("cache"), Some(&0.02));
    assert_eq!(result.additional_costs.get("audio"), Some(&0.01));
}

// ==================== CostError Tests ====================

#[test]
fn test_cost_error_model_not_supported() {
    let error = CostError::ModelNotSupported {
        model: "unknown-model".to_string(),
        provider: "openai".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("unknown-model"));
    assert!(msg.contains("openai"));
}

#[test]
fn test_cost_error_provider_not_supported() {
    let error = CostError::ProviderNotSupported {
        provider: "unknown-provider".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("unknown-provider"));
}

#[test]
fn test_cost_error_missing_pricing() {
    let error = CostError::MissingPricing {
        model: "gpt-4".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("gpt-4"));
}

#[test]
fn test_cost_error_invalid_usage() {
    let error = CostError::InvalidUsage {
        message: "negative tokens".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("negative tokens"));
}

#[test]
fn test_cost_error_calculation_error() {
    let error = CostError::CalculationError {
        message: "overflow occurred".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("overflow occurred"));
}

#[test]
fn test_cost_error_config_error() {
    let error = CostError::ConfigError {
        message: "invalid config".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("invalid config"));
}

#[test]
fn test_cost_error_clone() {
    let error = CostError::ModelNotSupported {
        model: "test".to_string(),
        provider: "test".to_string(),
    };
    let cloned = error.clone();
    assert_eq!(error.to_string(), cloned.to_string());
}

// ==================== ModelCostComparison Tests ====================

#[test]
fn test_model_cost_comparison_structure() {
    let comparison = ModelCostComparison {
        model: "gpt-4".to_string(),
        provider: "openai".to_string(),
        total_cost: 0.05,
        cost_per_token: 0.00005,
        efficiency_score: 20000.0,
    };

    assert_eq!(comparison.model, "gpt-4");
    assert_eq!(comparison.provider, "openai");
    assert_eq!(comparison.total_cost, 0.05);
}

#[test]
fn test_model_cost_comparison_serialization() {
    let comparison = ModelCostComparison {
        model: "gpt-4".to_string(),
        provider: "openai".to_string(),
        total_cost: 0.05,
        cost_per_token: 0.00005,
        efficiency_score: 20000.0,
    };

    let json = serde_json::to_value(&comparison).unwrap();
    assert_eq!(json["model"], "gpt-4");
    assert_eq!(json["total_cost"], 0.05);
}

// ==================== ProviderPricing Tests ====================

#[test]
fn test_provider_pricing_structure() {
    let mut model_pricing = HashMap::new();
    model_pricing.insert(
        "gpt-4".to_string(),
        ModelPricing {
            model: "gpt-4".to_string(),
            input_cost_per_1k_tokens: 0.03,
            output_cost_per_1k_tokens: 0.06,
            ..Default::default()
        },
    );

    let provider_pricing = ProviderPricing {
        provider: "openai".to_string(),
        default_pricing: None,
        model_pricing,
    };

    assert_eq!(provider_pricing.provider, "openai");
    assert!(provider_pricing.model_pricing.contains_key("gpt-4"));
}

// ==================== CostSummary Tests ====================

#[test]
fn test_cost_summary_serialization() {
    let summary = CostSummary {
        total_cost: 0.15,
        total_requests: 2,
        total_input_tokens: 300,
        total_output_tokens: 150,
        total_tokens: 450,
        total_input_cost: 0.09,
        total_output_cost: 0.06,
        average_cost_per_request: 0.075,
        provider_breakdown: HashMap::new(),
        model_breakdown: HashMap::new(),
        currency: "USD".to_string(),
    };

    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["total_cost"], 0.15);
    assert_eq!(json["total_requests"], 2);
    assert_eq!(json["currency"], "USD");
}
