use std::env;
use std::ffi::OsString;

use super::*;
use crate::core::net::ProviderEndpointAccess;

const ACCESS_KEY: &str = "LITELLM_PROVIDER_OPENAI_ENDPOINT_ACCESS";
const TEST_ENV_KEYS: &[&str] = &[
    ENV_HOST,
    ENV_PORT,
    ENV_WORKERS,
    ENV_TIMEOUT,
    ENV_DATABASE_URL,
    ENV_DATABASE_MAX_CONNECTIONS,
    ENV_DATABASE_CONNECTION_TIMEOUT,
    ENV_DATABASE_SSL,
    ENV_DATABASE_ENABLED,
    ENV_DATABASE_AUTO_MIGRATE,
    ENV_REDIS_URL,
    ENV_REDIS_ENABLED,
    ENV_REDIS_MAX_CONNECTIONS,
    ENV_REDIS_CONNECTION_TIMEOUT,
    ENV_REDIS_CLUSTER,
    ENV_ENABLE_JWT,
    ENV_ENABLE_API_KEY,
    ENV_JWT_SECRET,
    ENV_JWT_EXPIRATION,
    ENV_API_KEY_HEADER,
    ENV_PROVIDERS,
    ENV_PRICING_SOURCE,
    ENV_UNPRICED_MODEL_POLICY,
    ENV_UNPRICED_FALLBACK_COST_PER_1K_TOKENS,
    ENV_CACHE_ENABLED,
    ENV_RATE_LIMIT_ENABLED,
    ENV_ENTERPRISE_ENABLED,
    "LITELLM_PROVIDER_OPENAI_TYPE",
    "LITELLM_PROVIDER_OPENAI_API_KEY",
    "LITELLM_PROVIDER_OPENAI_BASE_URL",
    ACCESS_KEY,
    "LITELLM_PROVIDER_OPENAI_API_VERSION",
    "LITELLM_PROVIDER_OPENAI_ORGANIZATION",
    "LITELLM_PROVIDER_OPENAI_PROJECT",
    "LITELLM_PROVIDER_OPENAI_WEIGHT",
    "LITELLM_PROVIDER_OPENAI_RPM",
    "LITELLM_PROVIDER_OPENAI_TPM",
    "LITELLM_PROVIDER_OPENAI_MAX_CONCURRENT_REQUESTS",
    "LITELLM_PROVIDER_OPENAI_TIMEOUT",
    "LITELLM_PROVIDER_OPENAI_MAX_RETRIES",
    "LITELLM_PROVIDER_OPENAI_ENABLED",
    "LITELLM_PROVIDER_OPENAI_MODELS",
    "LITELLM_PROVIDER_OPENAI_TAGS",
];

struct EnvGuard(Vec<(&'static str, Option<OsString>)>);

impl EnvGuard {
    fn cleared() -> Self {
        let saved = TEST_ENV_KEYS
            .iter()
            .map(|&key| (key, env::var_os(key)))
            .collect();
        for key in TEST_ENV_KEYS {
            unsafe { env::remove_var(key) };
        }
        Self(saved)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            match value {
                Some(value) => unsafe { env::set_var(key, value) },
                None => unsafe { env::remove_var(key) },
            }
        }
    }
}

#[test]
fn endpoint_access_env_preserves_presence_and_rejects_invalid_values() {
    let _guard = GATEWAY_ENV_LOCK.blocking_lock();
    let _env = EnvGuard::cleared();
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
}

#[test]
fn public_env_overlay_tracks_explicit_false_and_omission() {
    let _guard = GATEWAY_ENV_LOCK.blocking_lock();
    let _env = EnvGuard::cleared();
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
}

#[test]
fn redis_only_env_overlay_merges_into_valid_base_without_standalone_auth_bypass() {
    let _guard = GATEWAY_ENV_LOCK.blocking_lock();
    let _env = EnvGuard::cleared();
    unsafe { env::set_var(ENV_REDIS_CLUSTER, "false") };

    let mut base = crate::config::Config::default();
    base.gateway.providers.push(ProviderConfig {
        name: "openai".to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        ..ProviderConfig::default()
    });
    base.gateway.auth.jwt_secret = "StrongJwtSecretWithMixedCaseAndNumbers1234!".to_string();
    base.gateway.auth.enable_jwt = true;
    base.gateway.auth.enable_api_key = false;
    base.gateway.ip_access = IpAccessConfig::new()
        .enable()
        .with_mode(crate::core::ip_access::IpAccessMode::Allowlist)
        .allow_ip("127.0.0.1");
    base.gateway.storage.redis.enabled = true;
    base.gateway.storage.redis.cluster = true;

    let overlay = crate::config::Config::overlay_from_env()
        .expect("Redis-only environment overlay should parse without standalone requirements");
    let merged = base.merge_overlay(overlay);
    merged
        .validate()
        .expect("valid base plus explicit cluster=false should validate");
    assert!(!merged.gateway.storage.redis.cluster);
    assert!(merged.gateway.auth.enable_jwt);
    assert!(!merged.gateway.auth.enable_api_key);
    assert!(merged.gateway.ip_access.enabled);
    assert_eq!(merged.gateway.ip_access.allowlist.len(), 1);

    let gateway_error = GatewayConfig::from_env()
        .expect_err("standalone GatewayConfig environment loading must require providers");
    assert!(gateway_error.to_string().contains(ENV_PROVIDERS));
    assert!(crate::config::Config::from_env().is_err());
}

#[test]
fn standalone_gateway_env_loader_preserves_jwt_requirement() {
    let _guard = GATEWAY_ENV_LOCK.blocking_lock();
    let _env = EnvGuard::cleared();
    unsafe {
        env::set_var(ENV_ENABLE_JWT, "true");
        env::set_var(ENV_PROVIDERS, "openai");
        env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
        env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test");
        env::set_var("LITELLM_PROVIDER_OPENAI_ENABLED", "invalid");
    }

    let error = GatewayConfig::from_env()
        .expect_err("standalone GatewayConfig environment loading must require the JWT secret");
    assert!(error.to_string().contains(ENV_JWT_SECRET));
    assert!(crate::config::Config::from_env().is_err());

    unsafe {
        env::set_var(ENV_ENABLE_JWT, "false");
        env::remove_var(ENV_PROVIDERS);
        env::set_var(ENV_CACHE_ENABLED, "invalid");
    }
    let error = GatewayConfig::from_env()
        .expect_err("standalone loading must require providers before later fields");
    assert!(error.to_string().contains(ENV_PROVIDERS));
}

#[test]
fn config_environment_overlay_defers_final_cluster_validation() {
    let _guard = GATEWAY_ENV_LOCK.blocking_lock();
    let _env = EnvGuard::cleared();
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
}
