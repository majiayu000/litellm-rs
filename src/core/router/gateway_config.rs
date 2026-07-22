//! Gateway configuration integration
//!
//! This module contains the from_gateway_config method for creating
//! a Router from gateway configuration.

use super::config::RouterConfig;
use super::deployment::{
    Deployment, DeploymentConfig, HealthCheckPolicy, LegacySelectorMetadata, RetrySchedule,
};
use super::error::RouterError;
use super::unified::Router;
use crate::config::Validate;
use crate::config::models::provider::ProviderConfig;
use crate::config::models::router::GatewayRouterConfig;
use crate::core::providers::{Provider, create_provider};

/// Return the only credential provenance audited for the conservative D3Ca
/// intermediate state.
///
/// The canonical OpenAI factory inserts a non-empty top-level `api_key` before
/// merging settings and its builder consumes that exact value. A custom
/// Authorization header makes the upstream credential ambiguous. Aliases,
/// settings-sourced credentials, catalog dispatch and provider-specific/env
/// fallbacks remain intentionally unpublishable until D3Cb normalizes them at
/// the construction boundary.
fn proven_top_level_legacy_credential(config: &ProviderConfig) -> Option<&str> {
    let selector = config.provider_type.trim();
    let credential = config.api_key.as_str();
    let has_competing_credential = ["api_key", "api_token", "google_api_key", "gemini_api_key"]
        .into_iter()
        .any(|key| config.settings.contains_key(key));
    let has_custom_authorization = ["headers", "custom_headers"].into_iter().any(|key| {
        config
            .settings
            .get(key)
            .and_then(serde_json::Value::as_object)
            .is_some_and(|headers| {
                headers.iter().any(|(name, value)| {
                    name.eq_ignore_ascii_case("authorization") && value.is_string()
                })
            })
    });

    (selector == "openai"
        && !credential.trim().is_empty()
        && !has_competing_credential
        && !has_custom_authorization)
        .then_some(credential)
}

/// Build runtime router config from gateway YAML router config.
pub fn runtime_router_config_from_gateway(
    config: &GatewayRouterConfig,
) -> Result<RouterConfig, String> {
    Validate::validate(config)?;

    Ok(RouterConfig {
        routing_strategy: config.strategy,
        allowed_fails: config.circuit_breaker.failure_threshold,
        min_requests: config.circuit_breaker.min_requests,
        cooldown_time_secs: config.circuit_breaker.recovery_timeout,
        success_threshold: config.circuit_breaker.success_threshold,
        enable_pre_call_checks: config.load_balancer.health_check_enabled,
        ..RouterConfig::default()
    })
}

impl Router {
    /// Create a Router from gateway configuration
    ///
    /// This method initializes a Router with deployments created from provider configurations.
    /// Each provider in the config becomes a deployment in the router.
    pub async fn from_gateway_config(
        providers: &[ProviderConfig],
        router_config: Option<RouterConfig>,
    ) -> Result<Self, RouterError> {
        let config = router_config.unwrap_or_default();
        let router = Self::new(config);

        for provider_config in providers {
            provider_config
                .validate_health_check_runtime()
                .map_err(|error| {
                    RouterError::InvalidConfiguration(format!(
                        "provider '{}': {error}",
                        provider_config.name
                    ))
                })?;
            if !provider_config.enabled {
                continue;
            }

            // Create provider instance via the single canonical factory.
            let provider = create_provider(provider_config.clone())
                .await
                .map_err(|e| {
                    RouterError::DeploymentNotFound(format!(
                        "Failed to create provider {}: {}",
                        provider_config.name, e
                    ))
                })?;
            let legacy_metadata = proven_top_level_legacy_credential(provider_config)
                .map(LegacySelectorMetadata::from_stored_credential);

            // Determine which models this deployment serves
            let models: Vec<String> = if !provider_config.models.is_empty() {
                provider_config.models.clone()
            } else {
                provider
                    .list_models()
                    .iter()
                    .map(|m| m.id.clone())
                    .collect()
            };

            // Create deployments
            if models.is_empty() {
                // Create a single deployment with provider name
                let deployment = create_deployment_from_config(
                    &provider_config.name,
                    provider.clone(),
                    &provider_config.name,
                    provider_config,
                )?;
                match legacy_metadata {
                    Some(metadata) => router.add_gateway_deployment(deployment, metadata),
                    None => router.add_deployment(deployment),
                }
            } else {
                // Create one deployment per model
                for model in models {
                    let deployment_id = format!("{}-{}", provider_config.name, model);
                    let deployment = create_deployment_from_config(
                        &deployment_id,
                        provider.clone(),
                        &model,
                        provider_config,
                    )?;
                    match legacy_metadata.clone() {
                        Some(metadata) => router.add_gateway_deployment(deployment, metadata),
                        None => router.add_deployment(deployment),
                    }
                }
            }
        }

        router.start_configured_health_checks()?;
        Ok(router)
    }
}

