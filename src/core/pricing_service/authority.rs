//! Provider-aware pricing authority helpers.

use super::service::PricingService;
use super::types::{
    CostResult, CostType, LiteLLMModelInfo, PricingCostBreakdown, PricingCostEstimate, PricingData,
    PricingSnapshot, PricingUsage,
};
use crate::core::types::model_id::ModelIdRef;
use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::SystemTime;

impl PricingService {
    /// Create a pricing service preloaded with the bundled default pricing data.
    ///
    /// Compatibility adapters use this when they cannot access `AppState.pricing`.
    /// Live request paths should use the runtime service in `AppState`.
    pub fn with_embedded_default() -> Result<Self> {
        let service = Self::new(Some(super::DEFAULT_PRICING_SOURCE.to_string()));
        let models = service.load_from_embedded_default()?;
        let next = super::service::build_pricing_data(models, SystemTime::now());
        {
            let _write_guard = service.pricing_write_lock.lock();
            service.pricing_data.store(std::sync::Arc::new(next));
        }
        Ok(service)
    }

    /// Return the process-wide embedded pricing authority for compatibility
    /// adapters that cannot access the runtime service in `AppState`.
    pub(crate) fn shared_embedded_default() -> Result<&'static Self> {
        static SERVICE: LazyLock<std::result::Result<PricingService, String>> =
            LazyLock::new(|| {
                PricingService::with_embedded_default().map_err(|error| error.to_string())
            });
        SERVICE.as_ref().map_err(|error| {
            GatewayError::Internal(format!(
                "failed to initialize shared embedded pricing authority: {error}"
            ))
        })
    }

    /// Resolve pricing metadata for a provider/model pair using provider aliases
    /// and provider-prefixed model rules.
    pub fn get_model_info_for_provider(
        &self,
        provider: &str,
        model: &str,
    ) -> Option<(String, LiteLLMModelInfo)> {
        self.get_model_info_for_provider_at(provider, model, Utc::now())
    }

    /// Resolve provider-scoped pricing at an explicit UTC instant.
    pub fn get_model_info_for_provider_at(
        &self,
        provider: &str,
        model: &str,
        pricing_time: DateTime<Utc>,
    ) -> Option<(String, LiteLLMModelInfo)> {
        self.snapshot()
            .get_model_info_for_provider_at(provider, model, pricing_time)
    }

    /// Calculate a completion cost from already-loaded pricing data.
    ///
    /// This method does not refresh pricing data, so it is safe for live spend
    /// reservation and settlement paths that must not perform network I/O.
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_loaded_completion_cost_for_provider(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        prompt: Option<&str>,
        completion: Option<&str>,
        total_time_seconds: Option<f64>,
    ) -> Result<CostResult> {
        self.calculate_loaded_completion_cost_for_provider_at(
            provider,
            model,
            input_tokens,
            output_tokens,
            prompt,
            completion,
            total_time_seconds,
            Utc::now(),
        )
    }

    /// Calculate a completion cost from already-loaded pricing data at a
    /// specific UTC pricing instant.
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_loaded_completion_cost_for_provider_at(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        prompt: Option<&str>,
        completion: Option<&str>,
        total_time_seconds: Option<f64>,
        pricing_time: DateTime<Utc>,
    ) -> Result<CostResult> {
        let (resolved_model, model_info) = self
            .get_model_info_for_provider_at(provider, model, pricing_time)
            .ok_or_else(|| model_not_found(provider, model))?;

        if model_info.cost_per_second.is_some() {
            let total_time_seconds =
                super::service::require_total_time_seconds(&resolved_model, total_time_seconds)?;
            return self.calculate_time_based_cost(
                &resolved_model,
                &model_info,
                total_time_seconds,
            );
        }

        let requested_provider = crate::core::pricing::normalize_pricing_provider(provider);
        let catalog_provider =
            crate::core::pricing::normalize_pricing_provider(&model_info.litellm_provider);
        if super::google::uses_google_completion_calculator(&requested_provider, &catalog_provider)
        {
            self.calculate_google_cost(
                &resolved_model,
                &model_info,
                input_tokens,
                output_tokens,
                prompt,
                completion,
            )
        } else {
            let usage = PricingUsage::new(input_tokens, output_tokens);
            let breakdown = super::usage_cost::calculate_usage_cost_with_pricing_at(
                &model_info.litellm_provider,
                &resolved_model,
                &model_info,
                &usage,
                pricing_time,
            )?;
            Ok(CostResult {
                input_cost: breakdown.input_cost,
                output_cost: breakdown.output_cost,
                total_cost: breakdown.total_cost,
                input_tokens,
                output_tokens,
                model: resolved_model,
                provider: model_info.litellm_provider,
                cost_type: CostType::TokenBased,
            })
        }
    }

    /// Calculate detailed token usage cost for spend settlement and legacy cost
    /// adapters from already-loaded pricing data.
    pub fn calculate_loaded_usage_cost_for_provider(
        &self,
        provider: &str,
        model: &str,
        usage: &PricingUsage,
    ) -> Result<PricingCostBreakdown> {
        self.snapshot()
            .calculate_loaded_usage_cost_for_provider(provider, model, usage)
    }

    /// Calculate detailed token usage cost at a specific UTC instant.
    pub fn calculate_loaded_usage_cost_for_provider_at(
        &self,
        provider: &str,
        model: &str,
        usage: &PricingUsage,
        pricing_time: DateTime<Utc>,
    ) -> Result<PricingCostBreakdown> {
        self.snapshot().calculate_loaded_usage_cost_for_provider_at(
            provider,
            model,
            usage,
            pricing_time,
        )
    }

    /// Calculate settlement cost for an already-successful request.
    ///
    /// Request-time dry runs fail closed on missing modality-specific pricing.
    /// Settlement must not convert that error into free spend when text tokens
    /// are still priced, so it falls back to the text/cache/reasoning portion.
    pub fn calculate_loaded_settlement_cost_for_provider(
        &self,
        provider: &str,
        model: &str,
        usage: &PricingUsage,
    ) -> Result<PricingCostBreakdown> {
        self.snapshot()
            .calculate_loaded_settlement_cost_for_provider(provider, model, usage)
    }

    /// Validate and estimate an arbitrary usage shape without mutating spend
    /// or budget state.
    ///
    /// Request-time gates should use this instead of checking only whether a
    /// provider/model row exists, because modality-specific usage can require
    /// additional pricing fields.
    pub fn dry_run_loaded_usage_cost_for_provider(
        &self,
        provider: &str,
        model: &str,
        usage: &PricingUsage,
    ) -> Result<PricingCostBreakdown> {
        self.calculate_loaded_usage_cost_for_provider(provider, model, usage)
    }

    /// Estimate reservation cost from the same authority used for completed
    /// spend settlement.
    pub fn estimate_loaded_completion_cost_for_provider(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u32,
        max_output_tokens: Option<u32>,
    ) -> Result<PricingCostEstimate> {
        self.snapshot()
            .estimate_loaded_completion_cost_for_provider(
                provider,
                model,
                input_tokens,
                max_output_tokens,
            )
    }

    /// Get provider-aware max output tokens from the loaded pricing catalog.
    pub fn max_output_tokens_for_provider(&self, provider: &str, model: &str) -> Option<u32> {
        self.snapshot()
            .get_model_info_for_provider(provider, model)
            .and_then(|(_, info)| info.max_output_tokens)
    }
}

