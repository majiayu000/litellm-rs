use crate::core::providers::{Provider, ProviderError};
use crate::core::router::deployment::Deployment;
use crate::core::router::error::CooldownReason;
use crate::core::router::execution::{infer_cooldown_reason, router_error_to_provider_error};
use crate::core::router::retry_policy::{RetryContext, RetryPolicy};
use crate::core::router::{RouterError, UnifiedRouter};
use crate::core::types::model::ProviderCapability;
use crate::utils::error::gateway_error::GatewayError;
use std::collections::HashSet;
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;
#[path = "execution_observability.rs"]
pub(super) mod observability;
pub(super) struct StreamingDeploymentLease {
    router: Arc<UnifiedRouter>,
    deployment: Arc<Deployment>,
    started_at: Instant,
    finalized: bool,
}

impl StreamingDeploymentLease {
    fn new(router: Arc<UnifiedRouter>, deployment: Arc<Deployment>, started_at: Instant) -> Self {
        Self {
            router,
            deployment,
            started_at,
            finalized: false,
        }
    }

    pub(super) fn finish_success(mut self, tokens_used: u64) {
        let latency_us = self.started_at.elapsed().as_micros() as u64;
        self.router
            .record_success_for_deployment(&self.deployment, tokens_used, latency_us);
        self.release();
    }

    pub(super) fn finish_failure(mut self, error: &ProviderError) {
        // Mid-stream failures cannot be retried. Preserve fail-fast cooldowns
        // for rate limits and deterministic misconfiguration, while routing
        // ordinary transient failures through the counted breaker path.
        let inferred = infer_cooldown_reason(error);
        let cooldown_reason = match inferred {
            CooldownReason::RateLimit | CooldownReason::AuthError | CooldownReason::NotFound => {
                inferred
            }
            _ => CooldownReason::ConsecutiveFailures,
        };
        self.router
            .record_failure_with_reason_for_deployment(&self.deployment, cooldown_reason);
        self.release();
    }

    fn release(&mut self) {
        if !self.finalized {
            UnifiedRouter::release_selected_deployment(&self.deployment);
            self.finalized = true;
        }
    }
}

impl Drop for StreamingDeploymentLease {
    fn drop(&mut self) {
        self.release();
    }
}

