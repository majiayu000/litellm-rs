use super::model_identity::{
    DeploymentModelIdentity, ModelIdentityMapping, validate_deployment_identity,
};
use crate::core::pricing_service::{LiteLLMModelInfo, PricingService};
use crate::core::providers::registry::model_catalog_authority::CatalogAuthority;
use std::collections::HashMap;

fn pricing_info(provider: &str) -> LiteLLMModelInfo {
    LiteLLMModelInfo {
        max_tokens: Some(4096),
        max_input_tokens: Some(4096),
        max_output_tokens: Some(1024),
        input_cost_per_token: Some(0.01),
        output_cost_per_token: Some(0.02),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: provider.to_string(),
        mode: "chat".to_string(),
        supports_function_calling: None,
        supports_vision: None,
        supports_streaming: None,
        supports_parallel_function_calling: None,
        supports_system_message: None,
        extra: HashMap::new(),
    }
}

fn authorities() -> (CatalogAuthority, PricingService) {
    let catalog = CatalogAuthority::from_embedded().expect("embedded catalog authority");
    let pricing = PricingService::new(None);
    (catalog, pricing)
}

#[test]
fn mapping_serde_round_trip_preserves_explicit_unpriced() {
    let mapping = ModelIdentityMapping::new(Some("gpt-4".to_string()), None);
    let json = serde_json::to_value(&mapping).expect("serialize mapping");
    assert!(
        json.get("pricing_model")
            .is_some_and(serde_json::Value::is_null)
    );
    assert_eq!(
        serde_json::from_value::<ModelIdentityMapping>(json).expect("deserialize mapping"),
        mapping
    );
}

#[test]
fn runtime_pricing_mapping_uses_injected_snapshot_and_preserves_wire_model() {
    let (catalog, pricing) = authorities();
    pricing.add_custom_model("runtime-only-price".to_string(), pricing_info("openai"));
    let snapshot = pricing.snapshot();
    let mapping = ModelIdentityMapping::new(
        Some("gpt-4".to_string()),
        Some("runtime-only-price".to_string()),
    );

    let identity = validate_deployment_identity(
        "edge-openai",
        "openai",
        "wire-deployment",
        Some(&mapping),
        None,
        &catalog,
        &snapshot,
    )
    .expect("runtime-only target should validate against injected snapshot");

    assert_eq!(identity.wire_model(), "wire-deployment");
    assert_eq!(identity.capability_catalog_model(), Some("gpt-4"));
    assert_eq!(identity.pricing_provider(), Some("openai"));
    assert_eq!(identity.pricing_model(), Some("runtime-only-price"));
}

#[test]
fn explicit_unpriced_never_inherits_raw_wire_pricing() {
    let (catalog, pricing) = authorities();
    pricing.add_custom_model("gpt-4".to_string(), pricing_info("openai"));
    let mapping = ModelIdentityMapping::new(Some("gpt-4".to_string()), None);
    let identity = validate_deployment_identity(
        "edge-openai",
        "openai",
        "gpt-4",
        Some(&mapping),
        None,
        &catalog,
        &pricing.snapshot(),
    )
    .expect("explicit unpriced mapping is valid");
    assert_eq!(identity.pricing_model(), None);
}

#[test]
fn explicit_mapping_precedes_legacy_and_raw_catalog() {
    let (catalog, pricing) = authorities();
    for model in ["gpt-4", "gpt-4o-mini"] {
        pricing.add_custom_model(model.to_string(), pricing_info("openai"));
    }
    let explicit = ModelIdentityMapping::new(
        Some("gpt-4o-mini".to_string()),
        Some("gpt-4o-mini".to_string()),
    );
    let identity = validate_deployment_identity(
        "edge-openai",
        "openai",
        "gpt-4",
        Some(&explicit),
        Some("gpt-4"),
        &catalog,
        &pricing.snapshot(),
    )
    .expect("explicit mapping should win");
    assert_eq!(identity.capability_catalog_model(), Some("gpt-4o-mini"));
    assert_eq!(identity.pricing_model(), Some("gpt-4o-mini"));
}

#[test]
fn wrong_provider_unknown_and_pricing_only_capability_fail_closed() {
    let (catalog, pricing) = authorities();
    let snapshot = pricing.snapshot();
    for (provider, capability, expected) in [
        ("azure", "azure_ai/Phi-4", "provider"),
        ("openai", "fake-gpt-5-2099-01-01", "unknown"),
        ("openai", "openai/container", "pricing-only"),
    ] {
        let mapping = ModelIdentityMapping::new(Some(capability.to_string()), None);
        let error = validate_deployment_identity(
            "edge",
            provider,
            "wire",
            Some(&mapping),
            None,
            &catalog,
            &snapshot,
        )
        .expect_err("invalid capability target must fail");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn exact_catalog_auto_resolution_needs_no_redundant_mapping() {
    let (catalog, pricing) = authorities();
    pricing.add_custom_model("gpt-4".to_string(), pricing_info("openai"));
    let identity: DeploymentModelIdentity = validate_deployment_identity(
        "edge-openai",
        "openai",
        "gpt-4",
        None,
        None,
        &catalog,
        &pricing.snapshot(),
    )
    .expect("exact callable catalog model should auto-resolve");
    assert_eq!(identity.wire_model(), "gpt-4");
    assert_eq!(identity.capability_catalog_model(), Some("gpt-4"));
}
