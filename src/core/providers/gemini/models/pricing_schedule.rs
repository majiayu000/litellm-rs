use chrono::{DateTime, Utc};

use super::{GeminiModelFamily, GeminiModelRegistry, GoogleGeminiApiSurface, ModelInfo};

const FLASH_STANDARD_PRICING_START_UTC: i64 = 1_798_761_600;

pub(crate) fn flash_uses_standard_pricing_at(now: DateTime<Utc>) -> bool {
    now.timestamp() >= FLASH_STANDARD_PRICING_START_UTC
}

impl GeminiModelRegistry {
    /// List model metadata for a concrete Google API surface at the current price tier.
    pub fn list_model_infos_for_surface(&self, surface: GoogleGeminiApiSurface) -> Vec<ModelInfo> {
        self.list_model_infos_for_surface_at(surface, Utc::now())
    }

    pub(crate) fn list_model_infos_for_surface_at(
        &self,
        surface: GoogleGeminiApiSurface,
        now: DateTime<Utc>,
    ) -> Vec<ModelInfo> {
        let mut models = self
            .models
            .values()
            .filter(|spec| surface.includes(spec))
            .map(|spec| {
                let mut info = surface.overlay_model_info(spec);
                if flash_uses_standard_pricing_at(now)
                    && matches!(
                        spec.family,
                        GeminiModelFamily::Gemini36Flash | GeminiModelFamily::Gemini37Flash
                    )
                {
                    info.input_cost_per_1k_tokens = Some(0.0015);
                    info.output_cost_per_1k_tokens = Some(0.0075);
                    if let Some(standard_cache_storage) = info
                        .metadata
                        .get("google_standard_cache_storage_cost_per_million_token_hour")
                        .cloned()
                    {
                        info.metadata.insert(
                            "google_current_cache_storage_cost_per_million_token_hour".to_string(),
                            standard_cache_storage,
                        );
                    }
                }
                info
            })
            .collect::<Vec<_>>();
        models.sort_by(|left, right| left.id.cmp(&right.id));
        models
    }
}
