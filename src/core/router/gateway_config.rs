//! Gateway configuration integration
//!
//! This module contains the from_gateway_config method for creating
//! a Router from gateway configuration.

#[cfg(test)]
#[path = "gateway_config_tests.rs"]
mod gateway_config_tests;

#[cfg(test)]
#[path = "gateway_alias_tests.rs"]
mod gateway_alias_tests;

use super::config::RouterConfig;
use super::deployment::{
    Deployment, DeploymentConfig, HealthCheckPolicy, LegacySelectorMetadata,
    ProviderInstanceIdentity, RetrySchedule,
};
use super::error::RouterError;
use super::gateway_aliases::normalize_model_aliases;
use super::unified::Router;
use crate::config::Validate;
use crate::config::models::gateway::GatewayConfig;
use crate::config::models::provider::ProviderConfig;
use crate::config::models::router::GatewayRouterConfig;
use crate::core::providers::provider_type::ProviderType;
use crate::core::providers::registry::{
    self as provider_registry, ProviderDispatchKind, catalog_policy,
};
use crate::core::providers::{Provider, create_provider};
use std::collections::{HashMap, HashSet};

/// Construction-only handoff. Raw credentials never enter a deployment or snapshot.
struct NormalizedProviderConstruction {
    config: ProviderConfig,
    legacy_metadata: Option<LegacySelectorMetadata>,
}

fn non_blank(value: &str) -> Option<&str> {
    (!value.trim().is_empty()).then_some(value)
}

fn setting(config: &ProviderConfig, key: &str) -> Option<String> {
    config
        .settings
        .get(key)
        .and_then(serde_json::Value::as_str)
        .and_then(non_blank)
        .map(str::to_owned)
}

fn environment(keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| std::env::var(key).ok())
        .find_map(|value| non_blank(&value).map(str::to_owned))
}

fn has_custom_authorization(config: &ProviderConfig) -> bool {
    ["headers", "custom_headers"].into_iter().any(|key| {
        config
            .settings
            .get(key)
            .and_then(serde_json::Value::as_object)
            .is_some_and(|headers| {
                headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("authorization"))
            })
    })
}

fn catalog_definition(selector: &str) -> Option<&'static provider_registry::ProviderDefinition> {
    let normalized = selector.trim().to_ascii_lowercase();
    match provider_registry::entry_for_name(&normalized) {
        Some(entry) if entry.dispatch_kind == ProviderDispatchKind::CatalogOpenAiLike => {
            provider_registry::get_definition(entry.canonical_name)
        }
        Some(_) => None,
        None => provider_registry::get_definition(&normalized),
    }
}

