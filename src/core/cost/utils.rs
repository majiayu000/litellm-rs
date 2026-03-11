//! Cost Calculation Utilities
//!
//! Helper functions for cost calculations, similar to Python's utils.py

use crate::core::cost::types::{CostError, ModelPricing, UsageTokens};

/// Get cost per unit with type safety
pub fn get_cost_per_unit(pricing: &ModelPricing, cost_key: &str) -> f64 {
    match cost_key {
        "input_cost_per_1k_tokens" => pricing.input_cost_per_1k_tokens,
        "output_cost_per_1k_tokens" => pricing.output_cost_per_1k_tokens,
        "cache_read_input_token_cost" => pricing.cache_read_input_token_cost.unwrap_or(0.0),
        "cache_creation_input_token_cost" => pricing.cache_creation_input_token_cost.unwrap_or(0.0),
        "input_cost_per_audio_token" => pricing.input_cost_per_audio_token.unwrap_or(0.0),
        "output_cost_per_audio_token" => pricing.output_cost_per_audio_token.unwrap_or(0.0),
        "image_cost_per_token" => pricing.image_cost_per_token.unwrap_or(0.0),
        "reasoning_cost_per_token" => pricing.reasoning_cost_per_token.unwrap_or(0.0),
        "cost_per_second" => pricing.cost_per_second.unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Calculate cost component generically
pub fn calculate_cost_component(
    pricing: &ModelPricing,
    cost_key: &str,
    usage_value: Option<f64>,
) -> f64 {
    let cost_per_unit = get_cost_per_unit(pricing, cost_key);

    if let Some(value) = usage_value
        && cost_per_unit > 0.0
        && value > 0.0
    {
        return value * cost_per_unit;
    }

    0.0
}

/// Select tiered pricing based on token usage (like Python's tiered pricing logic)
///
/// Returns: (input_cost_per_1k, output_cost_per_1k, cache_creation_cost_per_1k, cache_read_cost_per_1k)
pub fn select_tiered_pricing(pricing: &ModelPricing, usage: &UsageTokens) -> (f64, f64, f64, f64) {
    let mut input_cost = pricing.input_cost_per_1k_tokens;
    let mut output_cost = pricing.output_cost_per_1k_tokens;
    let mut cache_creation_cost = pricing.cache_creation_input_token_cost.unwrap_or(0.0);
    let mut cache_read_cost = pricing.cache_read_input_token_cost.unwrap_or(0.0);

    // Check for tiered pricing (e.g., above 128k tokens)
    if let Some(ref tiered_pricing) = pricing.tiered_pricing {
        // Sort thresholds in descending order to apply highest applicable tier
        let mut thresholds: Vec<_> = tiered_pricing.iter().collect();
        thresholds.sort_by(|a, b| {
            extract_threshold(a.0)
                .partial_cmp(&extract_threshold(b.0))
                .unwrap_or(std::cmp::Ordering::Equal)
                .reverse()
        });

        for (key, &cost) in thresholds {
            if let Some(threshold) = extract_threshold(key)
                && usage.prompt_tokens as f64 > threshold
            {
                if key.starts_with("input_cost_per_token_above_") {
                    input_cost = cost;
                } else if key.starts_with("output_cost_per_token_above_") {
                    output_cost = cost;
                } else if key.starts_with("cache_creation_input_token_cost_above_") {
                    cache_creation_cost = cost;
                } else if key.starts_with("cache_read_input_token_cost_above_") {
                    cache_read_cost = cost;
                }
                break; // Apply only the first (highest) applicable tier
            }
        }
    }

    (
        input_cost,
        output_cost,
        cache_creation_cost,
        cache_read_cost,
    )
}

/// Extract threshold value from tiered pricing key
/// e.g., "input_cost_per_token_above_128k_tokens" -> Some(128000.0)
///       "input_cost_per_token_above_100_tokens" -> Some(100.0)
fn extract_threshold(key: &str) -> Option<f64> {
    if let Some(above_part) = key.split("_above_").nth(1)
        && let Some(threshold_str) = above_part.split("_tokens").next()
    {
        if let Some(number_str) = threshold_str.strip_suffix('k') {
            if let Ok(number) = number_str.parse::<f64>() {
                return Some(number * 1000.0);
            }
        } else if let Ok(number) = threshold_str.parse::<f64>() {
            return Some(number);
        }
    }
    None
}

/// Check if token count is above threshold
pub fn is_above_threshold(tokens: u32, threshold: u32) -> bool {
    tokens > threshold
}

/// Convert tokens to cost
pub fn tokens_to_cost(tokens: u32, cost_per_1k: f64) -> f64 {
    (tokens as f64 / 1000.0) * cost_per_1k
}

/// Convert cost to estimated tokens
pub fn cost_to_tokens(cost: f64, cost_per_1k: f64) -> u32 {
    if cost_per_1k > 0.0 {
        ((cost / cost_per_1k) * 1000.0) as u32
    } else {
        0
    }
}

/// Format cost for display
pub fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("${:.6}", cost)
    } else if cost < 1.0 {
        format!("${:.4}", cost)
    } else {
        format!("${:.2}", cost)
    }
}