impl PricingSnapshot {
    pub(crate) fn get_model_info(&self, model: &str) -> Option<LiteLLMModelInfo> {
        self.data.models.get(model).cloned()
    }

    pub(crate) fn get_model_info_for_provider(
        &self,
        provider: &str,
        model: &str,
    ) -> Option<(String, LiteLLMModelInfo)> {
        self.get_model_info_for_provider_at(provider, model, Utc::now())
    }

    pub(crate) fn get_model_info_for_provider_at(
        &self,
        provider: &str,
        model: &str,
        pricing_time: DateTime<Utc>,
    ) -> Option<(String, LiteLLMModelInfo)> {
        let (resolved_model, model_info) =
            resolve_model_info_for_provider(&self.data, provider, model)?;
        let effective = super::google::effective_model_info_at(
            provider,
            &resolved_model,
            &model_info,
            pricing_time,
        )
        .into_owned();
        Some((resolved_model, effective))
    }

    pub(crate) fn calculate_loaded_usage_cost_for_provider(
        &self,
        provider: &str,
        model: &str,
        usage: &PricingUsage,
    ) -> Result<PricingCostBreakdown> {
        self.calculate_loaded_usage_cost_for_provider_at(provider, model, usage, Utc::now())
    }

    pub(crate) fn calculate_loaded_usage_cost_for_provider_at(
        &self,
        provider: &str,
        model: &str,
        usage: &PricingUsage,
        pricing_time: DateTime<Utc>,
    ) -> Result<PricingCostBreakdown> {
        let (resolved_model, model_info) = self
            .get_model_info_for_provider_at(provider, model, pricing_time)
            .ok_or_else(|| model_not_found(provider, model))?;
        super::usage_cost::calculate_usage_cost_with_pricing_at(
            provider,
            &resolved_model,
            &model_info,
            usage,
            pricing_time,
        )
    }