pub(super) async fn execute_with_selected_deployment<T, F, Fut>(
    router: &UnifiedRouter,
    requested_model: &str,
    capability: ProviderCapability,
    operation: F,
) -> Result<T, GatewayError>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
{
    let max_attempts = router.config().num_retries + 1;
    let mut attempt = 1;
    // Selection failures control retry timing but must not replace the most
    // recent error returned by a real provider operation.
    let mut last_operation_error = None;
    // Hard exclusions (budget/unpriced policy): never retried in this request.
    let mut excluded_budget_deployments = HashSet::new();
    // Soft exclusions (already tried): avoided while untried candidates remain.
    let mut tried_deployments = HashSet::new();

    while attempt <= max_attempts {
        let started_at = Instant::now();

        // Prefer deployments this request has not already tried; when every
        // candidate was tried once, fall back to the pool minus budget
        // exclusions so single-deployment setups still get same-target
        // retries. When even that pool is empty, fail closed below.
        let deployment_lease = match router.select_deployment_lease_for_capability_matching(
            requested_model,
            &capability,
            |deployment| {
                !excluded_budget_deployments.contains(deployment.id.as_str())
                    && !tried_deployments.contains(deployment.id.as_str())
            },
        ) {
            Ok(lease) => lease,
            Err(RouterError::UnsupportedCapability { .. }) if !tried_deployments.is_empty() => {
                match router.select_deployment_lease_for_capability_matching(
                    requested_model,
                    &capability,
                    |deployment| !excluded_budget_deployments.contains(deployment.id.as_str()),
                ) {
                    Ok(lease) => {
                        // Opening the full pool starts a new sweep. Forget the
                        // previous sweep so a failure here advances to the
                        // other deployments instead of repeatedly selecting
                        // the highest-priority target.
                        tried_deployments.clear();
                        lease
                    }
                    Err(router_err) => {
                        if matches!(&router_err, RouterError::UnsupportedCapability { .. })
                            && let Some(err) = last_operation_error.clone()
                        {
                            return Err(GatewayError::Provider(err));
                        }

                        let provider_err = router_error_to_provider_error(router_err);
                        let retry_decision = RetryPolicy.decide(
                            router.config(),
                            &provider_err,
                            RetryContext::unary(attempt, max_attempts),
                        );
                        if retry_decision.should_retry {
                            attempt += 1;
                            if let Some(delay) = retry_decision.delay {
                                tokio::time::sleep(delay).await;
                            }
                            continue;
                        }

                        return Err(GatewayError::Provider(
                            last_operation_error.unwrap_or(provider_err),
                        ));
                    }
                }
            }
            Err(router_err) => {
                if matches!(&router_err, RouterError::UnsupportedCapability { .. })
                    && let Some(err) = last_operation_error.clone()
                {
                    return Err(GatewayError::Provider(err));
                }

                let provider_err = router_error_to_provider_error(router_err);

                let retry_decision = RetryPolicy.decide(
                    router.config(),
                    &provider_err,
                    RetryContext::unary(attempt, max_attempts),
                );
                if retry_decision.should_retry {
                    attempt += 1;
                    if let Some(delay) = retry_decision.delay {
                        tokio::time::sleep(delay).await;
                    }
                    continue;
                }

                return Err(GatewayError::Provider(
                    last_operation_error.unwrap_or(provider_err),
                ));
            }
        };

        let selected_deployment_id = deployment_lease.clone_deployment_id();
        let provider = deployment_lease.deployment().provider_for_request();
        let selected_model = deployment_lease.deployment().model.clone();

        match operation.clone()(provider, selected_model, selected_deployment_id).await {
            Ok((value, tokens_used)) => {
                let latency_us = started_at.elapsed().as_micros() as u64;
                router.record_success_for_deployment(
                    deployment_lease.deployment(),
                    tokens_used,
                    latency_us,
                );
                drop(deployment_lease);
                return Ok(value);
            }
            Err(err) => {
                if observability::is_budget_or_unpriced_fallback(
                    deployment_lease.deployment(),
                    &err,
                    false,
                ) {
                    excluded_budget_deployments.insert(deployment_lease.clone_deployment_id());
                    drop(deployment_lease);
                    last_operation_error = Some(err);
                    continue;
                }

                let retry_decision = RetryPolicy.decide_for_deployment(
                    router.config(),
                    &deployment_lease.deployment().config,
                    &err,
                    RetryContext::unary(attempt, max_attempts),
                );
                if retry_decision.should_retry {
                    router.record_failure_with_reason_for_deployment(
                        deployment_lease.deployment(),
                        crate::core::router::CooldownReason::ConsecutiveFailures,
                    );
                    // Do not pick this deployment again in this request while
                    // another candidate is available.
                    tried_deployments.insert(deployment_lease.clone_deployment_id());
                    drop(deployment_lease);
                    last_operation_error = Some(err);
                    attempt += 1;
                    if let Some(delay) = retry_decision.delay {
                        tokio::time::sleep(delay).await;
                    }
                    continue;
                }

                let cooldown_reason = infer_cooldown_reason(&err);
                router.record_failure_with_reason_for_deployment(
                    deployment_lease.deployment(),
                    cooldown_reason,
                );
                drop(deployment_lease);
                return Err(GatewayError::Provider(err));
            }
        }
    }

    Err(GatewayError::Provider(last_operation_error.unwrap_or_else(
        || ProviderError::Other {
            provider: "router",
            message: "Unknown error during selected deployment retry".to_string(),
        },
    )))
}

#[cfg(test)]
fn retry_delay_for_error(
    config: &crate::core::router::config::RouterConfig,
    attempt: u32,
    error: &ProviderError,
) -> Option<Duration> {
    if crate::core::router::execution::retryable_budget_scope(error).is_some()
        || super::spend::is_model_not_priced_error(error)
    {
        return None;
    }

    let decision = RetryPolicy.decide(config, error, RetryContext::unary(attempt, attempt + 1));
    if decision.should_retry {
        decision.delay
    } else {
        None
    }
}

#[cfg(test)]
#[path = "execution_retry_delay_tests.rs"]
mod retry_delay_tests;

