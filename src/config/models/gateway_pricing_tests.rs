use super::*;
use std::env;

const TEST_ENV_KEYS: [&str; 7] = [
    ENV_ENABLE_JWT,
    ENV_PROVIDERS,
    ENV_UNPRICED_MODEL_POLICY,
    ENV_UNPRICED_FALLBACK_COST_PER_1K_TOKENS,
    "LITELLM_PROVIDER_VLLM_TYPE",
    "LITELLM_PROVIDER_VLLM_API_KEY",
    "LITELLM_PROVIDER_VLLM_BASE_URL",
];

fn clear_pricing_test_env() {
    for key in TEST_ENV_KEYS {
        unsafe { env::remove_var(key) };
    }
}

fn pricing_config_from_yaml(yaml: &str) -> GatewayPricingConfig {
    match serde_yml::from_str(yaml) {
        Ok(pricing) => pricing,
        Err(error) => panic!("expected pricing yaml to parse: {}", error),
    }
}

#[test]
fn default_pricing_policy_rejects_unpriced_models() {
    let config = GatewayPricingConfig::default();

    assert_eq!(config.source.as_deref(), Some(DEFAULT_PRICING_SOURCE));
    assert_eq!(config.unpriced_model_policy, UnpricedModelPolicy::Reject);
    assert!(config.unpriced_fallback_cost_per_1k_tokens.is_none());
}

#[test]
fn pricing_deserializes_unpriced_policy() {
    let pricing = pricing_config_from_yaml(
        "unpriced_model_policy: allow_unpriced\nunpriced_fallback_cost_per_1k_tokens: 0.25",
    );

    assert_eq!(
        pricing.unpriced_model_policy,
        UnpricedModelPolicy::AllowUnpriced
    );
    assert_eq!(pricing.unpriced_fallback_cost_per_1k_tokens, Some(0.25));
}

#[test]
fn pricing_merge_preserves_unpriced_policy_for_default_overlay() {
    let base = GatewayPricingConfig {
        unpriced_model_policy: UnpricedModelPolicy::AllowUnpriced,
        unpriced_fallback_cost_per_1k_tokens: Some(0.2),
        ..Default::default()
    };

    let merged = base.merge(GatewayPricingConfig::default());

    assert_eq!(
        merged.unpriced_model_policy,
        UnpricedModelPolicy::AllowUnpriced
    );
    assert_eq!(merged.unpriced_fallback_cost_per_1k_tokens, Some(0.2));
}

#[test]
fn pricing_merge_uses_explicit_reject_policy_and_null_fallback() {
    let base = GatewayPricingConfig {
        unpriced_model_policy: UnpricedModelPolicy::AllowUnpriced,
        unpriced_fallback_cost_per_1k_tokens: Some(0.2),
        ..Default::default()
    };
    let other = pricing_config_from_yaml(
        "unpriced_model_policy: reject\nunpriced_fallback_cost_per_1k_tokens: null",
    );

    let merged = base.merge(other);

    assert_eq!(merged.unpriced_model_policy, UnpricedModelPolicy::Reject);
    assert!(merged.unpriced_fallback_cost_per_1k_tokens.is_none());
}

#[test]
fn env_applies_unpriced_policy() {
    let _guard = GATEWAY_ENV_LOCK.blocking_lock();
    clear_pricing_test_env();
    unsafe {
        env::set_var(ENV_ENABLE_JWT, "false");
        env::set_var(ENV_PROVIDERS, "vllm");
        env::set_var("LITELLM_PROVIDER_VLLM_TYPE", "vllm");
        env::set_var(ENV_UNPRICED_MODEL_POLICY, "allow_unpriced");
        env::set_var(ENV_UNPRICED_FALLBACK_COST_PER_1K_TOKENS, "0.15");
    }

    let config = match GatewayConfig::from_env() {
        Ok(config) => config,
        Err(error) => panic!("expected GatewayConfig::from_env() to succeed: {}", error),
    };

    assert_eq!(
        config.pricing.unpriced_model_policy,
        UnpricedModelPolicy::AllowUnpriced
    );
    assert_eq!(
        config.pricing.unpriced_fallback_cost_per_1k_tokens,
        Some(0.15)
    );
    clear_pricing_test_env();
}
