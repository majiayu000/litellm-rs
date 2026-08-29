use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, OnceLock},
};

use chrono::{DateTime, Utc};

use super::{
    GeminiModelFamily, GeminiModelRegistry, GoogleGeminiApiSurface, ModelInfo, ModelPricing,
    ModelSpec,
};
use crate::core::pricing_service::PricingService;

// 2027-01-01T00:00:00Z, the first instant after Google's promotional period.
const STANDARD_PRICING_START_UTC_UNIX_SECONDS: i64 = 1_798_761_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashPricingTier {
    Promotional,
    Standard,
}

fn flash_pricing_tier_at(now: DateTime<Utc>) -> FlashPricingTier {
    if now.timestamp() < STANDARD_PRICING_START_UTC_UNIX_SECONDS {
        FlashPricingTier::Promotional
    } else {
        FlashPricingTier::Standard
    }
}

fn pricing_time_for_tier(tier: FlashPricingTier) -> Option<DateTime<Utc>> {
    let timestamp = match tier {
        FlashPricingTier::Promotional => STANDARD_PRICING_START_UTC_UNIX_SECONDS - 1,
        FlashPricingTier::Standard => STANDARD_PRICING_START_UTC_UNIX_SECONDS,
    };
    DateTime::from_timestamp(timestamp, 0)
}

fn central_flash_pricing_at(model: &str, now: DateTime<Utc>) -> Option<ModelPricing> {
    let service = PricingService::shared_embedded_default().ok()?;
    let (_, pricing) = service.get_model_info_for_provider_at("gemini", model, now)?;
    Some(ModelPricing {
        model: model.to_string(),
        input_cost_per_1k_tokens: per_token_to_per_thousand(pricing.input_cost_per_token?),
        output_cost_per_1k_tokens: per_token_to_per_thousand(pricing.output_cost_per_token?),
        cache_read_input_token_cost: pricing
            .extra
            .get("cache_read_input_token_cost")
            .and_then(serde_json::Value::as_f64)
            .map(per_token_to_per_thousand),
        currency: "USD".to_string(),
        updated_at: now,
        ..ModelPricing::default()
    })
}

fn per_token_to_per_thousand(price: f64) -> f64 {
    ((price * 1_000.0) * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

#[derive(Clone)]
pub(crate) struct GeminiUtcClock {
    now: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>,
}

impl GeminiUtcClock {
    pub(crate) fn system() -> Self {
        Self::new(Utc::now)
    }

    pub(crate) fn new(now: impl Fn() -> DateTime<Utc> + Send + Sync + 'static) -> Self {
        Self { now: Arc::new(now) }
    }

    pub(crate) fn now(&self) -> DateTime<Utc> {
        (self.now)()
    }
}

impl fmt::Debug for GeminiUtcClock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GeminiUtcClock")
    }
}

fn flash_pricing_table(tier: FlashPricingTier) -> HashMap<&'static str, ModelPricing> {
    ["gemini-3.7-flash", "gemini-3.6-flash"]
        .into_iter()
        .filter_map(|model| {
            pricing_time_for_tier(tier)
                .and_then(|pricing_time| central_flash_pricing_at(model, pricing_time))
                .map(|pricing| (model, pricing))
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
        let table = match flash_pricing_tier_at(Utc::now()) {
            FlashPricingTier::Promotional => {
                PROMOTIONAL.get_or_init(|| flash_pricing_table(FlashPricingTier::Promotional))
            }
            FlashPricingTier::Standard => {
                STANDARD.get_or_init(|| flash_pricing_table(FlashPricingTier::Standard))
            }
        };
        table.get(spec.model_info.id.as_str())
    } else {
        Some(&spec.pricing)
    }
}

pub(super) fn pricing_for_spec_at(spec: &ModelSpec, now: DateTime<Utc>) -> Option<ModelPricing> {
    let mut pricing = if matches!(
        spec.family,
        GeminiModelFamily::Gemini37Flash | GeminiModelFamily::Gemini36Flash
    ) {
        central_flash_pricing_at(&spec.model_info.id, now)?
    } else {
        spec.pricing.clone()
    };
    pricing.model.clone_from(&spec.model_info.id);
    Some(pricing)
}

fn model_infos_for_tier(
    registry: &GeminiModelRegistry,
    surface: GoogleGeminiApiSurface,
    tier: FlashPricingTier,
) -> Vec<ModelInfo> {
    let mut models = registry
        .models
        .values()
        .filter(|spec| surface.includes(spec))
        .map(|spec| {
            let mut pricing = spec.pricing.clone();
            if matches!(
                spec.family,
                GeminiModelFamily::Gemini37Flash | GeminiModelFamily::Gemini36Flash
            ) {
                pricing = pricing_time_for_tier(tier)
                    .and_then(|pricing_time| {
                        central_flash_pricing_at(&spec.model_info.id, pricing_time)
                    })
                    .unwrap_or_default();
            }
            surface.overlay_model_info(spec, &pricing)
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.id.cmp(&right.id));
    models
}

impl GeminiModelRegistry {
    /// List model metadata for a concrete Google API surface using the current UTC price tier.
    pub fn list_model_infos_for_surface(&self, surface: GoogleGeminiApiSurface) -> Vec<ModelInfo> {
        model_infos_for_tier(self, surface, flash_pricing_tier_at(Utc::now()))
    }
}

#[derive(Debug)]
pub(crate) struct GeminiModelListings {
    promotional: Vec<ModelInfo>,
    standard: Vec<ModelInfo>,
}

impl GeminiModelListings {
    pub(crate) fn new(registry: &GeminiModelRegistry, surface: GoogleGeminiApiSurface) -> Self {
        Self {
            promotional: model_infos_for_tier(registry, surface, FlashPricingTier::Promotional),
            standard: model_infos_for_tier(registry, surface, FlashPricingTier::Standard),
        }
    }

    pub(crate) fn at(&self, now: DateTime<Utc>) -> &[ModelInfo] {
        match flash_pricing_tier_at(now) {
            FlashPricingTier::Promotional => &self.promotional,
            FlashPricingTier::Standard => &self.standard,
        }
    }
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
