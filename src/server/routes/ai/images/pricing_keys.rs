use crate::core::pricing_service::{LiteLLMModelInfo, PricingService};

pub(super) fn image_pricing_keys(
    pricing_provider: &str,
    pricing_model: &str,
    size: Option<&str>,
    quality: Option<&str>,
) -> Vec<String> {
    let model = pricing_model.trim();
    if model.is_empty() {
        return Vec::new();
    }

    let size = size.map(normalize_image_pricing_size);
    let qualities = image_pricing_quality_segments(quality);
    let provider = pricing_provider.trim();
    let mut keys = Vec::new();
    push_unique_key(&mut keys, model.to_string());
    if let Some(size) = size.as_deref() {
        push_unique_key(&mut keys, format!("{size}/{model}"));
        if !provider.is_empty() {
            push_unique_key(&mut keys, format!("{provider}/{size}/{model}"));
        }
        for quality in &qualities {
            push_unique_key(&mut keys, format!("{quality}/{size}/{model}"));
            push_unique_key(&mut keys, format!("{size}/{quality}/{model}"));
            if !provider.is_empty() {
                push_unique_key(&mut keys, format!("{provider}/{quality}/{size}/{model}"));
                push_unique_key(&mut keys, format!("{provider}/{size}/{quality}/{model}"));
            }
        }
    } else {
        for quality in &qualities {
            push_unique_key(&mut keys, format!("{quality}/{model}"));
            if !provider.is_empty() {
                push_unique_key(&mut keys, format!("{provider}/{quality}/{model}"));
            }
        }
    }
    keys
}

pub(super) fn resolve_image_pricing_model(
    pricing_service: &PricingService,
    pricing_provider: &str,
    model: &str,
    size: Option<&str>,
    quality: Option<&str>,
) -> Option<String> {
    image_pricing_model_candidates(model, size, quality)
        .into_iter()
        .find(|candidate| {
            pricing_service
                .get_model_info_for_provider(pricing_provider, candidate)
                .is_some_and(|(resolved, info)| {
                    resolved == candidate.as_str() && supports_image_output_pricing(&info)
                })
        })
}

pub(super) fn resolve_image_request_pricing(
    pricing: &super::super::spend::RequestPricing,
    size: Option<&str>,
    quality: Option<&str>,
) -> Option<super::super::spend::RequestPricing> {
    let (_, model) = pricing.priced_parts()?;
    image_pricing_model_candidates(model, size, quality)
        .into_iter()
        .filter_map(|candidate| pricing.with_exact_priced_model(&candidate))
        .find(|candidate| {
            candidate
                .model_info()
                .is_some_and(|info| supports_image_output_pricing(&info))
        })
}

fn image_pricing_model_candidates(
    model: &str,
    size: Option<&str>,
    quality: Option<&str>,
) -> Vec<String> {
    let model = image_pricing_base_model(model.trim());
    if model.is_empty() {
        return Vec::new();
    }

    let size = size.map(normalize_image_pricing_size);
    let qualities = image_pricing_quality_segments(quality);
    let mut candidates = Vec::new();
    if let Some(size) = size.as_deref() {
        for quality in &qualities {
            push_unique_key(&mut candidates, format!("{quality}/{size}/{model}"));
            push_unique_key(&mut candidates, format!("{size}/{quality}/{model}"));
        }
        push_unique_key(&mut candidates, format!("{size}/{model}"));
    } else {
        for quality in &qualities {
            push_unique_key(&mut candidates, format!("{quality}/{model}"));
        }
    }
    candidates
}

fn image_pricing_base_model(model: &str) -> &str {
    model
        .rsplit_once('/')
        .filter(|(prefix, _)| prefix.split('/').any(is_image_variant_segment))
        .map(|(_, model_id)| model_id)
        .unwrap_or(model)
}

fn normalize_image_pricing_size(size: &str) -> String {
    size.trim().replace('x', "-x-")
}

fn image_pricing_quality_segments(quality: Option<&str>) -> Vec<String> {
    let Some(quality) = quality
        .map(str::trim)
        .filter(|quality| !quality.is_empty())
        .map(str::to_ascii_lowercase)
    else {
        return Vec::new();
    };
    let mut segments = vec![quality.clone()];
    match quality.as_str() {
        "standard" => push_unique_key(&mut segments, "30-steps".to_string()),
        "hd" => push_unique_key(&mut segments, "50-steps".to_string()),
        _ => {}
    }
    segments
}

fn supports_image_output_pricing(info: &LiteLLMModelInfo) -> bool {
    info.input_cost_per_token.is_some()
        || info.output_cost_per_token.is_some()
        || [
            "output_cost_per_image",
            "image_cost_per_token",
            "output_cost_per_image_token",
        ]
        .into_iter()
        .any(|key| {
            info.extra
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .is_some()
        })
}

fn is_image_variant_segment(segment: &str) -> bool {
    let segment = segment.to_ascii_lowercase();
    matches!(
        segment.as_str(),
        "hd" | "standard" | "low" | "medium" | "high" | "max-steps"
    ) || segment.ends_with("-steps")
        || segment.contains("-x-")
        || segment.split_once('x').is_some_and(|(width, height)| {
            width.chars().all(|ch| ch.is_ascii_digit())
                && height.chars().all(|ch| ch.is_ascii_digit())
        })
}

