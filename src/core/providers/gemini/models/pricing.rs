use std::{collections::HashMap, sync::OnceLock};

use chrono::{NaiveDate, Utc};

use super::{GeminiModelFamily, ModelPricing, ModelSpec, pricing_per_million};

fn standard_pricing_start() -> NaiveDate {
    NaiveDate::from_ymd_opt(2027, 1, 1).expect("Google pricing boundary is a valid date")
}

pub(super) fn flash_pricing_for_date(date: NaiveDate) -> ModelPricing {
    let (input, output, cache) = if date < standard_pricing_start() {
        (0.75, 3.75, 0.075)
    } else {
        (1.5, 7.5, 0.15)
    };
    let mut pricing = pricing_per_million(input, output, Some(cache), None, None, None);
    pricing.batch_discount = Some(0.5);
    pricing
}

pub(super) fn current_flash_pricing() -> ModelPricing {
    flash_pricing_for_date(Utc::now().date_naive())
}

fn flash_pricing_table(date: NaiveDate) -> HashMap<&'static str, ModelPricing> {
    ["gemini-3.7-flash", "gemini-3.6-flash"]
        .into_iter()
        .map(|model| {
            let mut pricing = flash_pricing_for_date(date);
            pricing.model = model.to_string();
            (model, pricing)
        })
        .collect()
}

pub(super) fn current_pricing_for_spec(spec: &ModelSpec) -> Option<&ModelPricing> {
    static PROMOTIONAL: OnceLock<HashMap<&'static str, ModelPricing>> = OnceLock::new();
    static STANDARD: OnceLock<HashMap<&'static str, ModelPricing>> = OnceLock::new();

    if matches!(
        spec.family,
        GeminiModelFamily::Gemini37Flash | GeminiModelFamily::Gemini36Flash
    ) {
        let today = Utc::now().date_naive();
        let table = if today < standard_pricing_start() {
            PROMOTIONAL.get_or_init(|| flash_pricing_table(today))
        } else {
            STANDARD.get_or_init(|| flash_pricing_table(today))
        };
        table.get(spec.model_info.id.as_str())
    } else {
        Some(&spec.pricing)
    }
}

pub(super) fn pricing_for_spec_at(spec: &ModelSpec, date: NaiveDate) -> ModelPricing {
    let mut pricing = if matches!(
        spec.family,
        GeminiModelFamily::Gemini37Flash | GeminiModelFamily::Gemini36Flash
    ) {
        flash_pricing_for_date(date)
    } else {
        spec.pricing.clone()
    };
    pricing.model.clone_from(&spec.model_info.id);
    pricing
}

#[allow(clippy::too_many_arguments)]
pub(super) fn calculate_multimodal_cost(
    pricing: &ModelPricing,
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: Option<u32>,
    images: Option<u32>,
    video_seconds: Option<u32>,
    audio_seconds: Option<u32>,
) -> f64 {
    let mut total_cost = 0.0;
    let mut remaining_prompt_tokens = prompt_tokens;

    if let (Some(cached), Some(cached_price)) = (cached_tokens, pricing.cache_read_input_token_cost)
    {
        total_cost += (cached as f64 / 1000.0) * cached_price;
        remaining_prompt_tokens = remaining_prompt_tokens.saturating_sub(cached);
    }

    total_cost += (remaining_prompt_tokens as f64 / 1000.0) * pricing.input_cost_per_1k_tokens;
    total_cost += (completion_tokens as f64 / 1000.0) * pricing.output_cost_per_1k_tokens;

    let image_price = pricing
        .cost_per_image
        .as_ref()
        .and_then(|costs| costs.get("default"))
        .copied();
    if let (Some(count), Some(price)) = (images, image_price) {
        total_cost += count as f64 * price;
    }
    if let (Some(seconds), Some(price)) = (video_seconds, pricing.video_cost_per_second) {
        total_cost += seconds as f64 * price;
    }
    if let (Some(seconds), Some(price)) = (audio_seconds, pricing.audio_cost_per_second) {
        total_cost += seconds as f64 * price;
    }

    total_cost
}
