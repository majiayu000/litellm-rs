use serde_json::Value;

use crate::core::types::responses::{PromptTokensDetails, Usage};

pub(super) fn build_usage(usage_data: &Value) -> Usage {
    let read = |key: &str| usage_data.get(key).and_then(Value::as_u64);
    build_usage_from_parts(
        read("input_tokens"),
        read("output_tokens"),
        read("cache_creation_input_tokens"),
        read("cache_read_input_tokens"),
    )
}

pub(crate) fn build_usage_from_parts(
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation: Option<u64>,
    cache_read: Option<u64>,
) -> Usage {
    // Saturate instead of `as u32`, which silently wraps on overflow.
    let to_u32 = |v: u64| u32::try_from(v).unwrap_or(u32::MAX);
    let input_tokens = input_tokens.unwrap_or(0);
    let prompt_tokens = input_tokens
        .saturating_add(cache_creation.unwrap_or(0))
        .saturating_add(cache_read.unwrap_or(0));
    let completion_tokens = output_tokens.unwrap_or(0);

    Usage {
        prompt_tokens: to_u32(prompt_tokens),
        completion_tokens: to_u32(completion_tokens),
        total_tokens: to_u32(prompt_tokens.saturating_add(completion_tokens)),
        completion_tokens_details: None,
        prompt_tokens_details: if cache_creation.is_some() || cache_read.is_some() {
            Some(PromptTokensDetails {
                cached_tokens: cache_read.map(to_u32),
                cache_creation_tokens: cache_creation.map(to_u32),
                cache_read_tokens: cache_read.map(to_u32),
                audio_tokens: None,
            })
        } else {
            None
        },
        thinking_usage: None,
    }
}
