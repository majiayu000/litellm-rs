use super::*;

const ACCESS_KEY: &str = "LITELLM_PROVIDER_OPENAI_ENDPOINT_ACCESS";

fn clear_env() {
    for key in [
        ENV_ENABLE_JWT,
        ENV_PROVIDERS,
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
fn redis_cluster_env_false_overrides_true_base() {
    let _guard = GATEWAY_ENV_LOCK.blocking_lock();
    clear_env();
    unsafe {
        env::set_var(ENV_ENABLE_JWT, "false");
        env::set_var(ENV_PROVIDERS, "openai");
        env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
        env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test");
        env::set_var(ENV_REDIS_CLUSTER, "false");
    }

    let overlay = GatewayConfig::from_env().expect("Redis cluster env should parse");
    assert!(!overlay.storage.redis.cluster);
    assert!(overlay.storage.redis.cluster_configured);

    let mut base = GatewayConfig::default();
    base.storage.redis.cluster = true;
    let merged = base.merge(overlay);

    assert!(!merged.storage.redis.cluster);
    clear_env();
}
