//! Provider-aware pricing authority helpers.

use super::service::PricingService;
use super::types::{
    CostResult, CostType, LiteLLMModelInfo, PricingCostBreakdown, PricingCostEstimate, PricingUsage,
};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
#[cfg(any(feature = "providers-extended", feature = "providers-extra"))]
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
        {
            let mut data = service.pricing_data.write();
            data.models = models;
            data.last_updated = SystemTime::now();
        }
        Ok(service)
    }

    /// Return the process-wide embedded pricing authority for compatibility
    /// adapters that cannot access the runtime service in `AppState`.
    #[cfg(any(feature = "providers-extended", feature = "providers-extra"))]
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
        let data = self.pricing_data.read();
        resolve_model_info_for_provider(&data.models, provider, model)
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
        let (resolved_model, model_info) = self
            .get_model_info_for_provider(provider, model)
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
            let breakdown = calculate_usage_cost_with_pricing(
                &model_info.litellm_provider,
                &resolved_model,
                &model_info,
                &usage,
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
        let (resolved_model, model_info) = self
            .get_model_info_for_provider(provider, model)
            .ok_or_else(|| model_not_found(provider, model))?;

        calculate_usage_cost_with_pricing(provider, &resolved_model, &model_info, usage)
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
        match self.calculate_loaded_usage_cost_for_provider(provider, model, usage) {
            Ok(breakdown) => Ok(breakdown),
            Err(error) => {
                let Some(text_usage) = text_only_usage_for_modal_settlement(usage) else {
                    return Err(error);
                };
                match self.calculate_loaded_usage_cost_for_provider(provider, model, &text_usage) {
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
        let estimated_output_tokens = max_output_tokens.unwrap_or(100);
        let input_only = PricingUsage::new(input_tokens, 0);
        let full_usage = PricingUsage::new(input_tokens, estimated_output_tokens);
        let input = self.dry_run_loaded_usage_cost_for_provider(provider, model, &input_only)?;
        let full = self.dry_run_loaded_usage_cost_for_provider(provider, model, &full_usage)?;

        Ok(PricingCostEstimate {
            min_cost: input.total_cost,
            max_cost: full.total_cost,
            input_cost: input.input_cost,
            estimated_output_cost: full.output_cost,
            currency: full.currency,
        })
    }

    /// Get provider-aware max output tokens from the loaded pricing catalog.
    pub fn max_output_tokens_for_provider(&self, provider: &str, model: &str) -> Option<u32> {
        self.get_model_info_for_provider(provider, model)
            .and_then(|(_, info)| info.max_output_tokens)
    }
}

fn resolve_model_info_for_provider(
    models: &HashMap<String, LiteLLMModelInfo>,
    provider: &str,
    model: &str,
) -> Option<(String, LiteLLMModelInfo)> {
    let normalized_provider = crate::core::pricing::normalize_pricing_provider(provider);
    if normalized_provider == "amazon_nova" {
        return amazon_nova_pricing_model_info(model);
    }
    if normalized_provider == "openai_like"
        && let Some((prefixed_provider, stripped_model)) = provider_prefixed_model(model)
    {
        let prefixed_provider = crate::core::pricing::normalize_pricing_provider(prefixed_provider);
        if prefixed_provider != "openai_like"
            && let Some(resolved) =
                resolve_model_info_for_provider(models, &prefixed_provider, stripped_model)
                    .or_else(|| resolve_model_info_for_provider(models, &prefixed_provider, model))
        {
            return Some(resolved);
        }
    }

    let provider_aliases = pricing_provider_aliases(provider, model);
    if let Some((prefixed_provider, _)) = provider_prefixed_model(model)
        && crate::core::providers::registry::selector_has_matrix_entry(prefixed_provider)
        && !super::google::is_vertex_publisher_prefix(&normalized_provider, prefixed_provider)
        && !provider_name_matches(prefixed_provider, &provider_aliases)
    {
        return None;
    }

    let normalized_model = crate::core::pricing::normalize_model_key(model);
    if normalized_provider == "vertex_ai" {
        for candidate in [
            super::google::vertex_pricing_alias(model),
            super::google::vertex_pricing_alias(normalized_model),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(info) = models
                .get(candidate)
                .filter(|info| provider_name_matches(&info.litellm_provider, &provider_aliases))
            {
                return Some((candidate.to_string(), info.clone()));
            }
        }
    }

    if let Some(info) = models
        .get(model)
        .filter(|info| provider_name_matches(&info.litellm_provider, &provider_aliases))
    {
        return Some((model.to_string(), info.clone()));
    }

    if normalized_model != model
        && let Some(info) = models
            .get(normalized_model)
            .filter(|info| provider_name_matches(&info.litellm_provider, &provider_aliases))
    {
        return Some((normalized_model.to_string(), info.clone()));
    }

    if matches!(normalized_provider.as_str(), "gemini" | "vertex_ai") {
        for candidate in
            super::google::exact_pricing_candidates(&normalized_provider, model, normalized_model)
        {
            if let Some(info) = models
                .get(&candidate)
                .filter(|info| provider_name_matches(&info.litellm_provider, &provider_aliases))
            {
                return Some((candidate, info.clone()));
            }
        }
        return None;
    }

    let requested = normalized_model.to_lowercase();
    models
        .iter()
        .filter(|(_, info)| provider_name_matches(&info.litellm_provider, &provider_aliases))
        .filter(|(candidate, _)| is_shared_model_match(&candidate.to_lowercase(), &requested))
        .max_by_key(|(candidate, _)| candidate.len())
        .map(|(candidate, info)| (candidate.clone(), info.clone()))
        .or_else(|| provider_catalog_model_info(&normalized_provider, model))
}

fn provider_catalog_model_info(
    normalized_provider: &str,
    model: &str,
) -> Option<(String, LiteLLMModelInfo)> {
    match normalized_provider {
        "azure" | "azure_ai" => crate::core::cost::calculator::pricing::get_azure_pricing(model)
            .ok()
            .map(|pricing| {
                let resolved_model = pricing.model.clone();
                (
                    resolved_model,
                    core_pricing_to_litellm_model_info(normalized_provider, pricing),
                )
            }),
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

fn provider_prefixed_model(model: &str) -> Option<(&str, &str)> {
    let (provider, stripped_model) = model.split_once('/')?;
    if provider.is_empty() || stripped_model.is_empty() {
        return None;
    }
    Some((provider, stripped_model))
}

fn is_xiaomi_mimo_model(model: &str) -> bool {
    crate::core::pricing::normalize_model_key(model).starts_with("mimo-")
}

fn provider_name_matches(provider: &str, aliases: &[String]) -> bool {
    let provider = crate::core::pricing::normalize_pricing_provider(provider);
    aliases
        .iter()
        .any(|alias| crate::core::pricing::normalize_pricing_provider(alias) == provider)
}

fn is_shared_model_match(candidate: &str, requested: &str) -> bool {
    fn model_id_matches(candidate: &str, requested: &str) -> bool {
        if candidate == requested {
            return true;
        }

        candidate
            .strip_prefix(requested)
            .and_then(|suffix| suffix.strip_prefix('-'))
            .is_some_and(alias_suffix_matches)
    }

    if candidate == requested {
        return true;
    }

    model_id_matches(candidate, requested)
        || model_id_matches(requested, candidate)
        || candidate
            .rsplit_once('/')
            .map(|(_, model_id)| {
                model_id_matches(model_id, requested) || model_id_matches(requested, model_id)
            })
            .unwrap_or(false)
}

fn alias_suffix_matches(suffix: &str) -> bool {
    if suffix == "latest" {
        return true;
    }

    let digit_prefix_len = suffix.chars().take_while(|ch| ch.is_ascii_digit()).count();
    digit_prefix_len >= 4
        && suffix
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
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

fn calculate_usage_cost_with_pricing(
    requested_provider: &str,
    model: &str,
    model_info: &LiteLLMModelInfo,
    usage: &PricingUsage,
) -> Result<PricingCostBreakdown> {
    let (input_cost_per_token, output_cost_per_token) =
        super::image_pricing::token_unit_prices(model, model_info, usage)?;

    let input_cost_per_token = tiered_cost_per_token(
        model_info,
        input_cost_per_token,
        "input_cost_per_token_above_",
        usage.prompt_tokens,
    );
    let output_cost_per_token = tiered_cost_per_token(
        model_info,
        output_cost_per_token,
        "output_cost_per_token_above_",
        usage.prompt_tokens,
    );
    let cache_read_cost_per_token = tiered_cost_per_token(
        model_info,
        model_info
            .extra
            .get("cache_read_input_token_cost")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(input_cost_per_token),
        "cache_read_input_token_cost_above_",
        usage.prompt_tokens,
    );
    let cache_creation_cost_per_token = tiered_cost_per_token(
        model_info,
        model_info
            .extra
            .get("cache_creation_input_token_cost")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(input_cost_per_token),
        "cache_creation_input_token_cost_above_",
        usage.prompt_tokens,
    );
    let cache_creation_tokens = usage.cache_creation_token_count();
    let cache_read_tokens = usage.cache_read_token_count();
    let non_cached_tokens = usage.non_cached_prompt_tokens();
    let input_cost = non_cached_tokens as f64 * input_cost_per_token;
    let output_cost = usage.completion_tokens as f64 * output_cost_per_token;
    let cache_cost = cache_creation_tokens as f64 * cache_creation_cost_per_token
        + cache_read_tokens as f64 * cache_read_cost_per_token;
    let audio_cost = priced_extra_units(
        model_info,
        model,
        usage.audio_tokens,
        &["input_cost_per_audio_token"],
        "audio pricing",
    )? + priced_extra_units(
        model_info,
        model,
        usage.output_audio_tokens,
        &["output_cost_per_audio_token"],
        "output audio pricing",
    )?;
    let image_cost_per_token =
        super::image_pricing::image_token_unit_price(model_info, usage).unwrap_or(0.0);
    let image_cost = usage.image_tokens.unwrap_or(0) as f64 * image_cost_per_token
        + super::image_pricing::output_image_cost(model, model_info, usage)?;
    let reasoning_cost = usage.reasoning_tokens.unwrap_or(0) as f64
        * extra_f64(model_info, "output_cost_per_reasoning_token");
    let total_cost =
        input_cost + output_cost + cache_cost + audio_cost + image_cost + reasoning_cost;

    Ok(PricingCostBreakdown {
        total_cost,
        input_cost,
        output_cost,
        cache_cost,
        audio_cost,
        image_cost,
        reasoning_cost,
        usage: usage.clone(),
        currency: "USD".to_string(),
        model: model.to_string(),
        provider: requested_provider.to_string(),
        cost_type: CostType::TokenBased,
    })
}

fn model_not_found(provider: &str, model: &str) -> GatewayError {
    GatewayError::not_found(format!(
        "Model not found for provider {}: {}",
        provider, model
    ))
}

fn extra_f64(pricing: &LiteLLMModelInfo, key: &str) -> f64 {
    pricing
        .extra
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0)
}

fn priced_extra_units(
    pricing: &LiteLLMModelInfo,
    model: &str,
    units: Option<u32>,
    keys: &[&str],
    pricing_type: &str,
) -> Result<f64> {
    let units = units.unwrap_or(0);
    if units == 0 {
        return Ok(0.0);
    }

    let (key, unit_price) = keys
        .iter()
        .find_map(|key| {
            pricing
                .extra
                .get(*key)
                .and_then(serde_json::Value::as_f64)
                .map(|price| (*key, price))
        })
        .ok_or_else(|| {
            GatewayError::Config(format!(
                "Missing {pricing_type} for model {model}: {}",
                keys.join(", ")
            ))
        })?;
    if unit_price < 0.0 || unit_price.is_nan() {
        return Err(GatewayError::Config(format!(
            "Invalid {pricing_type} for model {model}: {key} ({unit_price})"
        )));
    }

    Ok(units as f64 * unit_price)
}

fn tiered_cost_per_token(
    pricing: &LiteLLMModelInfo,
    base_cost: f64,
    key_prefix: &str,
    prompt_tokens: u32,
) -> f64 {
    pricing
        .extra
        .iter()
        .filter_map(|(key, value)| {
            if !key.starts_with(key_prefix) {
                return None;
            }
            let threshold = extract_tier_threshold(key)?;
            if prompt_tokens > threshold {
                value.as_f64().map(|cost| (threshold, cost))
            } else {
                None
            }
        })
        .max_by_key(|(threshold, _)| *threshold)
        .map(|(_, cost)| cost)
        .unwrap_or(base_cost)
}

fn extract_tier_threshold(key: &str) -> Option<u32> {
    let threshold = key.split("_above_").nth(1)?.strip_suffix("_tokens")?;
    if let Some(number) = threshold.strip_suffix('k') {
        number.parse::<u32>().ok().map(|value| value * 1000)
    } else {
        threshold.parse::<u32>().ok()
    }
}

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