fn normalize_provider_construction(config: &ProviderConfig) -> NormalizedProviderConstruction {
    let mut normalized = config.clone();
    let selector = if config.provider_type.trim().is_empty() {
        config.name.trim()
    } else {
        config.provider_type.trim()
    };
    let top_level = non_blank(&config.api_key).map(str::to_owned);

    let effective_credential = if let Some(definition) = catalog_definition(selector) {
        top_level.or_else(|| {
            environment(
                &std::iter::once(definition.auth_env_var)
                    .chain(definition.alternate_auth_env_vars.iter().copied())
                    .collect::<Vec<_>>(),
            )
        })
    } else {
        match selector.parse::<ProviderType>() {
            Ok(ProviderType::Cloudflare) => setting(config, "api_token")
                .or(top_level)
                .or_else(|| setting(config, "api_key"))
                .or_else(|| environment(&["CLOUDFLARE_API_TOKEN"])),
            Ok(ProviderType::Replicate) => top_level
                .or_else(|| setting(config, "api_key"))
                .or_else(|| setting(config, "api_token"))
                .or_else(|| environment(&["REPLICATE_API_TOKEN", "REPLICATE_API_KEY"])),
            Ok(ProviderType::FalAI) => top_level
                .or_else(|| setting(config, "api_key"))
                .or_else(|| environment(&["FAL_AI_API_KEY"])),
            Ok(ProviderType::Cohere) => top_level
                .or_else(|| setting(config, "api_key"))
                .or_else(|| environment(&["COHERE_API_KEY"])),
            Ok(ProviderType::Gemini) => top_level
                .or_else(|| setting(config, "api_key"))
                .or_else(|| setting(config, "google_api_key"))
                .or_else(|| setting(config, "gemini_api_key"))
                .or_else(|| environment(&["GEMINI_API_KEY", "GOOGLE_API_KEY"])),
            Ok(ProviderType::Bedrock) | Err(_) => None,
            Ok(_) => top_level.or_else(|| setting(config, "api_key")),
        }
    };

    if let Some(credential) = effective_credential.as_ref() {
        normalized.api_key = credential.clone();
        for key in ["api_key", "google_api_key", "gemini_api_key"] {
            normalized.settings.remove(key);
        }
        if matches!(
            selector.parse::<ProviderType>(),
            Ok(ProviderType::Cloudflare)
        ) {
            normalized.api_key.clear();
            normalized
                .settings
                .insert("api_token".to_string(), credential.clone().into());
        } else {
            normalized.settings.remove("api_token");
        }
    } else if config.api_key.trim().is_empty() {
        normalized.api_key.clear();
    }

    let legacy_metadata = (!has_custom_authorization(config))
        .then_some(effective_credential.as_deref())
        .flatten()
        .map(LegacySelectorMetadata::from_stored_credential);
    NormalizedProviderConstruction {
        config: normalized,
        legacy_metadata,
    }
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
        let model_aliases = HashMap::new();
        Self::from_gateway_config_with_aliases(providers, router_config, &model_aliases).await
    }

    /// Create a Router from gateway configuration with validated public aliases.
    pub async fn from_gateway_config_with_aliases(
        providers: &[ProviderConfig],
        router_config: Option<RouterConfig>,
        model_aliases: &HashMap<String, String>,
    ) -> Result<Self, RouterError> {
        GatewayConfig::validate_model_alias_map(model_aliases)
            .map_err(RouterError::InvalidConfiguration)?;
        let config = router_config.unwrap_or_default();
        let router = Self::new(config);
        let mut staged = Vec::new();
        let mut canonical_models = HashSet::new();
        let mut generated_deployment_ids = HashSet::new();
        let mut effective_model_aliases = model_aliases.clone();
        let mut catalog_model_aliases = HashMap::new();

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

            let construction = normalize_provider_construction(provider_config);
            let normalized_config = construction.config;
            let provider_name = normalized_config.name.clone();
            let configured_models = normalized_config.models.clone();
            let tags = normalized_config.tags.clone();
            let deployment_config = deployment_config_from_provider(&normalized_config)?;
            // Create provider instance via the single canonical factory.
            let provider = create_provider(normalized_config).await.map_err(|e| {
                RouterError::DeploymentNotFound(format!(
                    "Failed to create provider {}: {}",
                    provider_name, e
                ))
            })?;
            let provider_instance_identity = ProviderInstanceIdentity::new();
            let legacy_metadata = construction.legacy_metadata;

            // Determine which models this deployment serves.
            let uses_configured_models = !configured_models.is_empty();
            let mut models: Vec<String> = if uses_configured_models {
                configured_models
            } else {
                provider
                    .list_models()
                    .iter()
                    .map(|m| m.id.clone())
                    .collect()
            };
            catalog_policy::canonicalize_models(provider.name(), &mut models);

            let uses_provider_name_fallback = models.is_empty();
            catalog_policy::extend_model_aliases(
                provider.name(),
                &provider_name,
                uses_configured_models,
                &models,
                &mut catalog_model_aliases,
            );
            let preserves_configured_name_route = !uses_configured_models
                && crate::core::providers::registry::catalog_policy::preserves_configured_name_route(
                    provider.name(),
                );
            if uses_provider_name_fallback || preserves_configured_name_route {
                models.push(provider_name.clone());
            }

            let mut seen_provider_models = HashSet::new();
            for model in models {
                if !seen_provider_models.insert(model.clone()) {
                    continue;
                }
                let is_configured_name_route =
                    preserves_configured_name_route && model == provider_name;
                let deployment_id = if uses_provider_name_fallback || is_configured_name_route {
                    provider_name.clone()
                } else {
                    format!("{}-{}", provider_name, model)
                };
                if !generated_deployment_ids.insert(deployment_id.clone()) {
                    return Err(RouterError::InvalidConfiguration(format!(
                        "duplicate generated deployment ID '{deployment_id}'"
                    )));
                }
                canonical_models.insert(model.clone());
                staged.push((
                    create_deployment_from_config(
                        &deployment_id,
                        provider.clone(),
                        &model,
                        deployment_config.clone(),
                        tags.clone(),
                        provider_instance_identity.clone(),
                    ),
                    legacy_metadata.clone(),
                    provider_name.clone(),
                ));
            }
        }

        for (alias, target) in catalog_model_aliases {
            if !canonical_models.contains(&alias) {
                effective_model_aliases.entry(alias).or_insert(target);
            }
        }

        let normalized_aliases =
            normalize_model_aliases(&effective_model_aliases, &canonical_models)?;
        let mut aliases = normalized_aliases.iter().collect::<Vec<_>>();
        aliases.sort_unstable_by_key(|(alias, _)| *alias);
        router.try_update_routing_snapshot(move |snapshot| {
            for (deployment, legacy_metadata, provider_name) in staged {
                snapshot.insert_gateway_deployment(
                    deployment,
                    legacy_metadata,
                    provider_name.as_str(),
                );
            }
            for (alias, target) in aliases {
                snapshot.add_model_alias(alias, target)?;
            }
            Ok::<(), RouterError>(())
        })?;
        #[cfg(test)]
        gateway_alias_tests::observe_construction(
            gateway_alias_tests::ConstructionEvent::RoutingSnapshotPublication,
        );

        #[cfg(test)]
        gateway_alias_tests::observe_construction(
            gateway_alias_tests::ConstructionEvent::HealthCheckPhaseEntry,
        );
        router.start_configured_health_checks()?;
        Ok(router)
    }

    /// Return the normalized aliases installed in the current router snapshot.
    pub fn model_aliases(&self) -> HashMap<String, String> {
        self.load_routing_snapshot().model_aliases.clone()
    }
}

