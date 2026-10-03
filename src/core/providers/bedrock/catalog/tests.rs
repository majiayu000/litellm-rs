//! Cross-reference invariants for the Bedrock catalog.
//!
//! These tests enforce the acceptance criteria of issue #576:
//!
//! * No pricing ID exists without matching capability metadata.
//! * No metadata ID exists without an explicit pricing state (either
//!   `Some(pricing)` or a documented `NoPricingReason`).
//! * Catalog projections match the public [`ModelConfig`] facade and
//!   [`ModelPricing`] map bit-for-bit (numerically), so existing Bedrock
//!   `model_config` and `cost` tests continue to pass.

use std::collections::HashSet;

use super::super::model_config::get_all_model_ids;
use super::super::utils::cost::CostCalculator;
use super::{all_entries, all_model_ids, get_catalog_entry};

fn catalog_ids() -> HashSet<&'static str> {
    all_entries().iter().map(|e| e.model_id).collect()
}

fn public_model_config_ids() -> HashSet<&'static str> {
    get_all_model_ids().into_iter().collect()
}

fn legacy_pricing_ids() -> HashSet<&'static str> {
    CostCalculator::get_all_models().into_iter().collect()
}

#[test]
fn catalog_is_non_empty() {
    assert!(!all_entries().is_empty(), "catalog must seed entries");
    assert!(
        all_model_ids().len() >= 30,
        "catalog should cover all known Bedrock IDs"
    );
}

#[test]
fn catalog_has_no_duplicate_model_ids() {
    let mut seen: HashSet<&'static str> = HashSet::new();
    for entry in all_entries() {
        assert!(
            seen.insert(entry.model_id),
            "catalog has duplicate entry for {}",
            entry.model_id
        );
    }
}

/// Acceptance: every ID with pricing in the legacy `MODEL_PRICING` map must
/// have matching capability metadata in the catalog.
#[test]
fn no_pricing_id_without_capability_metadata() {
    let catalog = catalog_ids();
    let pricing = legacy_pricing_ids();
    let missing: Vec<&&str> = pricing.difference(&catalog).collect();
    assert!(
        missing.is_empty(),
        "the following pricing IDs lack catalog capability metadata: {:?}",
        missing
    );
}

/// Acceptance: every ID exposed by the public `model_config` facade must have
/// a catalog entry, and that entry must carry an explicit pricing state (either
/// pricing or a documented no-pricing reason).
#[test]
fn no_metadata_id_without_pricing_state() {
    let catalog = catalog_ids();
    let metadata = public_model_config_ids();
    let missing: Vec<&&str> = metadata.difference(&catalog).collect();
    assert!(
        missing.is_empty(),
        "the following metadata IDs lack a catalog entry: {:?}",
        missing
    );

    // Every catalog entry must declare a pricing state.
    let entries_without_state: Vec<&'static str> = all_entries()
        .iter()
        .filter(|e| !e.has_pricing_state())
        .map(|e| e.model_id)
        .collect();
    assert!(
        entries_without_state.is_empty(),
        "catalog entries missing pricing state: {:?}",
        entries_without_state
    );
}

/// Every priced catalog model must be available to the runtime cost calculator.
#[test]
fn catalog_pricing_matches_runtime_pricing_map() {
    for entry in all_entries() {
        let Some(expected) = entry.to_model_pricing() else {
            continue;
        };
        let actual = CostCalculator::get_model_pricing(entry.model_id)
            .expect("catalog pricing must be projected into runtime cost lookup");
        assert!(
            (actual.input_cost_per_1k_tokens - expected.input_cost_per_1k_tokens).abs() < 1e-9,
            "input pricing drift for {}: legacy={}, catalog={}",
            entry.model_id,
            actual.input_cost_per_1k_tokens,
            expected.input_cost_per_1k_tokens
        );
        assert!(
            (actual.output_cost_per_1k_tokens - expected.output_cost_per_1k_tokens).abs() < 1e-9,
            "output pricing drift for {}: legacy={}, catalog={}",
            entry.model_id,
            actual.output_cost_per_1k_tokens,
            expected.output_cost_per_1k_tokens
        );
    }
}

