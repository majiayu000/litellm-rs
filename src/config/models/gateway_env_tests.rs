use super::*;

const ACCESS_KEY: &str = "LITELLM_PROVIDER_OPENAI_ENDPOINT_ACCESS";

fn clear_env() {
    for key in [
        ENV_ENABLE_JWT,
        ENV_PROVIDERS,
        ENV_REDIS_ENABLED,
        ENV_REDIS_CLUSTER,
        "LITELLM_PROVIDER_OPENAI_TYPE",
        "LITELLM_PROVIDER_OPENAI_API_KEY",
        ACCESS_KEY,
    ] {
        unsafe { env::remove_var(key) };
    }
}

#[test]
fn endpoint_access_env_preserves_presence_and_rejects_invalid_values() {
    let _guard = GATEWAY_ENV_LOCK.blocking_lock();
    clear_env();
    unsafe {
        env::set_var(ENV_ENABLE_JWT, "false");
        env::set_var(ENV_PROVIDERS, "openai");
        env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
        env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test");
        env::set_var(ACCESS_KEY, "private_network");
    }
    let private = GatewayConfig::from_env().expect("private access env should parse");
    assert_eq!(
        private.providers[0].endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
    for invalid in ["", "private"] {
        unsafe { env::set_var(ACCESS_KEY, invalid) };
        assert!(GatewayConfig::from_env().is_err());
    }
    clear_env();
}

#[test]
fn public_env_overlay_tracks_explicit_false_and_omission() {
    let _guard = GATEWAY_ENV_LOCK.blocking_lock();
    clear_env();
    unsafe {
        env::set_var(ENV_ENABLE_JWT, "false");
        env::set_var(ENV_PROVIDERS, "openai");
        env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
        env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test");
        env::set_var(ENV_REDIS_CLUSTER, "false");
    }

    let mut base = crate::config::Config::default();
    base.gateway.providers.push(ProviderConfig {
        name: "openai".to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        ..ProviderConfig::default()
    });
    base.gateway.storage.redis.enabled = true;
    base.gateway.storage.redis.cluster = true;

    let explicit_false = crate::config::Config::overlay_from_env()
        .expect("public Redis environment overlay should parse");
    let merged = base.clone().merge_overlay(explicit_false);
    merged
        .validate()
        .expect("explicit false must produce a valid merged configuration");
    assert!(merged.gateway.storage.redis.enabled);
    assert!(!merged.gateway.storage.redis.cluster);

    unsafe { env::remove_var(ENV_REDIS_CLUSTER) };
    let omitted = crate::config::Config::overlay_from_env()
        .expect("omitted Redis environment overlay should parse");
    assert!(base.merge_overlay(omitted).gateway.storage.redis.cluster);
    clear_env();
}

#[test]
fn config_environment_overlay_defers_final_cluster_validation() {
    let _guard = GATEWAY_ENV_LOCK.blocking_lock();
    clear_env();
    unsafe {
        env::set_var(ENV_ENABLE_JWT, "false");
        env::set_var(ENV_PROVIDERS, "openai");
        env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
        env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test");
        env::set_var(ENV_REDIS_ENABLED, "true");
        env::set_var(ENV_REDIS_CLUSTER, "true");
    }

    assert!(crate::config::Config::from_env().is_err());
    let layer = crate::config::Config::overlay_from_env()
        .expect("environment overlay should parse before final validation")
        .into_config();
    assert!(
        layer
            .validate()
            .expect_err("materialized cluster layer must remain invalid before merge")
            .to_string()
            .contains("storage.redis.cluster=false")
    );
    clear_env();
}
