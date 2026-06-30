//! Provider-aware pricing authority helpers.

use super::service::PricingService;
use super::types::{
    CostResult, CostType, LiteLLMModelInfo, PricingCostBreakdown, PricingCostEstimate, PricingUsage,
};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
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

        match crate::core::pricing::normalize_pricing_provider(&model_info.litellm_provider)
            .as_str()
        {
            "vertex_ai" => self.calculate_google_cost(
                &resolved_model,
                &model_info,
                input_tokens,
                output_tokens,
                prompt,
                completion,
            ),
            _ => {
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
        let input = self.calculate_loaded_usage_cost_for_provider(provider, model, &input_only)?;
        let full = self.calculate_loaded_usage_cost_for_provider(provider, model, &full_usage)?;

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

    if let Some(info) = models
        .get(model)
        .filter(|info| provider_name_matches(&info.litellm_provider, &provider_aliases))
    {
        return Some((model.to_string(), info.clone()));
    }

    let normalized_model = crate::core::pricing::normalize_model_key(model);
    if normalized_model != model
        && let Some(info) = models
            .get(normalized_model)
            .filter(|info| provider_name_matches(&info.litellm_provider, &provider_aliases))
    {
        return Some((normalized_model.to_string(), info.clone()));
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
        "amazon_nova" => amazon_nova_pricing_model_info(model),
        "xai" => xai_pricing_model_info(model),
        _ => None,
    }
}

#[cfg(feature = "providers-extended")]
fn amazon_nova_pricing_model_info(model: &str) -> Option<(String, LiteLLMModelInfo)> {
    let registry = crate::core::providers::amazon_nova::AmazonNovaModelRegistry::new();
    let model = registry.get(model)?;
    let resolved_model = model.id.clone();

    Some((
        resolved_model,
        LiteLLMModelInfo {
            max_tokens: Some(model.context_length),
            max_input_tokens: Some(model.context_length),
            max_output_tokens: Some(model.max_output_tokens),
            input_cost_per_token: Some(model.input_cost_per_1k / 1000.0),
            output_cost_per_token: Some(model.output_cost_per_1k / 1000.0),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: "amazon_nova".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: Some(model.supports_tools),
            supports_vision: Some(model.supports_vision),
            supports_streaming: Some(model.supports_streaming),
            supports_parallel_function_calling: Some(model.supports_tools),
            supports_system_message: Some(true),
            extra: HashMap::new(),
        },
    ))
}

#[cfg(not(feature = "providers-extended"))]
fn amazon_nova_pricing_model_info(_model: &str) -> Option<(String, LiteLLMModelInfo)> {
    None
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
        "gemini" => vec!["gemini", "vertex_ai"],
        "vertex_ai" => vec!["vertex_ai", "google"],
        "xiaomi_mimo" => vec!["xiaomi_mimo", "xiaomi", "mimo"],
        "zhipuai" => vec!["zhipuai", "glm", "zai"],
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

fn calculate_usage_cost_with_pricing(
    requested_provider: &str,
    model: &str,
    model_info: &LiteLLMModelInfo,
    usage: &PricingUsage,
) -> Result<PricingCostBreakdown> {
    let input_cost_per_token = super::service::require_pricing_field(
        model_info.input_cost_per_token,
        model,
        "token pricing",
        "input_cost_per_token",
    )?;
    let output_cost_per_token = super::service::require_pricing_field(
        model_info.output_cost_per_token,
        model,
        "token pricing",
        "output_cost_per_token",
    )?;

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
        extra_f64(model_info, "cache_read_input_token_cost"),
        "cache_read_input_token_cost_above_",
        usage.prompt_tokens,
    );

    let non_cached_tokens = usage
        .cached_tokens
        .map(|cached| usage.prompt_tokens.saturating_sub(cached))
        .unwrap_or(usage.prompt_tokens);
    let input_cost = non_cached_tokens as f64 * input_cost_per_token;
    let output_cost = usage.completion_tokens as f64 * output_cost_per_token;
    let cache_cost = usage.cached_tokens.unwrap_or(0) as f64 * cache_read_cost_per_token;
    let audio_cost = usage.audio_tokens.unwrap_or(0) as f64
        * extra_f64(model_info, "input_cost_per_audio_token");
    let image_cost =
        usage.image_tokens.unwrap_or(0) as f64 * extra_f64(model_info, "image_cost_per_token");
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
mod tests {
    use super::*;

    fn test_model_info(provider: &str) -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: Some(4096),
            max_output_tokens: Some(4096),
            input_cost_per_token: Some(0.00001),
            output_cost_per_token: Some(0.00003),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: provider.to_string(),
            mode: "chat".to_string(),
            supports_function_calling: Some(true),
            supports_vision: Some(false),
            supports_streaming: Some(true),
            supports_parallel_function_calling: Some(true),
            supports_system_message: Some(true),
            extra: HashMap::new(),
        }
    }

    #[test]
    fn provider_aware_authority_uses_loaded_custom_model() {
        let service = PricingService::new(None);
        service.add_custom_model(
            "runtime-only-priced-model".to_string(),
            test_model_info("runtime_provider"),
        );

        let cost = match service.calculate_loaded_usage_cost_for_provider(
            "runtime_provider",
            "runtime-only-priced-model",
            &PricingUsage::new(1000, 500),
        ) {
            Ok(cost) => cost,
            Err(error) => panic!("runtime-loaded pricing should calculate cost: {error}"),
        };

        assert_eq!(cost.model, "runtime-only-priced-model");
        assert_eq!(cost.provider, "runtime_provider");
        assert_eq!(cost.input_cost, 0.01);
        assert!((cost.output_cost - 0.015).abs() < f64::EPSILON);
        assert!((cost.total_cost - 0.025).abs() < f64::EPSILON);
    }

    #[test]
    fn provider_aware_authority_resolves_anthropic_mimo_alias() {
        let service = match PricingService::with_embedded_default() {
            Ok(service) => service,
            Err(error) => panic!("embedded pricing service should initialize for tests: {error}"),
        };

        let cost = match service.calculate_loaded_usage_cost_for_provider(
            "anthropic",
            "mimo-v2.5-pro",
            &PricingUsage::new(1000, 500),
        ) {
            Ok(cost) => cost,
            Err(error) => {
                panic!("Anthropic-compatible MiMo should resolve through Xiaomi pricing: {error}")
            }
        };

        assert_eq!(cost.model, "mimo-v2.5-pro");
        assert_eq!(cost.provider, "anthropic");
        assert!(cost.total_cost > 0.0);
    }

    #[test]
    fn provider_aware_authority_resolves_loaded_openai_like_model_without_prefix() {
        let service = PricingService::new(None);
        service.add_custom_model(
            "runtime-openai-like-model".to_string(),
            test_model_info("openai_like"),
        );

        let cost = match service.calculate_loaded_usage_cost_for_provider(
            "openai_like",
            "runtime-openai-like-model",
            &PricingUsage::new(1000, 500),
        ) {
            Ok(cost) => cost,
            Err(error) => panic!("loaded OpenAI-like pricing should calculate cost: {error}"),
        };

        assert_eq!(cost.model, "runtime-openai-like-model");
        assert_eq!(cost.provider, "openai_like");
        assert!((cost.total_cost - 0.025).abs() < f64::EPSILON);
    }

    #[test]
    fn provider_aware_authority_resolves_xai_openai_like_prefix() {
        let service = match PricingService::with_embedded_default() {
            Ok(service) => service,
            Err(error) => panic!("embedded pricing service should initialize for tests: {error}"),
        };

        let cost = match service.calculate_loaded_usage_cost_for_provider(
            "openai_like",
            "xai/grok-4.3",
            &PricingUsage::new(1000, 500),
        ) {
            Ok(cost) => cost,
            Err(error) => panic!("xAI OpenAI-like prefixed model should resolve: {error}"),
        };

        assert_eq!(cost.model, "xai/grok-4.3-latest");
        assert_eq!(cost.provider, "openai_like");
        assert!((cost.total_cost - 0.0025).abs() < f64::EPSILON);
    }

    #[cfg(feature = "providers-extended")]
    #[test]
    fn provider_aware_authority_resolves_amazon_nova_short_alias() {
        let service = match PricingService::with_embedded_default() {
            Ok(service) => service,
            Err(error) => panic!("embedded pricing service should initialize for tests: {error}"),
        };

        let cost = match service.calculate_loaded_usage_cost_for_provider(
            "amazon_nova",
            "nova-2-lite",
            &PricingUsage::new(1000, 500),
        ) {
            Ok(cost) => cost,
            Err(error) => panic!("Amazon Nova short alias should resolve: {error}"),
        };

        assert_eq!(cost.model, "amazon.nova-2-lite-v1:0");
        assert_eq!(cost.provider, "amazon_nova");
        assert!((cost.total_cost - 0.00155).abs() < f64::EPSILON);
    }

    #[test]
    fn provider_aware_authority_preserves_core_pricing_tiers() {
        let service = match PricingService::with_embedded_default() {
            Ok(service) => service,
            Err(error) => panic!("embedded pricing service should initialize for tests: {error}"),
        };

        let cost = match service.calculate_loaded_usage_cost_for_provider(
            "azure",
            "gpt-5.5",
            &PricingUsage::new(300_000, 1_000),
        ) {
            Ok(cost) => cost,
            Err(error) => panic!("Azure tiered fallback pricing should resolve: {error}"),
        };

        assert_eq!(cost.model, "azure/gpt-5.5-2026-04-23");
        assert_eq!(cost.provider, "azure");
        assert!(
            (cost.input_cost - 3.0).abs() < 1e-12,
            "unexpected input cost: {}",
            cost.input_cost
        );
        assert!(
            (cost.output_cost - 0.045).abs() < 1e-12,
            "unexpected output cost: {}",
            cost.output_cost
        );
        assert!(
            (cost.total_cost - 3.045).abs() < 1e-12,
            "unexpected total cost: {}",
            cost.total_cost
        );
    }

    #[test]
    fn tier_threshold_ignores_named_price_variants() {
        assert_eq!(
            extract_tier_threshold("input_cost_per_token_above_272k_tokens"),
            Some(272_000)
        );
        assert_eq!(
            extract_tier_threshold("input_cost_per_token_above_272k_tokens_priority"),
            None
        );
        assert_eq!(
            extract_tier_threshold("input_cost_per_token_above_272k_tokens_flex"),
            None
        );
    }

    #[test]
    fn provider_aware_authority_rejects_missing_token_pricing() {
        let service = PricingService::new(None);
        let mut model_info = test_model_info("runtime_provider");
        model_info.output_cost_per_token = None;
        service.add_custom_model("partial-priced-model".to_string(), model_info);

        let error = match service.calculate_loaded_usage_cost_for_provider(
            "runtime_provider",
            "partial-priced-model",
            &PricingUsage::new(1000, 500),
        ) {
            Ok(_) => panic!("incomplete pricing must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("output_cost_per_token"));
    }
}
