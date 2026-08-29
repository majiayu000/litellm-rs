use super::get_anthropic_registry;

/// Cost calculation utility
pub struct CostCalculator;

impl CostCalculator {
    /// Calculate basic cost
    pub fn calculate_cost(
        model_id: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Option<f64> {
        let usage = crate::core::cost::types::UsageTokens::new(prompt_tokens, completion_tokens);
        if let Ok(breakdown) =
            crate::core::cost::calculator::generic_cost_per_token(model_id, &usage, "anthropic")
        {
            return Some(breakdown.total_cost);
        }

        let registry = get_anthropic_registry();
        let pricing = registry.get_core_model_pricing(model_id)?;

        let input_cost = (prompt_tokens as f64 / 1000.0) * pricing.input_cost_per_1k_tokens;
        let output_cost = (completion_tokens as f64 / 1000.0) * pricing.output_cost_per_1k_tokens;

        Some(input_cost + output_cost)
    }

    /// Calculate extended cost (including cache)
    pub fn calculate_extended_cost(
        model_id: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
        cache_read_tokens: Option<u32>,
        cache_write_tokens: Option<u32>,
        is_batch: bool,
    ) -> Option<f64> {
        let pricing =
            crate::core::cost::calculator::get_model_pricing(model_id, "anthropic").ok()?;

        let batch_multiplier = if is_batch {
            pricing.batch_discount.unwrap_or(1.0)
        } else {
            1.0
        };

        let mut total_cost = 0.0;
        let mut remaining_prompt_tokens = prompt_tokens;

        // Handle cache read tokens
        if let (Some(cache_read), Some(cache_read_price)) =
            (cache_read_tokens, pricing.cache_read_input_token_cost)
        {
            let cache_cost = (cache_read as f64 / 1000.0) * cache_read_price;
            total_cost += cache_cost;
            remaining_prompt_tokens = remaining_prompt_tokens.saturating_sub(cache_read);
        }

        // Handle cache write tokens
        if let (Some(cache_write), Some(cache_write_price)) =
            (cache_write_tokens, pricing.cache_creation_input_token_cost)
        {
            let cache_write_cost =
                (cache_write as f64 / 1000.0) * cache_write_price * batch_multiplier;
            total_cost += cache_write_cost;
            remaining_prompt_tokens = remaining_prompt_tokens.saturating_sub(cache_write);
        }

        // Regular input tokens
        let input_cost = (remaining_prompt_tokens as f64 / 1000.0)
            * pricing.input_cost_per_1k_tokens
            * batch_multiplier;
        total_cost += input_cost;

        // Output tokens
        let output_cost = (completion_tokens as f64 / 1000.0)
            * pricing.output_cost_per_1k_tokens
            * batch_multiplier;
        total_cost += output_cost;

        Some(total_cost)
    }

    /// Estimate token count
    pub fn estimate_tokens(text: &str) -> u32 {
        // Anthropic uses approximately 4 characters = 1 token ratio (English)
        (text.len() as f32 / 4.0).ceil() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::CostCalculator;

    #[test]
    fn extended_cost_uses_shared_claude_5_cache_pricing() {
        let cost = CostCalculator::calculate_extended_cost(
            "claude-fable-5",
            1_000,
            100,
            Some(200),
            Some(300),
            false,
        )
        .expect("Claude 5 pricing should resolve from the shared authority");

        assert!((cost - 0.01395).abs() < 1e-12, "unexpected cost: {cost}");
    }
}