    pub(crate) fn calculate_loaded_settlement_cost_for_provider(
        &self,
        provider: &str,
        model: &str,
        usage: &PricingUsage,
    ) -> Result<PricingCostBreakdown> {
        let pricing_time = Utc::now();
        match self.calculate_loaded_usage_cost_for_provider_at(provider, model, usage, pricing_time)
        {
            Ok(breakdown) => Ok(breakdown),
            Err(error) => {
                if usage.billing_mode == super::types::PricingBillingMode::Batch {
                    return Err(error);
                }
                let Some(text_usage) = text_only_usage_for_modal_settlement(usage) else {
                    return Err(error);
                };
                match self.calculate_loaded_usage_cost_for_provider_at(
                    provider,
                    model,
                    &text_usage,
                    pricing_time,
                ) {
                    Ok(mut breakdown) => {
                        tracing::error!(
                            "modal cost calculation failed for '{provider}'/'{model}': {error}; \
                             settling text/token cost only"
                        );
                        breakdown.usage = usage.clone();
                        Ok(breakdown)
                    }
                    Err(_) => Err(error),
                }
            }
        }
    }

    pub(crate) fn estimate_loaded_completion_cost_for_provider(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u32,
        max_output_tokens: Option<u32>,
    ) -> Result<PricingCostEstimate> {
        let (resolved_model, model_info) = self
            .get_model_info_for_provider(provider, model)
            .ok_or_else(|| model_not_found(provider, model))?;
        let model_info =
            super::google::maximum_scheduled_model_info(provider, &resolved_model, &model_info);
        let estimated_output_tokens = max_output_tokens.unwrap_or(100);
        let input_only = PricingUsage::new(input_tokens, 0);
        let full_usage = PricingUsage::new(input_tokens, estimated_output_tokens);
        let input = super::usage_cost::calculate_usage_cost_with_maximum_rates(
            provider,
            &resolved_model,
            &model_info,
            &input_only,
        )?;
        let full = super::usage_cost::calculate_usage_cost_with_maximum_rates(
            provider,
            &resolved_model,
            &model_info,
            &full_usage,
        )?;
        Ok(PricingCostEstimate {
            min_cost: input.total_cost,
            max_cost: full.total_cost,
            input_cost: input.input_cost,
            estimated_output_cost: full.output_cost,
            currency: full.currency,
        })
    }
}

