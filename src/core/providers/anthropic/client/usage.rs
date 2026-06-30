use serde_json::Value;

use crate::core::types::responses::{PromptTokensDetails, Usage};

pub(super) fn build_usage(usage_data: &Value) -> Usage {
    let read = |key: &str| usage_data.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
    // Saturate instead of `as u32`, which silently wraps on overflow.
    let to_u32 = |v: u64| u32::try_from(v).unwrap_or(u32::MAX);
    let cache_creation = read("cache_creation_input_tokens");
    let cache_read = read("cache_read_input_tokens");
    let input_tokens = read("input_tokens");
    let prompt_tokens = input_tokens
        .saturating_add(cache_creation)
        .saturating_add(cache_read);
    let completion_tokens = read("output_tokens");

    Usage {
        prompt_tokens: to_u32(prompt_tokens),
        completion_tokens: to_u32(completion_tokens),
        total_tokens: to_u32(prompt_tokens + completion_tokens),
        completion_tokens_details: None,
        prompt_tokens_details: if cache_creation > 0 || cache_read > 0 {
            Some(PromptTokensDetails {
                cached_tokens: Some(to_u32(cache_read)),
                cache_creation_tokens: Some(to_u32(cache_creation)),
                cache_read_tokens: Some(to_u32(cache_read)),
                audio_tokens: None,
            })
        } else {
            None
        },
        thinking_usage: None,
    }
}