/// Calculate cost savings between two providers
pub fn calculate_savings(cost1: f64, cost2: f64) -> f64 {
    if cost2 > 0.0 {
        ((cost2 - cost1) / cost2) * 100.0
    } else {
        0.0
    }
}

/// Estimate monthly cost based on usage patterns
pub fn estimate_monthly_cost(
    daily_requests: u32,
    avg_input_tokens: u32,
    avg_output_tokens: u32,
    input_cost_per_1k: f64,
    output_cost_per_1k: f64,
) -> f64 {
    let daily_input_cost = tokens_to_cost(daily_requests * avg_input_tokens, input_cost_per_1k);
    let daily_output_cost = tokens_to_cost(daily_requests * avg_output_tokens, output_cost_per_1k);
    (daily_input_cost + daily_output_cost) * 30.0
}

/// Calculate cost efficiency score (higher is better)
pub fn calculate_efficiency_score(total_tokens: u32, total_cost: f64) -> f64 {
    if total_cost > 0.0 {
        total_tokens as f64 / total_cost
    } else {
        0.0
    }
}

/// Validate usage data
pub fn validate_usage(usage: &UsageTokens) -> Result<(), CostError> {
    if usage.prompt_tokens == 0 && usage.completion_tokens == 0 {
        return Err(CostError::InvalidUsage {
            message: "Both prompt and completion tokens cannot be zero".to_string(),
        });
    }

    if usage.total_tokens != usage.prompt_tokens + usage.completion_tokens {
        return Err(CostError::InvalidUsage {
            message: "Total tokens does not match sum of prompt and completion tokens".to_string(),
        });
    }

    if let Some(cached) = usage.cached_tokens
        && cached > usage.prompt_tokens
    {
        return Err(CostError::InvalidUsage {
            message: "Cached tokens cannot exceed prompt tokens".to_string(),
        });
    }

    Ok(())
}

/// Get model category for cost optimization suggestions
pub fn get_model_category(model: &str) -> &'static str {
    let model_lower = model.to_lowercase();

    if model_lower.contains("gpt-5.2")
        || model_lower.contains("gpt-5.1")
        || model_lower.contains("gpt-5-codex")
        || model_lower.contains("o3-pro")
        || model_lower.contains("gpt-4o")
        || model_lower.contains("claude-opus-4-6")
        || model_lower.contains("claude-opus-4-5")
    {
        "flagship"
    } else if model_lower.contains("gpt-4.1")
        || model_lower.contains("o3-mini")
        || model_lower.contains("o4-mini")
        || model_lower.contains("gpt-4")
        || model_lower.contains("claude-sonnet-4-5")
        || model_lower.contains("claude-sonnet-4")
        || model_lower.contains("claude-3-5-sonnet")
        || model_lower.contains("claude-3-sonnet")
    {
        "advanced"
    } else if model_lower.contains("gpt-3.5")
        || model_lower.contains("claude-3-5-haiku")
        || model_lower.contains("claude-3-haiku")
    {
        "efficient"
    } else if model_lower.contains("mini") || model_lower.contains("nano") {
        "lightweight"
    } else {
        "unknown"
    }
}

/// Suggest cost optimizations
pub fn suggest_optimizations(
    current_model: &str,
    monthly_cost: f64,
    usage_pattern: &str, // "frequent", "occasional", "batch"
) -> Vec<String> {
    let mut suggestions = Vec::new();
    let category = get_model_category(current_model);

    match (category, usage_pattern) {
        ("flagship", "occasional") => {
            suggestions.push(
                "Consider using GPT-5 Mini, GPT-4.1 Mini, or Claude Sonnet variants for occasional use".to_string(),
            );
        }
        ("flagship", "batch") => {
            suggestions.push(
                "For batch processing, consider efficient models like GPT-5 Nano or GPT-4.1 Nano"
                    .to_string(),
            );
        }
        ("advanced", "frequent") if monthly_cost > 100.0 => {
            suggestions
                .push("For frequent use, consider prompt caching to reduce costs".to_string());
        }
        _ => {}
    }

    if monthly_cost > 50.0 {
        suggestions.push("Consider setting up cost alerts and usage limits".to_string());
    }

    if monthly_cost > 200.0 {
        suggestions.push("Evaluate enterprise pricing tiers for better rates".to_string());
    }

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_threshold() {
        assert_eq!(
            extract_threshold("input_cost_per_token_above_128k_tokens"),
            Some(128000.0)
        );
        assert_eq!(
            extract_threshold("input_cost_per_token_above_100_tokens"),
            Some(100.0)
        );
        assert_eq!(extract_threshold("invalid_key"), None);
    }

    #[test]
    fn test_tokens_to_cost() {
        assert_eq!(tokens_to_cost(1000, 1.0), 1.0);
        assert_eq!(tokens_to_cost(500, 2.0), 1.0);
        assert_eq!(tokens_to_cost(0, 1.0), 0.0);
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.001234), "$0.001234");
        assert_eq!(format_cost(0.1234), "$0.1234");
        assert_eq!(format_cost(1.234), "$1.23");
    }
}