fn resolve_model_info_for_provider(
    data: &PricingData,
    provider: &str,
    model: &str,
) -> Option<(String, LiteLLMModelInfo)> {
    let normalized_provider = crate::core::pricing::normalize_pricing_provider(provider);
    if normalized_provider == "amazon_nova" {
        return amazon_nova_pricing_model_info(model);
    }

    let provider_aliases = pricing_provider_aliases(provider, model);
    let candidates = exact_pricing_candidates(&normalized_provider, model, &provider_aliases);
    if let Some(resolved) = exact_provider_model(data, &provider_aliases, &candidates) {
        return Some(resolved);
    }
    if normalized_provider == "openai_like" {
        let parsed = ModelIdRef::parse(model);
        if let Some(prefix) = parsed.provider()
            && crate::core::providers::registry::selector_has_matrix_entry(prefix)
        {
            let prefixed_provider = canonical_pricing_selector(prefix);
            if prefixed_provider != "openai_like" {
                return resolve_model_info_for_provider(data, &prefixed_provider, parsed.model());
            }
        }
    }
    if let Some(alias) = super::google::explicit_pricing_alias(&normalized_provider, model)
        && let Some(resolved) = exact_provider_model(data, &provider_aliases, &[alias.to_string()])
    {
        return Some(resolved);
    }
    if matches!(normalized_provider.as_str(), "gemini" | "vertex_ai") {
        return None;
    }
    provider_catalog_model_info(&normalized_provider, model)
}

fn exact_pricing_candidates(
    provider: &str,
    model: &str,
    provider_aliases: &[String],
) -> Vec<String> {
    let parsed = ModelIdRef::parse(model);
    let mut candidates = Vec::with_capacity(5);
    push_unique(&mut candidates, parsed.raw());
    push_unique(&mut candidates, &format!("{provider}/{}", parsed.raw()));
    if parsed.provider().is_some_and(|prefix| {
        let prefix = crate::core::pricing::normalize_pricing_provider(prefix);
        prefix == provider || provider_aliases.iter().any(|alias| alias == &prefix)
    }) {
        push_unique(&mut candidates, parsed.model());
        push_unique(&mut candidates, &format!("{provider}/{}", parsed.model()));
    }
    candidates
}

fn canonical_pricing_selector(selector: &str) -> String {
    let canonical = crate::core::providers::registry::canonical_selector(selector);
    if canonical == "openai_compatible" {
        "openai_like".to_string()
    } else {
        crate::core::pricing::normalize_pricing_provider(&canonical)
    }
}

fn push_unique(candidates: &mut Vec<String>, candidate: &str) {
    if !candidate.is_empty() && !candidates.iter().any(|existing| existing == candidate) {
        candidates.push(candidate.to_string());
    }
}

fn exact_provider_model(
    data: &PricingData,
    providers: &[String],
    candidates: &[String],
) -> Option<(String, LiteLLMModelInfo)> {
    for candidate in candidates {
        if let Some(info) = data
            .models
            .get(candidate)
            .filter(|info| provider_name_matches(&info.litellm_provider, providers))
        {
            return Some((candidate.clone(), info.clone()));
        }
    }
    for provider in providers {
        let Some(index) = data.exact_by_provider.get(provider) else {
            continue;
        };
        for candidate in candidates {
            if let Some(canonical) = index
                .get(&candidate.to_ascii_lowercase())
                .and_then(|collisions| collisions.first())
                && let Some(info) = data.models.get(canonical)
            {
                return Some((canonical.clone(), info.clone()));
            }
        }
    }
    None
}

fn provider_catalog_model_info(
    normalized_provider: &str,
    model: &str,
) -> Option<(String, LiteLLMModelInfo)> {
    match normalized_provider {
        "bedrock" => crate::core::providers::bedrock::CostCalculator::get_core_model_pricing(model)
            .map(|pricing| {
                let resolved_model = pricing.model.clone();
                (
                    resolved_model,
                    core_pricing_to_litellm_model_info("bedrock", pricing),
                )
            }),
        "xai" => xai_pricing_model_info(model),
        _ => None,
    }
}

fn amazon_nova_pricing_model_info(model: &str) -> Option<(String, LiteLLMModelInfo)> {
    let info = crate::core::providers::registry::catalog::amazon_nova_catalog_model_info(model)?;
    let resolved_model = info.id.clone();
    Some((
        resolved_model,
        model_info_to_litellm_model_info("amazon_nova", info),
    ))
}

