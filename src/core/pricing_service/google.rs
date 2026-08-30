//! Exact catalog resolution rules for Google pricing surfaces.

use std::borrow::Cow;

use chrono::{DateTime, Utc};

use super::types::LiteLLMModelInfo;

const GEMINI_FLASH_STANDARD_PRICING_START_UTC: i64 = 1_798_761_600;
const PROMOTIONAL_INPUT: f64 = 7.5e-7;
const PROMOTIONAL_OUTPUT: f64 = 3.75e-6;
const PROMOTIONAL_CACHE_READ: f64 = 7.5e-8;
const STANDARD_INPUT: f64 = 1.5e-6;
const STANDARD_OUTPUT: f64 = 7.5e-6;
const STANDARD_CACHE_READ: f64 = 1.5e-7;
const PROMOTIONAL_PRIORITY_INPUT: f64 = 1.35e-6;
const PROMOTIONAL_PRIORITY_OUTPUT: f64 = 6.75e-6;
const PROMOTIONAL_PRIORITY_CACHE_READ: f64 = 1.35e-7;
const STANDARD_PRIORITY_INPUT: f64 = 2.7e-6;
const STANDARD_PRIORITY_OUTPUT: f64 = 13.5e-6;
const STANDARD_PRIORITY_CACHE_READ: f64 = 2.7e-7;

pub(super) fn effective_model_info_at<'a>(
    requested_provider: &str,
    resolved_model: &str,
    model_info: &'a LiteLLMModelInfo,
    pricing_time: DateTime<Utc>,
) -> Cow<'a, LiteLLMModelInfo> {
    let requested_provider = crate::core::pricing::normalize_pricing_provider(requested_provider);
    let catalog_provider =
        crate::core::pricing::normalize_pricing_provider(&model_info.litellm_provider);
    if !uses_google_completion_calculator(&requested_provider, &catalog_provider) {
        return Cow::Borrowed(model_info);
    }

    let local_model = crate::core::types::model_id::ModelIdRef::parse(resolved_model).model();
    if !matches!(local_model, "gemini-3.6-flash" | "gemini-3.7-flash")
        || !has_official_flash_source(model_info)
    {
        return Cow::Borrowed(model_info);
    }

    let is_promotional = has_complete_rate_signature(
        model_info,
        PROMOTIONAL_INPUT,
        PROMOTIONAL_OUTPUT,
        PROMOTIONAL_CACHE_READ,
    );
    let is_standard = has_complete_rate_signature(
        model_info,
        STANDARD_INPUT,
        STANDARD_OUTPUT,
        STANDARD_CACHE_READ,
    );
    let use_standard = pricing_time.timestamp() >= GEMINI_FLASH_STANDARD_PRICING_START_UTC;

    match (use_standard, is_promotional, is_standard) {
        (true, true, false) => model_info_with_rates(
            model_info,
            STANDARD_INPUT,
            STANDARD_OUTPUT,
            STANDARD_CACHE_READ,
        ),
        (false, false, true) => model_info_with_rates(
            model_info,
            PROMOTIONAL_INPUT,
            PROMOTIONAL_OUTPUT,
            PROMOTIONAL_CACHE_READ,
        ),
        _ => Cow::Borrowed(model_info),
    }
}

pub(super) fn maximum_scheduled_model_info<'a>(
    requested_provider: &str,
    resolved_model: &str,
    model_info: &'a LiteLLMModelInfo,
) -> Cow<'a, LiteLLMModelInfo> {
    let requested_provider = crate::core::pricing::normalize_pricing_provider(requested_provider);
    let catalog_provider =
        crate::core::pricing::normalize_pricing_provider(&model_info.litellm_provider);
    let local_model = crate::core::types::model_id::ModelIdRef::parse(resolved_model).model();
    if uses_google_completion_calculator(&requested_provider, &catalog_provider)
        && matches!(local_model, "gemini-3.6-flash" | "gemini-3.7-flash")
        && has_official_flash_source(model_info)
        && has_complete_rate_signature(
            model_info,
            PROMOTIONAL_INPUT,
            PROMOTIONAL_OUTPUT,
            PROMOTIONAL_CACHE_READ,
        )
    {
        model_info_with_rates(
            model_info,
            STANDARD_INPUT,
            STANDARD_OUTPUT,
            STANDARD_CACHE_READ,
        )
    } else {
        Cow::Borrowed(model_info)
    }
}

fn has_official_flash_source(model_info: &LiteLLMModelInfo) -> bool {
    matches!(
        model_info
            .extra
            .get("source")
            .and_then(serde_json::Value::as_str),
        Some("https://ai.google.dev/gemini-api/docs/pricing")
            | Some("https://cloud.google.com/vertex-ai/generative-ai/pricing")
    )
}

fn has_complete_rate_signature(
    model_info: &LiteLLMModelInfo,
    input: f64,
    output: f64,
    cache_read: f64,
) -> bool {
    let (priority_input, priority_output, priority_cache_read) = priority_rates(input);
    model_info.input_cost_per_token == Some(input)
        && model_info.output_cost_per_token == Some(output)
        && required_extra_rate_matches(model_info, "cache_read_input_token_cost", cache_read)
        && optional_extra_rate_matches(model_info, "output_cost_per_reasoning_token", output)
        && optional_extra_rate_matches(model_info, "input_cost_per_token_batches", input / 2.0)
        && optional_extra_rate_matches(model_info, "output_cost_per_token_batches", output / 2.0)
        && optional_extra_rate_matches(
            model_info,
            "cache_read_input_token_cost_batches",
            cache_read / 2.0,
        )
        && optional_extra_rate_matches(model_info, "input_cost_per_token_flex", input / 2.0)
        && optional_extra_rate_matches(model_info, "output_cost_per_token_flex", output / 2.0)
        && optional_extra_rate_matches(
            model_info,
            "cache_read_input_token_cost_flex",
            cache_read / 2.0,
        )
        && optional_extra_rate_matches(model_info, "input_cost_per_token_priority", priority_input)
        && optional_extra_rate_matches(
            model_info,
            "output_cost_per_token_priority",
            priority_output,
        )
        && optional_extra_rate_matches(
            model_info,
            "cache_read_input_token_cost_priority",
            priority_cache_read,
        )
}