fn push_unique_key(keys: &mut Vec<String>, key: String) {
    if !keys.contains(&key) {
        keys.push(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn model_info(extra: HashMap<String, serde_json::Value>) -> LiteLLMModelInfo {
        model_info_for_provider("openai", extra)
    }

    fn model_info_for_provider(
        provider: &str,
        extra: HashMap<String, serde_json::Value>,
    ) -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: None,
            output_cost_per_token: None,
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: provider.to_string(),
            mode: "image_generation".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra,
        }
    }

    #[test]
    fn resolve_image_pricing_model_skips_unsupported_input_only_variant() {
        let service = PricingService::new(None);
        service.add_custom_model(
            "medium/1024-x-1024/gpt-image-1.5".to_string(),
            model_info(HashMap::from([(
                "input_cost_per_image".to_string(),
                serde_json::Value::from(0.034),
            )])),
        );

        let resolved = resolve_image_pricing_model(
            &service,
            "openai",
            "gpt-image-1.5",
            Some("1024x1024"),
            Some("medium"),
        );

        assert_eq!(resolved, None);
    }

    #[test]
    fn resolve_image_pricing_model_accepts_output_priced_variant() {
        let service = PricingService::new(None);
        service.add_custom_model(
            "hd/1024-x-1024/flat-variant-model".to_string(),
            model_info(HashMap::from([(
                "output_cost_per_image".to_string(),
                serde_json::Value::from(0.10),
            )])),
        );

        let resolved = resolve_image_pricing_model(
            &service,
            "openai",
            "flat-variant-model",
            Some("1024x1024"),
            Some("hd"),
        );

        assert_eq!(
            resolved,
            Some("hd/1024-x-1024/flat-variant-model".to_string())
        );
    }

    #[test]
    fn unpriced_canonical_image_identity_does_not_fall_back_to_raw_alias_variant() {
        let service = PricingService::new(None);
        service.add_custom_model(
            "hd/1024-x-1024/review-public-alias".to_string(),
            model_info(HashMap::from([(
                "output_cost_per_image".to_string(),
                serde_json::Value::from(0.10),
            )])),
        );

        let resolved = resolve_image_pricing_model(
            &service,
            "openai",
            "review-canonical-unpriced",
            Some("1024x1024"),
            Some("hd"),
        );

        assert_eq!(resolved, None);
    }

    #[test]
    fn image_pricing_keys_include_bedrock_hd_step_alias() {
        let keys = image_pricing_keys(
            "bedrock",
            "stability.stable-diffusion-xl-v1",
            Some("1024x1024"),
            Some("hd"),
        );

        assert!(
            keys.contains(&"1024-x-1024/50-steps/stability.stable-diffusion-xl-v1".to_string())
        );
    }

    #[test]
    fn resolve_image_pricing_model_accepts_bedrock_hd_step_variant() {
        let service = PricingService::new(None);
        service.add_custom_model(
            "1024-x-1024/50-steps/stability.stable-diffusion-xl-v1".to_string(),
            model_info_for_provider(
                "bedrock",
                HashMap::from([(
                    "output_cost_per_image".to_string(),
                    serde_json::Value::from(0.04),
                )]),
            ),
        );

        let resolved = resolve_image_pricing_model(
            &service,
            "bedrock",
            "stability.stable-diffusion-xl-v1",
            Some("1024x1024"),
            Some("hd"),
        );

        assert_eq!(
            resolved,
            Some("1024-x-1024/50-steps/stability.stable-diffusion-xl-v1".to_string())
        );
    }

    fn assert_explicit_mapping_wins_over_wire_variant() {
        let service = PricingService::new(None);
        for (model, cost) in [
            ("mapped-image", 0.01),
            ("hd/1024-x-1024/mapped-image", 0.02),
            ("hd/1024-x-1024/wire-image", 0.90),
        ] {
            service.add_custom_model(
                model.to_string(),
                model_info(HashMap::from([(
                    "output_cost_per_image".to_string(),
                    serde_json::Value::from(cost),
                )])),
            );
        }
        let mapped = super::super::super::spend::RequestPricing::from_exact(
            &service,
            "openai",
            "mapped-image",
        );

        let resolved = resolve_image_request_pricing(&mapped, Some("1024x1024"), Some("hd"))
            .expect("mapped image variant should resolve");

        assert_eq!(
            resolved.priced_parts(),
            Some(("openai", "hd/1024-x-1024/mapped-image"))
        );
    }

    #[test]
    fn generation_variant_stays_inside_explicit_pricing_mapping() {
        assert_explicit_mapping_wins_over_wire_variant();
    }

    #[test]
    fn edit_variant_stays_inside_explicit_pricing_mapping() {
        assert_explicit_mapping_wins_over_wire_variant();
    }

    #[test]
    fn variation_variant_stays_inside_explicit_pricing_mapping() {
        assert_explicit_mapping_wins_over_wire_variant();
    }
}
