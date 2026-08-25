use crate::core::cost::types::{CostError, ModelPricing};
use chrono::{DateTime, Utc};

pub(super) fn get_pricing_with_shared_source<F>(
    model: &str,
    provider_aliases: &[&str],
    fallback: F,
) -> Result<ModelPricing, CostError>
where
    F: FnOnce(&str) -> Result<ModelPricing, CostError>,
{
    get_pricing_with_shared_source_at(model, provider_aliases, fallback, Utc::now())
}

pub(super) fn get_pricing_with_shared_source_at<F>(
    model: &str,
    provider_aliases: &[&str],
    fallback: F,
    pricing_time: DateTime<Utc>,
) -> Result<ModelPricing, CostError>
where
    F: FnOnce(&str) -> Result<ModelPricing, CostError>,
{
    // Some(..) means the shared catalog matched this model; an inner Err means
    // it matched but carried no usable pricing and must not bill at $0.
    if let Some(pricing) = get_shared_model_pricing_at(model, provider_aliases, pricing_time) {
        return pricing;
    }
    fallback(model)
}

#[cfg(test)]
pub(super) fn get_shared_model_pricing(
    model: &str,
    provider_aliases: &[&str],
) -> Option<Result<ModelPricing, CostError>> {
    get_shared_model_pricing_at(model, provider_aliases, Utc::now())
}