fn priority_rates(input: f64) -> (f64, f64, f64) {
    if input == PROMOTIONAL_INPUT {
        (
            PROMOTIONAL_PRIORITY_INPUT,
            PROMOTIONAL_PRIORITY_OUTPUT,
            PROMOTIONAL_PRIORITY_CACHE_READ,
        )
    } else {
        (
            STANDARD_PRIORITY_INPUT,
            STANDARD_PRIORITY_OUTPUT,
            STANDARD_PRIORITY_CACHE_READ,
        )
    }
}

fn required_extra_rate_matches(model_info: &LiteLLMModelInfo, key: &str, expected: f64) -> bool {
    model_info
        .extra
        .get(key)
        .and_then(serde_json::Value::as_f64)
        == Some(expected)
}

fn optional_extra_rate_matches(model_info: &LiteLLMModelInfo, key: &str, expected: f64) -> bool {
    model_info
        .extra
        .get(key)
        .is_none_or(|value| value.as_f64() == Some(expected))
}

fn model_info_with_rates<'a>(
    model_info: &'a LiteLLMModelInfo,
    input: f64,
    output: f64,
    cache_read: f64,
) -> Cow<'a, LiteLLMModelInfo> {
    let mut effective = model_info.clone();
    let (priority_input, priority_output, priority_cache_read) = priority_rates(input);
    effective.input_cost_per_token = Some(input);
    effective.output_cost_per_token = Some(output);
    for (key, rate) in [
        ("cache_read_input_token_cost", cache_read),
        ("output_cost_per_reasoning_token", output),
        ("input_cost_per_token_batches", input / 2.0),
        ("output_cost_per_token_batches", output / 2.0),
        ("cache_read_input_token_cost_batches", cache_read / 2.0),
        ("input_cost_per_token_flex", input / 2.0),
        ("output_cost_per_token_flex", output / 2.0),
        ("cache_read_input_token_cost_flex", cache_read / 2.0),
        ("input_cost_per_token_priority", priority_input),
        ("output_cost_per_token_priority", priority_output),
        ("cache_read_input_token_cost_priority", priority_cache_read),
    ] {
        if effective.extra.contains_key(key) {
            effective
                .extra
                .insert(key.to_string(), serde_json::json!(rate));
        }
    }
    Cow::Owned(effective)
}

pub(super) const VERTEX_PROVIDER_ALIASES: &[&str] = &[
    "vertex_ai",
    "google",
    "vertex_ai-ai21_models",
    "vertex_ai-anthropic_models",
    "vertex_ai-deepseek_models",
    "vertex_ai-embedding-models",
    "vertex_ai-image-models",
    "vertex_ai-language-models",
    "vertex_ai-llama_models",
    "vertex_ai-minimax_models",
    "vertex_ai-mistral_models",
    "vertex_ai-moonshot_models",
    "vertex_ai-openai_models",
    "vertex_ai-qwen_models",
    "vertex_ai-text-models",
    "vertex_ai-video-models",
    "vertex_ai-zai_models",
];

pub(super) fn uses_google_completion_calculator(
    requested_provider: &str,
    catalog_provider: &str,
) -> bool {
    matches!(requested_provider, "gemini" | "vertex_ai")
        || catalog_provider == "vertex_ai"
        || catalog_provider.starts_with("vertex_ai_")
}

pub(super) fn explicit_pricing_alias(provider: &str, model: &str) -> Option<&'static str> {
    if provider != "vertex_ai" {
        return None;
    }
    let parsed = crate::core::types::model_id::ModelIdRef::parse(model);
    let local = if parsed.provider().is_some_and(|prefix| {
        crate::core::pricing::normalize_pricing_provider(prefix) == "vertex_ai"
    }) {
        parsed.model()
    } else {
        parsed.raw()
    };
    vertex_pricing_alias(&local.to_ascii_lowercase())
}

fn vertex_pricing_alias(model: &str) -> Option<&'static str> {
    match model {
        "gemini-1.5-pro-001" | "gemini-1.5-pro-002" => Some("gemini-1.5-pro"),
        "gemini-1.5-flash-001" | "gemini-1.5-flash-002" => Some("gemini-1.5-flash"),
        "claude-opus-4-6@20260114" => Some("vertex_ai/claude-opus-4-6"),
        "claude-opus-4-5@20251110" => Some("vertex_ai/claude-opus-4-5"),
        "claude-3-5-sonnet@20241022" => Some("vertex_ai/claude-3-5-sonnet"),
        "meta/llama-4-scout-17b-16e-instruct" => {
            Some("vertex_ai/meta/llama-4-scout-17b-16e-instruct-maas")
        }
        "meta/llama-4-maverick-17b-128e-instruct" => {
            Some("vertex_ai/meta/llama-4-maverick-17b-128e-instruct-maas")
        }
        "ai21/jamba-1.5-large" => Some("vertex_ai/jamba-1.5-large"),
        "mistral/mistral-large-2411" => Some("vertex_ai/mistral-large-2411"),
        "mistral/mistral-nemo" => Some("vertex_ai/mistral-nemo@latest"),
        _ => None,
    }
}
