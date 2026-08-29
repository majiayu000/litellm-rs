//! Exact catalog resolution rules for Google pricing surfaces.

use std::borrow::Cow;

use chrono::{DateTime, Utc};

use super::types::LiteLLMModelInfo;

const GEMINI_FLASH_STANDARD_PRICING_START_UTC: i64 = 1_798_761_600;
const GEMINI_FLASH_STANDARD_INPUT_COST_PER_TOKEN: f64 = 1.5e-6;
const GEMINI_FLASH_STANDARD_OUTPUT_COST_PER_TOKEN: f64 = 7.5e-6;
const GEMINI_FLASH_STANDARD_CACHE_READ_COST_PER_TOKEN: f64 = 1.5e-7;
const GEMINI_FLASH_PROMOTIONAL_INPUT_COST_PER_TOKEN: f64 = 7.5e-7;
const GEMINI_FLASH_PROMOTIONAL_OUTPUT_COST_PER_TOKEN: f64 = 3.75e-6;
const GEMINI_FLASH_PROMOTIONAL_CACHE_READ_COST_PER_TOKEN: f64 = 7.5e-8;

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

pub(super) fn effective_model_info_at<'a>(
    requested_provider: &str,
    resolved_model: &str,
    model_info: &'a LiteLLMModelInfo,
    pricing_time: DateTime<Utc>,
    catalog_owned: bool,
) -> Cow<'a, LiteLLMModelInfo> {
    let requested_provider = crate::core::pricing::normalize_pricing_provider(requested_provider);
    let catalog_provider =
        crate::core::pricing::normalize_pricing_provider(&model_info.litellm_provider);
    if !uses_google_completion_calculator(&requested_provider, &catalog_provider) {
        return Cow::Borrowed(model_info);
    }

    let parsed = crate::core::types::model_id::ModelIdRef::parse(resolved_model);
    let local_model = parsed.model();
    if !matches!(local_model, "gemini-3.6-flash" | "gemini-3.7-flash") {
        return Cow::Borrowed(model_info);
    }

    let is_promotional = is_catalog_row_at_rate(
        model_info,
        catalog_owned,
        GEMINI_FLASH_PROMOTIONAL_INPUT_COST_PER_TOKEN,
        GEMINI_FLASH_PROMOTIONAL_OUTPUT_COST_PER_TOKEN,
        GEMINI_FLASH_PROMOTIONAL_CACHE_READ_COST_PER_TOKEN,
    );
    let is_standard = is_catalog_row_at_rate(
        model_info,
        catalog_owned,
        GEMINI_FLASH_STANDARD_INPUT_COST_PER_TOKEN,
        GEMINI_FLASH_STANDARD_OUTPUT_COST_PER_TOKEN,
        GEMINI_FLASH_STANDARD_CACHE_READ_COST_PER_TOKEN,
    );
    let standard_time = pricing_time.timestamp() >= GEMINI_FLASH_STANDARD_PRICING_START_UTC;

    match (standard_time, is_promotional, is_standard) {
        (true, true, false) => model_info_with_rate(
            model_info,
            GEMINI_FLASH_STANDARD_INPUT_COST_PER_TOKEN,
            GEMINI_FLASH_STANDARD_OUTPUT_COST_PER_TOKEN,
            GEMINI_FLASH_STANDARD_CACHE_READ_COST_PER_TOKEN,
        ),
        (false, false, true) => model_info_with_rate(
            model_info,
            GEMINI_FLASH_PROMOTIONAL_INPUT_COST_PER_TOKEN,
            GEMINI_FLASH_PROMOTIONAL_OUTPUT_COST_PER_TOKEN,
            GEMINI_FLASH_PROMOTIONAL_CACHE_READ_COST_PER_TOKEN,
        ),
        _ => Cow::Borrowed(model_info),
    }
}

fn is_catalog_row_at_rate(
    model_info: &LiteLLMModelInfo,
    catalog_owned: bool,
    input_cost: f64,
    output_cost: f64,
    cache_read_cost: f64,
) -> bool {
    catalog_owned
        && model_info.input_cost_per_token == Some(input_cost)
        && model_info.output_cost_per_token == Some(output_cost)
        && model_info
            .extra
            .get("cache_read_input_token_cost")
            .and_then(serde_json::Value::as_f64)
            == Some(cache_read_cost)
        && model_info
            .extra
            .get("output_cost_per_reasoning_token")
            .and_then(serde_json::Value::as_f64)
            .is_none_or(|price| price == output_cost)
        && optional_extra_rate_matches(model_info, "input_cost_per_token_batches", input_cost / 2.0)
        && optional_extra_rate_matches(model_info, "input_cost_per_token_flex", input_cost / 2.0)
        && optional_extra_rate_matches(
            model_info,
            "output_cost_per_token_batches",
            output_cost / 2.0,
        )
        && optional_extra_rate_matches(model_info, "output_cost_per_token_flex", output_cost / 2.0)
        && optional_extra_rate_matches(
            model_info,
            "cache_read_input_token_cost_flex",
            cache_read_cost / 2.0,
        )
}

fn optional_extra_rate_matches(model_info: &LiteLLMModelInfo, key: &str, expected: f64) -> bool {
    model_info
        .extra
        .get(key)
        .is_none_or(|value| value.as_f64() == Some(expected))
}

fn model_info_with_rate<'a>(
    model_info: &'a LiteLLMModelInfo,
    input_cost: f64,
    output_cost: f64,
    cache_read_cost: f64,
) -> Cow<'a, LiteLLMModelInfo> {
    let mut effective = model_info.clone();
    effective.input_cost_per_token = Some(input_cost);
    effective.output_cost_per_token = Some(output_cost);
    effective.extra.insert(
        "cache_read_input_token_cost".to_string(),
        serde_json::json!(cache_read_cost),
    );
    effective.extra.insert(
        "output_cost_per_reasoning_token".to_string(),
        serde_json::json!(output_cost),
    );
    for (key, rate) in [
        ("input_cost_per_token_batches", input_cost / 2.0),
        ("input_cost_per_token_flex", input_cost / 2.0),
        ("output_cost_per_token_batches", output_cost / 2.0),
        ("output_cost_per_token_flex", output_cost / 2.0),
        ("cache_read_input_token_cost_flex", cache_read_cost / 2.0),
    ] {
        if effective.extra.contains_key(key) {
            effective
                .extra
                .insert(key.to_string(), serde_json::json!(rate));
        }
    }
    Cow::Owned(effective)
}

pub(super) fn maximum_scheduled_model_info<'a>(
    requested_provider: &str,
    resolved_model: &str,
    model_info: &'a LiteLLMModelInfo,
    catalog_owned: bool,
) -> Cow<'a, LiteLLMModelInfo> {
    let standard_start = DateTime::from_timestamp(GEMINI_FLASH_STANDARD_PRICING_START_UTC, 0)
        .expect("Gemini pricing schedule timestamp is valid");
    effective_model_info_at(
        requested_provider,
        resolved_model,
        model_info,
        standard_start,
        catalog_owned,
    )
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