/// Acceptance: catalog seeds round-trip into the existing model-config facade.
#[test]
fn catalog_model_config_matches_public_config_facade() {
    use super::super::model_config::get_model_config;

    for entry in all_entries() {
        if matches!(entry.lifecycle, super::ModelLifecycle::Retired { .. }) {
            assert!(get_model_config(entry.model_id).is_err());
            continue;
        }
        let projected = entry.to_model_config();
        let actual = match get_model_config(entry.model_id) {
            Ok(cfg) => cfg,
            Err(_) => panic!(
                "catalog seeds {} but the public model_config facade does not",
                entry.model_id
            ),
        };
        assert_eq!(
            projected.family, actual.family,
            "family drift for {}",
            entry.model_id
        );
        assert_eq!(
            projected.api_type, actual.api_type,
            "api_type drift for {}",
            entry.model_id
        );
        assert_eq!(
            projected.supports_streaming, actual.supports_streaming,
            "supports_streaming drift for {}",
            entry.model_id
        );
        assert_eq!(
            projected.supports_function_calling, actual.supports_function_calling,
            "supports_function_calling drift for {}",
            entry.model_id
        );
        assert_eq!(
            projected.supports_multimodal, actual.supports_multimodal,
            "supports_multimodal drift for {}",
            entry.model_id
        );
        assert_eq!(
            projected.max_context_length, actual.max_context_length,
            "max_context_length drift for {}",
            entry.model_id
        );
        assert_eq!(
            projected.max_output_length, actual.max_output_length,
            "max_output_length drift for {}",
            entry.model_id
        );
        assert!(
            (projected.input_cost_per_1k - actual.input_cost_per_1k).abs() < 1e-9,
            "input cost drift for {}",
            entry.model_id
        );
        assert!(
            (projected.output_cost_per_1k - actual.output_cost_per_1k).abs() < 1e-9,
            "output cost drift for {}",
            entry.model_id
        );
    }
}

/// Acceptance: vendor inferrable from the model ID prefix.
#[test]
fn vendor_prefix_matches_catalog_vendor() {
    use super::BedrockVendor;

    for entry in all_entries() {
        let inferred = BedrockVendor::from_model_id(entry.model_id).unwrap_or_else(|| {
            panic!(
                "could not infer a vendor from {} — catalog must classify all IDs",
                entry.model_id
            )
        });
        assert_eq!(
            inferred, entry.vendor,
            "vendor mismatch for {}: prefix says {:?}, catalog says {:?}",
            entry.model_id, inferred, entry.vendor
        );
    }
}

#[test]
fn lookup_helpers_return_seeded_entry() {
    let entry = get_catalog_entry("anthropic.claude-3-opus-20240229")
        .expect("Claude 3 Opus must be in the catalog");
    assert_eq!(entry.vendor, super::BedrockVendor::Anthropic);
    assert!(entry.pricing.is_some());

    let kimi = get_catalog_entry("moonshotai.kimi-k2.5")
        .expect("Kimi K2.5 must use the Bedrock runtime model ID");
    assert_eq!(kimi.vendor, super::BedrockVendor::Moonshot);
    assert_eq!(kimi.limits.max_context_length, 256_000);
    assert_eq!(kimi.limits.max_output_length, Some(16_000));
    assert!(kimi.capabilities.multimodal);
    assert!(get_catalog_entry("moonshot.kimi-k2.5").is_none());

    let thinking = get_catalog_entry("moonshot.kimi-k2-thinking")
        .expect("Kimi K2 Thinking must use the Bedrock runtime model ID");
    assert_eq!(thinking.limits.max_context_length, 256_000);
    assert_eq!(thinking.limits.max_output_length, Some(16_000));
    assert!(!thinking.capabilities.multimodal);
}

#[test]
fn current_bedrock_models_keep_platform_specific_limits_and_scopes() {
    use super::InferenceProfileScope;
    let premier = get_catalog_entry("amazon.nova-premier-v1:0").unwrap();
    assert_eq!(premier.limits.max_output_length, Some(25_000));
    assert_eq!(
        premier.inference_profiles,
        &[InferenceProfileScope::UnitedStates]
    );
    let sol = get_catalog_entry("openai.gpt-6.1-sol").unwrap();
    assert_eq!(sol.limits.max_output_length, Some(131_072));
    assert_eq!(
        sol.inference_profiles,
        &[
            InferenceProfileScope::UnitedStates,
            InferenceProfileScope::Global
        ]
    );
    assert_eq!(
        sol.pricing.as_ref().unwrap().input_cost_per_1k_tokens,
        0.0022
    );
    let sonnet = get_catalog_entry("anthropic.claude-sonnet-5-5").unwrap();
    assert_eq!(sonnet.inference_profiles, &[InferenceProfileScope::Global]);
    assert!(sonnet.capabilities.thinking);
}

