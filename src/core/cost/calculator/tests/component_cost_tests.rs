use super::*;

// Tests for calculate_input_cost
#[test]
fn test_calculate_input_cost_no_cache() {
    let usage = create_usage(1000, 500);
    let cost = calculate_input_cost(&usage, 1.0);
    assert_eq!(cost, 1.0);
}

#[test]
fn test_calculate_input_cost_with_cache() {
    let mut usage = create_usage(2000, 500);
    usage.cached_tokens = Some(500);
    let cost = calculate_input_cost(&usage, 1.0);
    // Should only charge for 1500 non-cached tokens
    assert_eq!(cost, 1.5);
}

#[test]
fn test_calculate_input_cost_all_cached() {
    let mut usage = create_usage(1000, 500);
    usage.cached_tokens = Some(1000);
    let cost = calculate_input_cost(&usage, 1.0);
    // All tokens cached, should be 0
    assert_eq!(cost, 0.0);
}

#[test]
fn test_calculate_input_cost_zero_tokens() {
    let usage = create_usage(0, 500);
    let cost = calculate_input_cost(&usage, 1.0);
    assert_eq!(cost, 0.0);
}

// Tests for calculate_output_cost
#[test]
fn test_calculate_output_cost_basic() {
    let usage = create_usage(1000, 500);
    let cost = calculate_output_cost(&usage, 2.0);
    assert_eq!(cost, 1.0); // 500 / 1000 * 2.0
}

#[test]
fn test_calculate_output_cost_zero() {
    let usage = create_usage(1000, 0);
    let cost = calculate_output_cost(&usage, 2.0);
    assert_eq!(cost, 0.0);
}

// Tests for calculate_cache_cost
#[test]
fn test_calculate_cache_cost() {
    let cost = calculate_cache_cost(1000, 0.5, 0.1);
    // Using read cost: 1000 / 1000 * 0.1 = 0.1
    assert_eq!(cost, 0.1);
}

#[test]
fn test_calculate_cache_cost_zero_tokens() {
    let cost = calculate_cache_cost(0, 0.5, 0.1);
    assert_eq!(cost, 0.0);
}

// Tests for calculate_audio_cost
#[test]
fn test_calculate_audio_cost_with_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        input_cost_per_audio_token: Some(0.001),
        ..Default::default()
    };

    let cost = calculate_audio_cost(&pricing, 1000);
    assert_eq!(cost, 1.0); // 1000 * 0.001
}

#[test]
fn test_calculate_audio_cost_no_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        ..Default::default()
    };

    let cost = calculate_audio_cost(&pricing, 1000);
    assert_eq!(cost, 0.0);
}

// Tests for calculate_image_cost
#[test]
fn test_calculate_image_cost_with_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        image_cost_per_token: Some(0.002),
        ..Default::default()
    };

    let cost = calculate_image_cost(&pricing, 500);
    assert_eq!(cost, 1.0); // 500 * 0.002
}

#[test]
fn test_calculate_image_cost_no_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        ..Default::default()
    };

    let cost = calculate_image_cost(&pricing, 500);
    assert_eq!(cost, 0.0);
}

// Tests for calculate_reasoning_cost
#[test]
fn test_calculate_reasoning_cost_with_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        reasoning_cost_per_token: Some(0.003),
        ..Default::default()
    };

    let cost = calculate_reasoning_cost(&pricing, 300);
    assert_eq!(cost, 0.9); // 300 * 0.003
}

#[test]
fn test_calculate_reasoning_cost_no_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        ..Default::default()
    };

    let cost = calculate_reasoning_cost(&pricing, 300);
    assert_eq!(cost, 0.0);
}