/// Helper function to create deployment from provider config
fn create_deployment_from_config(
    deployment_id: &str,
    provider: Provider,
    model: &str,
    config: &ProviderConfig,
) -> Result<Deployment, RouterError> {
    let deployment_config = deployment_config_from_provider(config)?;

    Ok(Deployment::new(
        deployment_id.to_string(),
        provider,
        model.to_string(),
        model.to_string(),
    )
    .with_config(deployment_config)
    .with_tags(config.tags.clone()))
}

fn deployment_config_from_provider(
    config: &ProviderConfig,
) -> Result<DeploymentConfig, RouterError> {
    let endpoint = config.resolved_health_check_endpoint().map_err(|error| {
        RouterError::InvalidConfiguration(format!("provider '{}': {error}", config.name))
    })?;

    Ok(DeploymentConfig {
        tpm_limit: if config.tpm > 0 {
            Some(config.tpm as u64)
        } else {
            None
        },
        rpm_limit: if config.rpm > 0 {
            Some(config.rpm as u64)
        } else {
            None
        },
        max_parallel_requests: if config.max_concurrent_requests > 0 {
            Some(config.max_concurrent_requests)
        } else {
            None
        },
        weight: (config.weight.max(1.0)).round() as u32,
        timeout_secs: config.timeout,
        priority: 0,
        retry_schedule: Some(RetrySchedule {
            base_delay_ms: config.retry.base_delay,
            max_delay_ms: config.retry.max_delay,
            backoff_multiplier: config.retry.backoff_multiplier,
            jitter_ratio: config.retry.jitter,
        }),
        health_check_policy: config.health_check.has_runtime_overrides().then(|| {
            HealthCheckPolicy {
                provider_name: config.name.clone(),
                interval_secs: config.health_check.interval,
                failure_threshold: config.health_check.failure_threshold,
                recovery_timeout_secs: config.health_check.recovery_timeout,
                endpoint,
                endpoint_access: config.endpoint_access,
                expected_codes: config.health_check.expected_codes.clone(),
            }
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::provider::RetryConfig as ProviderRetryConfig;
    use crate::config::models::router::{
        CircuitBreakerConfig, GatewayRouterConfig, LoadBalancerConfig, RoutingStrategyConfig,
    };
    use crate::core::providers::unified_provider::ProviderError;
    use crate::utils::auth::crypto::hmac::CredentialDigest;
    use std::cell::Cell;
    use std::mem::size_of;

    macro_rules! assert_not_impl_any {
        ($type:ty: $($trait:path),+ $(,)?) => {
            const _: fn() = || {
                trait AmbiguousIfImpl<A> {
                    fn marker() {}
                }
                impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
                $({
                    struct Invalid;
                    impl<T: ?Sized + $trait> AmbiguousIfImpl<Invalid> for T {}
                })+
                <$type as AmbiguousIfImpl<_>>::marker();
            };
        };
    }

    // Compile-time proof replaces the former source-AST meta-test.
    const _: [(); 32] = [(); size_of::<CredentialDigest>()];
    const _: [(); size_of::<CredentialDigest>()] = [(); size_of::<LegacySelectorMetadata>()];
    assert_not_impl_any!(
        CredentialDigest: std::fmt::Display, serde::Serialize, serde::de::DeserializeOwned
    );
    assert_not_impl_any!(
        LegacySelectorMetadata: std::fmt::Display, serde::Serialize, serde::de::DeserializeOwned
    );

    fn credential_test_provider(name: &str, api_key: &str) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            provider_type: "openai".to_string(),
            api_key: api_key.to_string(),
            models: vec!["credential-model".to_string()],
            ..ProviderConfig::default()
        }
    }

    #[test]
    fn conservative_provenance_rejects_alias_catalog_and_env_fallback_candidates() {
        let canonical = credential_test_provider("canonical", "sk-canonical");
        assert_eq!(
            proven_top_level_legacy_credential(&canonical),
            Some("sk-canonical")
        );

        for provider_type in ["azure-openai", "openrouter", "cohere"] {
            let mut unproven = credential_test_provider("unproven", "sk-unproven");
            unproven.provider_type = provider_type.to_string();
            assert_eq!(proven_top_level_legacy_credential(&unproven), None);
        }

        let empty_top_level = credential_test_provider("env-fallback", "");
        assert_eq!(proven_top_level_legacy_credential(&empty_top_level), None);
    }

    #[test]
    fn test_runtime_router_config_from_gateway_round_robin() {
        let gateway = GatewayRouterConfig::default();
        let runtime = runtime_router_config_from_gateway(&gateway).unwrap();
        assert_eq!(
            runtime.routing_strategy,
            super::super::config::RoutingStrategy::RoundRobin
        );
    }

    #[test]
    fn test_runtime_router_config_from_gateway_strategy_mapping() {
        let gateway = GatewayRouterConfig {
            strategy: RoutingStrategyConfig::LatencyBased,
            circuit_breaker: CircuitBreakerConfig::default(),
            load_balancer: LoadBalancerConfig::default(),
        };
        let runtime = runtime_router_config_from_gateway(&gateway).unwrap();
        assert_eq!(
            runtime.routing_strategy,
            super::super::config::RoutingStrategy::LatencyBased
        );
    }

    #[test]
    fn test_runtime_router_config_from_gateway_circuit_breaker_mapping() {
        let gateway = GatewayRouterConfig {
            strategy: RoutingStrategyConfig::RoundRobin,
            circuit_breaker: CircuitBreakerConfig {
                failure_threshold: 8,
                recovery_timeout: 45,
                min_requests: 20,
                success_threshold: 5,
            },
            load_balancer: LoadBalancerConfig::default(),
        };
        let runtime = runtime_router_config_from_gateway(&gateway).unwrap();
        assert_eq!(runtime.allowed_fails, 8);
        assert_eq!(runtime.cooldown_time_secs, 45);
        assert_eq!(runtime.min_requests, 20);
        assert_eq!(runtime.success_threshold, 5);
    }

    #[test]
    fn test_runtime_router_config_rejects_sticky_sessions() {
        let gateway = GatewayRouterConfig {
            strategy: RoutingStrategyConfig::RoundRobin,
            circuit_breaker: CircuitBreakerConfig::default(),
            load_balancer: LoadBalancerConfig {
                sticky_sessions: true,
                ..LoadBalancerConfig::default()
            },
        };

        let err = runtime_router_config_from_gateway(&gateway).unwrap_err();
        assert!(err.contains("sticky_sessions"));
    }

    #[test]
    fn test_runtime_router_config_rejects_session_timeout() {
        let gateway = GatewayRouterConfig {
            strategy: RoutingStrategyConfig::RoundRobin,
            circuit_breaker: CircuitBreakerConfig::default(),
            load_balancer: LoadBalancerConfig {
                session_timeout: 900,
                ..LoadBalancerConfig::default()
            },
        };

        let err = runtime_router_config_from_gateway(&gateway).unwrap_err();
        assert!(err.contains("session_timeout"));
    }

    #[test]
    fn test_provider_retry_schedule_maps_to_deployment_config() {
        let provider = ProviderConfig {
            retry: ProviderRetryConfig {
                base_delay: 250,
                max_delay: 900,
                backoff_multiplier: 2.0,
                jitter: 0.0,
            },
            ..ProviderConfig::default()
        };

        let deployment = deployment_config_from_provider(&provider).unwrap();
        let retry = deployment
            .retry_schedule
            .expect("gateway provider retry schedule should be preserved");

        assert_eq!(retry.base_delay_ms, 250);
        assert_eq!(retry.max_delay_ms, 900);
        assert!((retry.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert!((retry.jitter_ratio - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_health_policy_maps_every_runtime_field() {
        let provider = ProviderConfig {
            name: "openai-primary".to_string(),
            base_url: Some("https://8.8.8.8/v1/".to_string()),
            health_check: crate::config::models::provider::ProviderHealthCheckConfig {
                interval: 11,
                failure_threshold: 3,
                recovery_timeout: 47,
                endpoint: Some("health".to_string()),
                expected_codes: vec![200, 204],
            },
            ..ProviderConfig::default()
        };

        let deployment = deployment_config_from_provider(&provider).unwrap();
        let policy = deployment
            .health_check_policy
            .expect("gateway deployment should carry health policy");

        assert_eq!(policy.provider_name, "openai-primary");
        assert_eq!(policy.interval_secs, 11);
        assert_eq!(policy.failure_threshold, 3);
        assert_eq!(policy.recovery_timeout_secs, 47);
        assert_eq!(
            policy.endpoint_access,
            crate::core::net::ProviderEndpointAccess::PublicOnly
        );
        assert_eq!(
            policy.endpoint.expect("endpoint should resolve").as_str(),
            "https://8.8.8.8/v1/health"
        );
        assert_eq!(policy.expected_codes, vec![200, 204]);
    }

    #[test]
    fn test_default_provider_health_config_preserves_no_probe_behavior() {
        let deployment = deployment_config_from_provider(&ProviderConfig::default()).unwrap();

        assert!(deployment.health_check_policy.is_none());
    }

    #[tokio::test]
    async fn test_direct_factory_rejects_invalid_provider_health_config() {
        let provider = ProviderConfig {
            name: "disabled-invalid".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test-key".to_string(),
            enabled: false,
            health_check: crate::config::models::provider::ProviderHealthCheckConfig {
                endpoint: Some("/health".to_string()),
                ..Default::default()
            },
            ..ProviderConfig::default()
        };

        let error = Router::from_gateway_config(&[provider], None)
            .await
            .expect_err("direct factory must validate disabled providers too");
        assert!(matches!(error, RouterError::InvalidConfiguration(_)));
        assert!(error.to_string().contains("requires a configured endpoint"));
    }

    #[tokio::test]
    async fn gateway_snapshot_credential_matching_hashes_once_and_fails_closed() {
        let short_secret = "sk-s";
        let long_secret = "sk-different-length-secret-for-long-provider";
        let providers = [
            credential_test_provider("short", short_secret),
            credential_test_provider("long", long_secret),
        ];
        let router = match Router::from_gateway_config(&providers, None).await {
            Ok(router) => router,
            Err(error) => panic!("credential fixture should build: {error}"),
        };
        let snapshot = router.load_routing_snapshot();
        match snapshot.resolve_legacy_credential("credential-model", short_secret) {
            Ok(deployment_id) => assert_eq!(deployment_id, "short-credential-model"),
            Err(error) => panic!("short credential should match exactly once: {error}"),
        }

        let hash_count = Cell::new(0usize);
        let selected = snapshot.resolve_legacy_credential_with_test_hasher(
            "credential-model",
            long_secret,
            |raw| {
                hash_count.set(hash_count.get() + 1);
                CredentialDigest::from_credential(raw)
            },
        );
        assert_eq!(hash_count.get(), 1);
        match selected {
            Ok(deployment_id) => assert_eq!(deployment_id, "long-credential-model"),
            Err(error) => panic!("long credential should match exactly once: {error}"),
        }
        let no_match_hash_count = Cell::new(0usize);
        assert!(matches!(
            snapshot.resolve_legacy_credential_with_test_hasher(
                "credential-model",
                "no-match",
                |raw| {
                    no_match_hash_count.set(no_match_hash_count.get() + 1);
                    CredentialDigest::from_credential(raw)
                },
            ),
            Err(ProviderError::ModelNotFound { .. })
        ));
        assert_eq!(no_match_hash_count.get(), 1);

        let replacement = snapshot
            .deployments
            .values()
            .map(|deployment| deployment.as_ref().clone())
            .collect();
        router.set_model_list(replacement);
        assert!(matches!(
            router
                .load_routing_snapshot()
                .resolve_legacy_credential("credential-model", short_secret),
            Err(ProviderError::ModelNotFound { .. })
        ));

        let duplicates = [
            credential_test_provider("duplicate-a", "sk-duplicate-secret"),
            credential_test_provider("duplicate-b", "sk-duplicate-secret"),
        ];
        let duplicate_router = match Router::from_gateway_config(&duplicates, None).await {
            Ok(router) => router,
            Err(error) => panic!("duplicate credential fixture should build: {error}"),
        };
        let duplicate_hash_count = Cell::new(0usize);
        assert!(matches!(
            duplicate_router
                .load_routing_snapshot()
                .resolve_legacy_credential_with_test_hasher(
                    "credential-model",
                    "sk-duplicate-secret",
                    |raw| {
                        duplicate_hash_count.set(duplicate_hash_count.get() + 1);
                        CredentialDigest::from_credential(raw)
                    },
                ),
            Err(ProviderError::Configuration { .. })
        ));
        assert_eq!(duplicate_hash_count.get(), 1);

        let metadata_debug = format!(
            "{:?}",
            LegacySelectorMetadata::from_stored_credential(short_secret)
        );
        assert!(!metadata_debug.contains(short_secret));
        assert!(metadata_debug.contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn unproven_gateway_credentials_do_not_publish_selector_metadata() {
        let mut settings_only = credential_test_provider("settings-only", "");
        settings_only
            .settings
            .insert("api_key".to_string(), serde_json::json!("sk-settings-only"));
        let settings_router = match Router::from_gateway_config(&[settings_only], None).await {
            Ok(router) => router,
            Err(error) => panic!("settings-backed provider should build: {error}"),
        };
        assert!(matches!(
            settings_router
                .load_routing_snapshot()
                .resolve_legacy_credential("credential-model", "sk-settings-only"),
            Err(ProviderError::ModelNotFound { .. })
        ));

        for (setting_key, authorization_header) in [
            ("headers", "Authorization"),
            ("custom_headers", "aUtHoRiZaTiOn"),
        ] {
            let mut custom_authorization = credential_test_provider(setting_key, "sk-top-level");
            custom_authorization.settings.insert(
                setting_key.to_string(),
                serde_json::json!({authorization_header: "Bearer sk-custom-header"}),
            );
            let router = match Router::from_gateway_config(&[custom_authorization], None).await {
                Ok(router) => router,
                Err(error) => panic!("custom Authorization provider should build: {error}"),
            };
            assert!(matches!(
                router
                    .load_routing_snapshot()
                    .resolve_legacy_credential("credential-model", "sk-top-level"),
                Err(ProviderError::ModelNotFound { .. })
            ));
        }

        let mut cloudflare = credential_test_provider("cloudflare-conflict", "top-level-key");
        cloudflare.provider_type = "cloudflare".to_string();
        cloudflare.organization = Some("test-account".to_string());
        cloudflare
            .settings
            .insert("api_token".to_string(), serde_json::json!("settings-token"));
        let cloudflare_router = match Router::from_gateway_config(&[cloudflare], None).await {
            Ok(router) => router,
            Err(error) => panic!("Cloudflare conflict fixture should build: {error}"),
        };
        for unproven_credential in ["top-level-key", "settings-token"] {
            assert!(matches!(
                cloudflare_router
                    .load_routing_snapshot()
                    .resolve_legacy_credential("credential-model", unproven_credential),
                Err(ProviderError::ModelNotFound { .. })
            ));
        }
    }
}