fn get_shared_model_pricing_at(
    model: &str,
    provider_aliases: &[&str],
    pricing_time: DateTime<Utc>,
) -> Option<Result<ModelPricing, CostError>> {
    let db = crate::core::pricing::get_pricing_db();

    if let Some(info) = db.get_model_info(model)
        && litellm_provider_matches(&info.litellm_provider, provider_aliases)
    {
        return Some(litellm_to_cost_pricing_at(model, info, pricing_time));
    }

    let normalized_model = crate::core::pricing::normalize_model_key(model);
    if normalized_model != model
        && let Some(info) = db.get_model_info(normalized_model)
        && litellm_provider_matches(&info.litellm_provider, provider_aliases)
    {
        return Some(litellm_to_cost_pricing_at(
            normalized_model,
            info,
            pricing_time,
        ));
    }

    let model_lower = normalized_model.to_lowercase();
    for provider in provider_aliases {
        let mut candidates = db.get_provider_models(provider);
        candidates.sort();

        for model_id in candidates {
            let model_id_lower = model_id.to_lowercase();
            if is_shared_model_match(&model_id_lower, &model_lower)
                && let Some(info) = db.get_model_info(&model_id)
                && litellm_provider_matches(&info.litellm_provider, provider_aliases)
            {
                return Some(litellm_to_cost_pricing_at(&model_id, info, pricing_time));
            }
        }
    }

    None
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

    candidate == requested
        || model_id_matches(candidate, requested)
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

fn litellm_provider_matches(provider: &str, aliases: &[&str]) -> bool {
    let provider = crate::core::pricing::normalize_pricing_provider(provider);
    aliases
        .iter()
        .any(|alias| crate::core::pricing::normalize_pricing_provider(alias) == provider)
}

#[cfg(test)]
pub(super) fn litellm_to_cost_pricing(
    model: &str,
    info: &crate::core::pricing::LiteLLMModelInfo,
) -> Result<ModelPricing, CostError> {
    litellm_to_cost_pricing_at(model, info, Utc::now())
}

pub(super) fn litellm_to_cost_pricing_at(
    model: &str,
    info: &crate::core::pricing::LiteLLMModelInfo,
    pricing_time: DateTime<Utc>,
) -> Result<ModelPricing, CostError> {
    if info.input_cost_per_token.is_none()
        && info.output_cost_per_token.is_none()
        && !has_non_token_pricing(info)
    {
        return Err(CostError::MissingPricing {
            model: model.to_string(),
        });
    }
    if requires_bidirectional_token_pricing(info)
        && (info.input_cost_per_token.is_none() || info.output_cost_per_token.is_none())
    {
        return Err(CostError::MissingPricing {
            model: model.to_string(),
        });
    }
    if info.input_cost_per_token.is_none() || info.output_cost_per_token.is_none() {
        tracing::warn!(
            "model '{}' is missing {} token cost; billing that side at $0",
            model,
            if info.input_cost_per_token.is_none() {
                "input"
            } else {
                "output"
            }
        );
    }

    let peak_rates = crate::core::pricing::time_of_use::peak_token_rates_at(info, pricing_time)
        .map_err(|message| CostError::ConfigError {
            message: format!("Invalid time-of-use pricing for model {model}: {message}"),
        })?;
    let input_cost_per_token = peak_rates
        .map(|rates| rates.input_cost_per_token)
        .or(info.input_cost_per_token)
        .unwrap_or(0.0);
    let output_cost_per_token = peak_rates
        .map(|rates| rates.output_cost_per_token)
        .or(info.output_cost_per_token)
        .unwrap_or(0.0);
    let cache_read_input_token_cost = peak_rates
        .map(|rates| price_per_token_to_per_1k(rates.cache_read_input_token_cost))
        .or_else(|| extra_token_cost_per_1k(info, "cache_read_input_token_cost"));

    Ok(ModelPricing {
        model: model.to_string(),
        input_cost_per_1k_tokens: price_per_token_to_per_1k(input_cost_per_token),
        output_cost_per_1k_tokens: price_per_token_to_per_1k(output_cost_per_token),
        cache_read_input_token_cost,
        cache_creation_input_token_cost: extra_token_cost_per_1k(
            info,
            "cache_creation_input_token_cost",
        ),
        input_cost_per_audio_token: extra_f64(info, "input_cost_per_audio_token"),
        output_cost_per_audio_token: extra_f64(info, "output_cost_per_audio_token"),
        image_cost_per_token: image_cost_per_token(info),
        reasoning_cost_per_token: extra_f64(info, "output_cost_per_reasoning_token"),
        cost_per_second: info.cost_per_second,
        video_cost_per_second: extra_f64(info, "video_cost_per_second"),
        audio_cost_per_second: extra_f64(info, "audio_cost_per_second"),
        cost_per_image: extra_cost_per_image(model, info)?,
        tiered_pricing: extra_tiered_pricing_per_1k(info),
        batch_discount: extra_f64(info, "batch_discount"),
        currency: "USD".to_string(),
        updated_at: pricing_time,
    })
}

fn requires_bidirectional_token_pricing(info: &crate::core::pricing::LiteLLMModelInfo) -> bool {
    matches!(info.mode.as_str(), "chat" | "completion")
        || (info.mode.is_empty() && !has_non_token_pricing(info))
}

fn has_non_token_pricing(info: &crate::core::pricing::LiteLLMModelInfo) -> bool {
    info.cost_per_second.is_some()
        || extra_f64(info, "video_cost_per_second").is_some()
        || extra_f64(info, "audio_cost_per_second").is_some()
        || image_cost_per_token(info).is_some()
        || extra_f64(info, "output_cost_per_image").is_some()
}

fn extra_f64(info: &crate::core::pricing::LiteLLMModelInfo, key: &str) -> Option<f64> {
    info.extra.get(key).and_then(serde_json::Value::as_f64)
}

fn extra_token_cost_per_1k(
    info: &crate::core::pricing::LiteLLMModelInfo,
    key: &str,
) -> Option<f64> {
    extra_f64(info, key).map(price_per_token_to_per_1k)
}

fn image_cost_per_token(info: &crate::core::pricing::LiteLLMModelInfo) -> Option<f64> {
    extra_f64(info, "image_cost_per_token")
        .or_else(|| extra_f64(info, "output_cost_per_image_token"))
}

fn extra_cost_per_image(
    model: &str,
    info: &crate::core::pricing::LiteLLMModelInfo,
) -> Result<Option<std::collections::HashMap<String, f64>>, CostError> {
    let Some(price) = extra_f64(info, "output_cost_per_image") else {
        return Ok(None);
    };
    if !price.is_finite() || price < 0.0 {
        return Err(CostError::InvalidUsage {
            message: format!(
                "Invalid image pricing for model {model}: output_cost_per_image ({price})"
            ),
        });
    }
    Ok(Some(std::collections::HashMap::from([(
        "base".to_string(),
        price,
    )])))
}

fn extra_tiered_pricing_per_1k(
    info: &crate::core::pricing::LiteLLMModelInfo,
) -> Option<std::collections::HashMap<String, f64>> {
    let tiered = info
        .extra
        .iter()
        .filter_map(|(key, value)| {
            let is_token_tier = key.starts_with("input_cost_per_token_above_")
                || key.starts_with("output_cost_per_token_above_")
                || key.starts_with("cache_creation_input_token_cost_above_")
                || key.starts_with("cache_read_input_token_cost_above_");

            if is_token_tier {
                value
                    .as_f64()
                    .map(|cost_per_token| (key.clone(), price_per_token_to_per_1k(cost_per_token)))
            } else {
                None
            }
        })
        .collect::<std::collections::HashMap<_, _>>();

    if tiered.is_empty() {
        None
    } else {
        Some(tiered)
    }
}

fn price_per_token_to_per_1k(cost_per_token: f64) -> f64 {
    let cost_per_1k = cost_per_token * 1000.0;
    (cost_per_1k * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

#[cfg(test)]
mod time_of_use_tests {
    use super::*;
    use crate::core::cost::calculator::{generic_cost_per_token_at, get_model_pricing_at};
    use crate::core::cost::types::UsageTokens;
    use chrono::TimeZone;

    #[test]
    fn compatibility_apis_apply_deepseek_peak_rates() {
        let mut usage = UsageTokens::new(1_000, 1_000);
        usage.cached_tokens = Some(250);
        let off_peak = Utc.with_ymd_and_hms(2026, 8, 29, 2, 0, 0).unwrap();
        let peak = Utc.with_ymd_and_hms(2026, 8, 24, 2, 0, 0).unwrap();

        for model in [
            "deepseek-chat",
            "deepseek-v4-flash-vision-exp",
            "deepseek-v4-pro",
            "deepseek/deepseek-reasoner",
        ] {
            let off_peak_cost =
                generic_cost_per_token_at(model, &usage, "deepseek", off_peak).unwrap();
            let peak_cost = generic_cost_per_token_at(model, &usage, "deepseek", peak).unwrap();
            assert!(
                (peak_cost.total_cost - off_peak_cost.total_cost * 2.0).abs() < 1e-12,
                "{model} should use peak token and cache rates"
            );

            let off_peak_pricing = get_model_pricing_at(model, "deepseek", off_peak).unwrap();
            let peak_pricing = get_model_pricing_at(model, "deepseek", peak).unwrap();
            assert_eq!(
                peak_pricing.input_cost_per_1k_tokens,
                off_peak_pricing.input_cost_per_1k_tokens * 2.0
            );
            assert_eq!(
                peak_pricing.cache_read_input_token_cost,
                off_peak_pricing
                    .cache_read_input_token_cost
                    .map(|rate| rate * 2.0)
            );
        }

        let prefixed_unlisted = "deepseek/deepseek-v4-flash-vision-exp-unlisted";
        let off_peak_cost =
            generic_cost_per_token_at(prefixed_unlisted, &usage, "openai_like", off_peak).unwrap();
        let peak_cost =
            generic_cost_per_token_at(prefixed_unlisted, &usage, "openai_like", peak).unwrap();
        assert!((peak_cost.total_cost - off_peak_cost.total_cost * 2.0).abs() < 1e-12);
    }
}
