//! Gateway configuration integration
//!
//! This module contains the from_gateway_config method for creating
//! a Router from gateway configuration.

use super::config::RouterConfig;
use super::deployment::{Deployment, DeploymentConfig, HealthCheckPolicy, RetrySchedule};
use super::error::RouterError;
use super::unified::Router;
use crate::config::Validate;
use crate::config::models::provider::ProviderConfig;
use crate::config::models::router::GatewayRouterConfig;
use crate::core::providers::{Provider, create_provider};

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
                router.add_deployment(deployment);
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
                    router.add_deployment(deployment);
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
}