fn xai_pricing_model_info(model: &str) -> Option<(String, LiteLLMModelInfo)> {
    let models = crate::core::providers::openai_like::models::get_openai_like_registry();
    if !crate::core::providers::openai_like::models::is_xai_priced_model(model) {
        return None;
    }

    let info = models.get_model_info(model);
    if info.input_cost_per_1k_tokens.is_none() || info.output_cost_per_1k_tokens.is_none() {
        return None;
    }
    let resolved_model = info.id.clone();

    Some((
        resolved_model,
        model_info_to_litellm_model_info("xai", info),
    ))
}

fn model_info_to_litellm_model_info(
    provider: &str,
    info: crate::core::types::model::ModelInfo,
) -> LiteLLMModelInfo {
    LiteLLMModelInfo {
        max_tokens: Some(info.max_context_length),
        max_input_tokens: Some(info.max_context_length),
        max_output_tokens: info.max_output_length,
        input_cost_per_token: info
            .input_cost_per_1k_tokens
            .map(|cost_per_1k| cost_per_1k / 1000.0),
        output_cost_per_token: info
            .output_cost_per_1k_tokens
            .map(|cost_per_1k| cost_per_1k / 1000.0),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: provider.to_string(),
        mode: "chat".to_string(),
        supports_function_calling: Some(info.supports_tools),
        supports_vision: Some(info.supports_multimodal),
        supports_streaming: Some(info.supports_streaming),
        supports_parallel_function_calling: Some(info.supports_tools),
        supports_system_message: Some(true),
        extra: info.metadata,
    }
}

fn core_pricing_to_litellm_model_info(
    provider: &str,
    pricing: crate::core::cost::types::ModelPricing,
) -> LiteLLMModelInfo {
    let mut extra = HashMap::new();
    insert_optional_token_cost(
        &mut extra,
        "cache_read_input_token_cost",
        pricing.cache_read_input_token_cost,
    );
    insert_optional_token_cost(
        &mut extra,
        "cache_creation_input_token_cost",
        pricing.cache_creation_input_token_cost,
    );
    insert_optional_cost(
        &mut extra,
        "input_cost_per_audio_token",
        pricing.input_cost_per_audio_token,
    );
    insert_optional_cost(
        &mut extra,
        "output_cost_per_audio_token",
        pricing.output_cost_per_audio_token,
    );
    insert_optional_cost(
        &mut extra,
        "image_cost_per_token",
        pricing.image_cost_per_token,
    );
    insert_optional_cost(
        &mut extra,
        "output_cost_per_reasoning_token",
        pricing.reasoning_cost_per_token,
    );
    insert_tiered_pricing(&mut extra, pricing.tiered_pricing.as_ref());

    LiteLLMModelInfo {
        max_tokens: None,
        max_input_tokens: None,
        max_output_tokens: None,
        input_cost_per_token: Some(pricing.input_cost_per_1k_tokens / 1000.0),
        output_cost_per_token: Some(pricing.output_cost_per_1k_tokens / 1000.0),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: pricing.cost_per_second,
        litellm_provider: provider.to_string(),
        mode: "chat".to_string(),
        supports_function_calling: None,
        supports_vision: None,
        supports_streaming: None,
        supports_parallel_function_calling: None,
        supports_system_message: None,
        extra,
    }
}

fn insert_tiered_pricing(
    extra: &mut HashMap<String, serde_json::Value>,
    tiered_pricing: Option<&HashMap<String, f64>>,
) {
    let Some(tiered_pricing) = tiered_pricing else {
        return;
    };

    for (key, cost_per_1k_tokens) in tiered_pricing {
        insert_optional_cost(extra, key, Some(cost_per_1k_tokens / 1000.0));
    }
}

