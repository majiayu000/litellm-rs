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

/// Acceptance: when the catalog and the legacy `MODEL_PRICING` map both
/// publish a per-token rate for the same ID, the two must agree.
///
/// The map is currently a strict superset of the catalog pricing-bearing IDs
/// minus one known gap: `amazon.titan-embed-text-v1` carries a per-token rate
/// in `model_config.rs` but is absent from `utils/cost.rs::MODEL_PRICING`.
/// That gap is documented on the catalog entry and intentionally excluded
/// here; a future cleanup PR can either backfill the cost map or remove the
/// stale `model_config.rs` entry.
#[test]
fn catalog_pricing_matches_legacy_pricing_map() {
    // Documented one-way gap; see the catalog entry comment.
    const LEGACY_PRICING_MAP_GAP: &[&str] = &["amazon.titan-embed-text-v1"];

    for entry in all_entries() {
        let Some(expected) = entry.to_model_pricing() else {
            continue;
        };
        let Some(actual) = CostCalculator::get_model_pricing(entry.model_id) else {
            assert!(
                LEGACY_PRICING_MAP_GAP.contains(&entry.model_id),
                "{} has catalog pricing but is missing from the legacy pricing map \
                 (and is not in the documented gap list)",
                entry.model_id
            );
            continue;
        };
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