/// Helper function to create deployment from provider config
fn create_deployment_from_config(
    deployment_id: &str,
    provider: Provider,
    model: &str,
    deployment_config: DeploymentConfig,
    tags: Vec<String>,
    provider_instance_identity: ProviderInstanceIdentity,
) -> Deployment {
    Deployment::new_with_provider_instance(
        deployment_id.to_string(),
        provider,
        model.to_string(),
        model.to_string(),
        provider_instance_identity,
    )
    .with_config(deployment_config)
    .with_tags(tags)
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
        priority: config.priority,
        retry_schedule: Some(RetrySchedule {
            base_delay_ms: config.retry.base_delay,
            max_delay_ms: config.retry.max_delay,
            backoff_multiplier: config.retry.backoff_multiplier,
            jitter_ratio: config.retry.jitter,
        }),
        health_check_policy: Some(HealthCheckPolicy {
            provider_name: config.name.clone(),
            interval_secs: config.health_check.interval,
            failure_threshold: config.health_check.failure_threshold,
            recovery_timeout_secs: config.health_check.recovery_timeout,
            endpoint,
            endpoint_access: config.endpoint_access,
            expected_codes: config.health_check.expected_codes.clone(),
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
    assert_not_impl_any!(
        NormalizedProviderConstruction: std::fmt::Debug, std::fmt::Display, serde::Serialize
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
    fn normalization_preserves_native_precedence_and_alias_identity() {
        let mut native = credential_test_provider("native", "top-level");
        native
            .settings
            .insert("api_key".to_string(), "shadowed".into());
        let normalized = normalize_provider_construction(&native);
        assert!(normalized.legacy_metadata.is_some());
        assert_eq!(normalized.config.api_key, "top-level");
        assert!(!normalized.config.settings.contains_key("api_key"));

        native.api_key = " ".to_string();
        let settings_fallback = normalize_provider_construction(&native);
        assert_eq!(settings_fallback.config.api_key, "shadowed");

        let mut canonical = credential_test_provider("canonical", "alias-key");
        canonical.provider_type = "gemini".to_string();
        let mut alias = canonical.clone();
        alias.provider_type = "google-gemini".to_string();
        let canonical = normalize_provider_construction(&canonical);
        let alias = normalize_provider_construction(&alias);
        assert_eq!(canonical.config.api_key, alias.config.api_key);
        assert_eq!(
            canonical.config.provider_type.parse::<ProviderType>().ok(),
            alias.config.provider_type.parse::<ProviderType>().ok()
        );
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
    fn test_default_provider_health_config_maps_native_probe_policy() {
        let deployment = deployment_config_from_provider(&ProviderConfig::default()).unwrap();
        let policy = deployment
            .health_check_policy
            .expect("default health configuration should enable native probes");

        assert_eq!(policy.interval_secs, 30);
        assert_eq!(policy.failure_threshold, 5);
        assert_eq!(policy.recovery_timeout_secs, 60);
        assert!(policy.endpoint.is_none());
        assert_eq!(policy.expected_codes, vec![200]);
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
    async fn normalized_gateway_credentials_publish_only_effective_digest() {
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
            Ok(deployment) if deployment == "settings-only-credential-model"
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
        let snapshot = cloudflare_router.load_routing_snapshot();
        assert!(matches!(
            snapshot.resolve_legacy_credential("credential-model", "top-level-key"),
            Err(ProviderError::ModelNotFound { .. })
        ));
        assert!(matches!(
            snapshot.resolve_legacy_credential("credential-model", "settings-token"),
            Ok(deployment) if deployment == "cloudflare-conflict-credential-model"
        ));
    }
}