fn insert_optional_token_cost(
    extra: &mut HashMap<String, serde_json::Value>,
    key: &str,
    cost_per_1k_tokens: Option<f64>,
) {
    if let Some(cost_per_1k_tokens) = cost_per_1k_tokens {
        insert_optional_cost(extra, key, Some(cost_per_1k_tokens / 1000.0));
    }
}

fn insert_optional_cost(
    extra: &mut HashMap<String, serde_json::Value>,
    key: &str,
    value: Option<f64>,
) {
    if let Some(value) = value {
        extra.insert(key.to_string(), serde_json::json!(value));
    }
}

fn pricing_provider_aliases(provider: &str, model: &str) -> Vec<String> {
    let normalized = crate::core::pricing::normalize_pricing_provider(provider);
    let aliases = match normalized.as_str() {
        "anthropic" if is_xiaomi_mimo_model(model) => vec!["xiaomi_mimo", "xiaomi", "mimo"],
        "gemini" => vec!["gemini"],
        "vertex_ai" => super::google::VERTEX_PROVIDER_ALIASES.to_vec(),
        "xiaomi_mimo" => vec!["xiaomi_mimo", "xiaomi", "mimo"],
        "zhipuai" => vec!["zhipuai", "glm"],
        "amazon_nova" => vec!["amazon_nova", "bedrock"],
        _ => return vec![normalized],
    };
    aliases
        .into_iter()
        .map(crate::core::pricing::normalize_pricing_provider)
        .fold(Vec::new(), |mut unique, alias| {
            if !unique.contains(&alias) {
                unique.push(alias);
            }
            unique
        })
}

fn is_xiaomi_mimo_model(model: &str) -> bool {
    let parsed = ModelIdRef::parse(model);
    let local = if parsed.provider().is_some_and(|prefix| {
        crate::core::pricing::normalize_pricing_provider(prefix) == "anthropic"
    }) {
        parsed.model()
    } else {
        parsed.raw()
    };
    local.to_ascii_lowercase().starts_with("mimo-")
}

fn provider_name_matches(provider: &str, aliases: &[String]) -> bool {
    let provider = crate::core::pricing::normalize_pricing_provider(provider);
    aliases
        .iter()
        .any(|alias| crate::core::pricing::normalize_pricing_provider(alias) == provider)
}

fn text_only_usage_for_modal_settlement(usage: &PricingUsage) -> Option<PricingUsage> {
    let has_modal_usage = usage.audio_token_count() > 0
        || usage.image_tokens.unwrap_or(0) > 0
        || usage.output_image_count.unwrap_or(0) > 0;
    if !has_modal_usage {
        return None;
    }

    let mut text_usage = usage.clone();
    text_usage.audio_tokens = None;
    text_usage.output_audio_tokens = None;
    text_usage.image_tokens = None;
    text_usage.output_image_count = None;
    text_usage.output_image_pricing_keys.clear();
    Some(text_usage)
}

fn model_not_found(provider: &str, model: &str) -> GatewayError {
    GatewayError::not_found(format!(
        "Model not found for provider {}: {}",
        provider, model
    ))
}

#[cfg(test)]
use super::usage_cost::extract_tier_threshold;

#[cfg(test)]
mod amazon_nova_catalog_authority_tests {
    use super::*;
    #[test]
    fn amazon_nova_catalog_authority_is_feature_independent() {
        let service = PricingService::with_embedded_default().unwrap();
        for model in ["amazon.nova-pro-v1:0", "nova-pro"] {
            let (resolved, info) = service
                .get_model_info_for_provider("amazon_nova", model)
                .unwrap();
            assert_eq!(resolved, "amazon.nova-pro-v1:0");
            assert_eq!(info.max_output_tokens, Some(5_000));
            let expected = "High-capability multimodal model for complex tasks";
            assert_eq!(info.extra["description"], expected);
            assert_eq!(info.extra["supports_reasoning"], true);
        }
        assert!(
            service
                .get_model_info_for_provider("amazon_nova", "unknown-nova")
                .is_none()
        );
    }
}
#[cfg(test)]
#[path = "authority_tests.rs"]
mod tests;