pub(super) async fn execute_stream_with_selected_deployment<T, F, Fut>(
    router: Arc<UnifiedRouter>,
    requested_model: &str,
    capability: ProviderCapability,
    operation: F,
) -> Result<(T, StreamingDeploymentLease), GatewayError>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
{
    let max_attempts = router.config().num_retries + 1;
    let mut attempt = 1;
    // Selection failures control retry timing but must not replace the most
    // recent error returned by a real provider operation.
    let mut last_operation_error = None;
    // Hard exclusions (budget/unpriced policy): never retried in this request.
    let mut excluded_budget_deployments = HashSet::new();
    // Soft exclusions (already tried): avoided while untried candidates remain.
    let mut tried_deployments = HashSet::new();

    while attempt <= max_attempts {
        let started_at = Instant::now();

        // Prefer deployments this request has not already tried; when every
        // candidate was tried once, fall back to the full pool so
        // single-deployment setups still get same-target retries.
        let deployment_lease = match router.select_deployment_lease_for_capability_matching(
            requested_model,
            &capability,
            |deployment| {
                !excluded_budget_deployments.contains(deployment.id.as_str())
                    && !tried_deployments.contains(deployment.id.as_str())
            },
        ) {
            Ok(lease) => lease,
            Err(RouterError::UnsupportedCapability { .. }) if !tried_deployments.is_empty() => {
                match router.select_deployment_lease_for_capability_matching(
                    requested_model,
                    &capability,
                    |deployment| !excluded_budget_deployments.contains(deployment.id.as_str()),
                ) {
                    Ok(lease) => {
                        // Opening the full pool starts a new sweep. Forget the
                        // previous sweep so a failure here advances to the
                        // other deployments instead of repeatedly selecting
                        // the highest-priority target.
                        tried_deployments.clear();
                        lease
                    }
                    Err(router_err) => {
                        if matches!(&router_err, RouterError::UnsupportedCapability { .. })
                            && let Some(err) = last_operation_error.clone()
                        {
                            return Err(GatewayError::Provider(err));
                        }

                        let provider_err = router_error_to_provider_error(router_err);
                        let retry_decision = RetryPolicy.decide(
                            router.config(),
                            &provider_err,
                            RetryContext::stream_pre_output(attempt, max_attempts),
                        );
                        if retry_decision.should_retry {
                            attempt += 1;
                            if let Some(delay) = retry_decision.delay {
                                tokio::time::sleep(delay).await;
                            }
                            continue;
                        }

                        return Err(GatewayError::Provider(
                            last_operation_error.unwrap_or(provider_err),
                        ));
                    }
                }
            }
            Err(router_err) => {
                if matches!(&router_err, RouterError::UnsupportedCapability { .. })
                    && let Some(err) = last_operation_error.clone()
                {
                    return Err(GatewayError::Provider(err));
                }

                let provider_err = router_error_to_provider_error(router_err);

                let retry_decision = RetryPolicy.decide(
                    router.config(),
                    &provider_err,
                    RetryContext::stream_pre_output(attempt, max_attempts),
                );
                if retry_decision.should_retry {
                    attempt += 1;
                    if let Some(delay) = retry_decision.delay {
                        tokio::time::sleep(delay).await;
                    }
                    continue;
                }

                return Err(GatewayError::Provider(
                    last_operation_error.unwrap_or(provider_err),
                ));
            }
        };
        let deployment = deployment_lease.clone_deployment();
        let selected_deployment_id = deployment_lease.clone_deployment_id();
        let provider = deployment.provider_for_request();
        let selected_model = deployment.model.clone();

        match operation.clone()(provider, selected_model, selected_deployment_id).await {
            Ok(stream) => {
                let _deployment_id = deployment_lease.into_deployment_id();
                let lease = StreamingDeploymentLease::new(router.clone(), deployment, started_at);
                return Ok((stream, lease));
            }
            Err(err) => {
                if observability::is_budget_or_unpriced_fallback(
                    deployment_lease.deployment(),
                    &err,
                    true,
                ) {
                    excluded_budget_deployments.insert(deployment_lease.clone_deployment_id());
                    drop(deployment_lease);
                    last_operation_error = Some(err);
                    continue;
                }

                let retry_decision = RetryPolicy.decide_for_deployment(
                    router.config(),
                    &deployment_lease.deployment().config,
                    &err,
                    RetryContext::stream_pre_output(attempt, max_attempts),
                );
                if retry_decision.should_retry {
                    router.record_failure_with_reason_for_deployment(
                        deployment_lease.deployment(),
                        crate::core::router::CooldownReason::ConsecutiveFailures,
                    );
                    // Do not pick this deployment again in this request while
                    // another candidate is available.
                    tried_deployments.insert(deployment_lease.clone_deployment_id());
                    drop(deployment_lease);
                    last_operation_error = Some(err);
                    attempt += 1;
                    if let Some(delay) = retry_decision.delay {
                        tokio::time::sleep(delay).await;
                    }
                    continue;
                }

                let cooldown_reason = infer_cooldown_reason(&err);
                router.record_failure_with_reason_for_deployment(
                    deployment_lease.deployment(),
                    cooldown_reason,
                );
                drop(deployment_lease);
                return Err(GatewayError::Provider(err));
            }
        }
    }

    Err(GatewayError::Provider(last_operation_error.unwrap_or_else(
        || ProviderError::Other {
            provider: "router",
            message: "Unknown error during streaming retry".to_string(),
        },
    )))
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "execution_failover_tests.rs"]
mod failover_tests;

#[cfg(test)]
#[path = "execution_exclusion_tests.rs"]
mod exclusion_tests;