#[test]
fn generic_converse_metadata_does_not_use_nova_defaults() {
    use super::super::model_config::BedrockModelFamily;
    let oss = get_catalog_entry("openai.gpt-oss-120b-1:0").unwrap();
    assert_eq!(oss.family, BedrockModelFamily::GenericConverse);
    assert_eq!(oss.limits.max_context_length, 128_000);
    assert_eq!(oss.limits.max_output_length, Some(16_000));
    assert!(!oss.capabilities.vision);
    assert_eq!(
        oss.inference_profiles,
        &[super::InferenceProfileScope::UnitedStatesGovCloud]
    );
    assert!(oss.capabilities.thinking);
    assert!(
        !get_catalog_entry("deepseek.r1-v1:0")
            .unwrap()
            .capabilities
            .function_calling
    );
    let scout = get_catalog_entry("meta.llama4-scout-17b-instruct-v1:0").unwrap();
    assert_eq!(scout.limits.max_context_length, 10_000_000);
    assert!(scout.capabilities.vision);
    let qwen = get_catalog_entry("qwen.qwen3-32b-v1:0").unwrap();
    assert_eq!(qwen.limits.max_context_length, 32_000);
    assert!(qwen.inference_profiles.is_empty());
    let voxtral = get_catalog_entry("mistral.voxtral-mini-3b-2507").unwrap();
    assert_eq!(voxtral.limits.max_output_length, None);
    assert!(!voxtral.capabilities.vision);
    assert!(
        get_catalog_entry("moonshot.kimi-k2-thinking")
            .unwrap()
            .capabilities
            .thinking
    );
    for id in ["amazon.nova-sonic-v1:0", "amazon.nova-2-sonic-v1:0"] {
        assert!(
            get_catalog_entry(id).is_none(),
            "speech-only models cannot be advertised as Converse chat"
        );
    }
    assert!(matches!(
        get_catalog_entry("amazon.nova-premier-v1:0")
            .unwrap()
            .lifecycle,
        super::ModelLifecycle::Retired {
            retirement_date: "2026-09-14"
        }
    ));
}

#[test]
fn corrected_catalog_prices_reach_runtime_cost_calculation() {
    for (id, input, output) in [
        ("openai.gpt-oss-120b-1:0", 0.15, 0.60),
        ("google.gemma-3-4b-it", 0.04, 0.08),
        ("deepseek.r1-v1:0", 1.35, 5.40),
        ("moonshot.kimi-k2-thinking", 0.60, 2.50),
        ("writer.palmyra-x4-v1:0", 2.50, 10.00),
    ] {
        let cost = CostCalculator::calculate_cost(id, 1_000_000, 1_000_000).unwrap();
        assert!(
            (cost - input - output).abs() < 1e-9,
            "wrong total for {id}: {cost}"
        );
    }
    assert!(CostCalculator::get_model_pricing("amazon.titan-embed-text-v1").is_some());
}

#[test]
fn retired_models_keep_historical_prices_without_being_routable() {
    use super::super::model_config::{get_model_config, model_supports_capability};
    let advertised = get_all_model_ids();
    for id in [
        "anthropic.claude-3-haiku-20240307-v1:0",
        "cohere.command-r-v1:0",
        "cohere.command-r-plus-v1:0",
        "amazon.nova-premier-v1:0",
        "amazon.nova-canvas-v1:0",
        "amazon.nova-reel-v1:0",
        "amazon.nova-reel-v1:1",
        "meta.llama2-13b-chat-v1",
        "meta.llama2-70b-chat-v1",
        "anthropic.claude-v2",
        "anthropic.claude-v2:1",
        "anthropic.claude-instant-v1",
    ] {
        assert!(get_catalog_entry(id).unwrap().pricing.is_some(), "{id}");
        assert!(get_model_config(id).is_err(), "{id}");
        assert!(!advertised.contains(&id), "{id}");
        assert!(!model_supports_capability(id, "streaming"), "{id}");
    }
}
