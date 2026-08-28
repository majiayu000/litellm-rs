use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, OnceLock},
};

use chrono::{DateTime, Utc};

use super::{
    GeminiModelFamily, GeminiModelRegistry, GoogleGeminiApiSurface, ModelInfo, ModelPricing,
    ModelSpec, pricing_per_million,
};

// 2027-01-01T00:00:00Z, the first instant after Google's promotional period.
const STANDARD_PRICING_START_UTC_UNIX_SECONDS: i64 = 1_798_761_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashPricingTier {
    Promotional,
    Standard,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Gemini37PriceSchedule;

impl Gemini37PriceSchedule {
    fn tier_at(self, now: DateTime<Utc>) -> FlashPricingTier {
        if now.timestamp() < STANDARD_PRICING_START_UTC_UNIX_SECONDS {
            FlashPricingTier::Promotional
        } else {
            FlashPricingTier::Standard
        }
    }

    fn pricing_at(self, now: DateTime<Utc>) -> ModelPricing {
        self.pricing_for_tier(self.tier_at(now))
    }

    fn pricing_for_tier(self, tier: FlashPricingTier) -> ModelPricing {
        let (input, output, cache) = match tier {
            FlashPricingTier::Promotional => (0.75, 3.75, 0.075),
            FlashPricingTier::Standard => (1.5, 7.5, 0.15),
        };
        let mut pricing = pricing_per_million(input, output, Some(cache), None, None, None);
        pricing.batch_discount = Some(0.5);
        pricing
    }
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

pub(super) fn current_flash_pricing() -> ModelPricing {
    Gemini37PriceSchedule.pricing_at(Utc::now())
}

fn flash_pricing_table(tier: FlashPricingTier) -> HashMap<&'static str, ModelPricing> {
    ["gemini-3.7-flash", "gemini-3.6-flash"]
        .into_iter()
        .map(|model| {
            let mut pricing = Gemini37PriceSchedule.pricing_for_tier(tier);
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
        let table = match Gemini37PriceSchedule.tier_at(Utc::now()) {
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

pub(super) fn pricing_for_spec_at(spec: &ModelSpec, now: DateTime<Utc>) -> ModelPricing {
    let mut pricing = if matches!(
        spec.family,
        GeminiModelFamily::Gemini37Flash | GeminiModelFamily::Gemini36Flash
    ) {
        Gemini37PriceSchedule.pricing_at(now)
    } else {
        spec.pricing.clone()
    };
    pricing.model.clone_from(&spec.model_info.id);
    pricing
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
                pricing = Gemini37PriceSchedule.pricing_for_tier(tier);
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
        model_infos_for_tier(self, surface, Gemini37PriceSchedule.tier_at(Utc::now()))
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
        match Gemini37PriceSchedule.tier_at(now) {
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
