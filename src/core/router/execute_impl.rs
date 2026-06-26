//! Execution implementation for Router
//!
//! This module contains the execute, execute_once, and execute_with_retry methods.

use super::deployment::{Deployment, DeploymentId};
use super::error::{CooldownReason, RouterError};
use super::execution::{
    build_execution_result, calculate_retry_delay, infer_cooldown_reason, is_retryable_error,
    provider_error_to_router_error, router_error_to_provider_error,
};
use super::fallback::{ExecutionResult, FallbackType};
use super::unified::Router;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::model::ProviderCapability;
use std::sync::Arc;

impl Router {
    /// Execute a request for a single model with retry logic
    ///
    /// Attempts to execute the operation with retry on transient failures.
    pub async fn execute_with_selected_deployment_retry<T, F, Fut>(
        &self,
        model_name: &str,
        operation: F,
    ) -> Result<(T, DeploymentId, u32, u64), (ProviderError, u32)>
    where
        F: Fn(Arc<Deployment>) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        self.execute_with_retry_inner(model_name, operation)
            .await
            .map(
                |(value, deployment_id, _model_used, attempts, latency_us)| {
                    (value, deployment_id, attempts, latency_us)
                },
            )
    }

    /// Execute a request for a single model with retry logic.
    ///
    /// Prefer `execute_with_selected_deployment_retry`; ID-only callbacks can
    /// re-read a newer snapshot with the same deployment ID.
    #[deprecated(
        since = "0.5.0",
        note = "Use execute_with_selected_deployment_retry so callbacks receive the selected snapshot deployment"
    )]
    pub async fn execute_with_retry<T, F, Fut>(
        &self,
        model_name: &str,
        operation: F,
    ) -> Result<(T, DeploymentId, u32, u64), (ProviderError, u32)>
    where
        F: Fn(DeploymentId) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        self.execute_with_selected_deployment_retry(model_name, move |deployment| {
            operation(deployment.id.clone())
        })
        .await
    }

    async fn execute_with_retry_inner<T, F, Fut>(
        &self,
        model_name: &str,
        operation: F,
    ) -> Result<(T, DeploymentId, String, u32, u64), (ProviderError, u32)>
    where
        F: Fn(Arc<Deployment>) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        let max_attempts = self.config.num_retries + 1;
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            let start = std::time::Instant::now();

            // Try to select a deployment
            let deployment_lease = match self.select_deployment_lease(model_name) {
                Ok(lease) => lease,
                Err(router_err) => {
                    let provider_err = router_error_to_provider_error(router_err);

                    if is_retryable_error(&provider_err) && attempt < max_attempts {
                        let delay = calculate_retry_delay(&self.config, attempt);
                        last_error = Some(provider_err);
                        tokio::time::sleep(delay).await;
                        continue;
                    } else {
                        return Err((provider_err, attempt));
                    }
                }
            };
            let selected_deployment = deployment_lease.clone_deployment();
            let deployment_id = selected_deployment.id.clone();

            // Execute the operation
            let result = operation(selected_deployment.clone()).await;

            let latency_us = start.elapsed().as_micros() as u64;

            match result {
                Ok((value, tokens_used)) => {
                    let model_used = selected_deployment.model.clone();
                    self.record_success_for_deployment(
                        deployment_lease.deployment(),
                        tokens_used,
                        latency_us,
                    );
                    drop(deployment_lease);
                    return Ok((value, deployment_id, model_used, attempt, latency_us));
                }
                Err(err) => {
                    if is_retryable_error(&err) && attempt < max_attempts {
                        // Use ConsecutiveFailures so the deployment only enters
                        // cooldown after exceeding allowed_fails threshold,
                        // giving retries a chance to succeed.
                        self.record_failure_with_reason_for_deployment(
                            deployment_lease.deployment(),
                            CooldownReason::ConsecutiveFailures,
                        );
                        drop(deployment_lease);
                        let delay = calculate_retry_delay(&self.config, attempt);
                        last_error = Some(err);
                        tokio::time::sleep(delay).await;
                        continue;
                    } else {
                        let cooldown_reason = infer_cooldown_reason(&err);
                        self.record_failure_with_reason_for_deployment(
                            deployment_lease.deployment(),
                            cooldown_reason,
                        );
                        drop(deployment_lease);
                        return Err((err, attempt));
                    }
                }
            }
        }

        Err((
            last_error.unwrap_or_else(|| ProviderError::Other {
                provider: "router",
                message: "Unknown error during retry".to_string(),
            }),
            max_attempts,
        ))
    }

    /// Execute a request for a single model with retry logic, constrained to
    /// deployments that support the requested provider capability.
    pub async fn execute_with_selected_deployment_capability_retry<T, F, Fut>(
        &self,
        model_name: &str,
        capability: &ProviderCapability,
        operation: F,
    ) -> Result<(T, DeploymentId, u32, u64), (ProviderError, u32)>
    where
        F: Fn(Arc<Deployment>) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        let max_attempts = self.config.num_retries + 1;
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            let start = std::time::Instant::now();

            let deployment_lease =
                match self.select_deployment_lease_for_capability(model_name, capability) {
                    Ok(lease) => lease,
                    Err(router_err) => {
                        let provider_err = router_error_to_provider_error(router_err);

                        if is_retryable_error(&provider_err) && attempt < max_attempts {
                            let delay = calculate_retry_delay(&self.config, attempt);
                            last_error = Some(provider_err);
                            tokio::time::sleep(delay).await;
                            continue;
                        } else {
                            return Err((provider_err, attempt));
                        }
                    }
                };
            let selected_deployment = deployment_lease.clone_deployment();
            let deployment_id = selected_deployment.id.clone();

            let result = operation(selected_deployment.clone()).await;
            let latency_us = start.elapsed().as_micros() as u64;

            match result {
                Ok((value, tokens_used)) => {
                    self.record_success_for_deployment(
                        deployment_lease.deployment(),
                        tokens_used,
                        latency_us,
                    );
                    drop(deployment_lease);
                    return Ok((value, deployment_id, attempt, latency_us));
                }
                Err(err) => {
                    if is_retryable_error(&err) && attempt < max_attempts {
                        self.record_failure_with_reason_for_deployment(
                            deployment_lease.deployment(),
                            CooldownReason::ConsecutiveFailures,
                        );
                        drop(deployment_lease);
                        let delay = calculate_retry_delay(&self.config, attempt);
                        last_error = Some(err);
                        tokio::time::sleep(delay).await;
                        continue;
                    } else {
                        let cooldown_reason = infer_cooldown_reason(&err);
                        self.record_failure_with_reason_for_deployment(
                            deployment_lease.deployment(),
                            cooldown_reason,
                        );
                        drop(deployment_lease);
                        return Err((err, attempt));
                    }
                }
            }
        }

        Err((
            last_error.unwrap_or_else(|| ProviderError::Other {
                provider: "router",
                message: "Unknown error during capability retry".to_string(),
            }),
            max_attempts,
        ))
    }

    /// Execute a request for a single model with retry logic, constrained to
    /// deployments that support the requested provider capability.
    ///
    /// Prefer `execute_with_selected_deployment_capability_retry`; ID-only
    /// callbacks can re-read a newer snapshot with the same deployment ID.
    #[deprecated(
        since = "0.5.0",
        note = "Use execute_with_selected_deployment_capability_retry so callbacks receive the selected snapshot deployment"
    )]
    pub async fn execute_with_capability_retry<T, F, Fut>(
        &self,
        model_name: &str,
        capability: &ProviderCapability,
        operation: F,
    ) -> Result<(T, DeploymentId, u32, u64), (ProviderError, u32)>
    where
        F: Fn(DeploymentId) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        self.execute_with_selected_deployment_capability_retry(
            model_name,
            capability,
            move |deployment| operation(deployment.id.clone()),
        )
        .await
    }

    /// Execute a request with full retry and fallback support
    ///
    /// This is the main execution method that implements the complete flow:
    /// 1. Try the original model with retries
    /// 2. On failure, try fallback models with retries
    /// 3. Respect max_fallbacks limit
    pub async fn execute_with_selected_deployment<T, F, Fut>(
        &self,
        model_name: &str,
        operation: F,
    ) -> Result<ExecutionResult<T>, RouterError>
    where
        F: Fn(Arc<Deployment>) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        let start = std::time::Instant::now();

        // Get all models to try (original + fallbacks), deduplicated to prevent cycles
        let models_to_try = self.get_models_with_fallbacks(model_name, FallbackType::General);
        let max_models = 1 + self.config.max_fallbacks as usize;
        let mut seen = std::collections::HashSet::new();
        let models_to_try: Vec<_> = models_to_try
            .into_iter()
            .filter(|m| seen.insert(m.clone()))
            .take(max_models)
            .collect();

        let mut last_error = None;
        let mut total_attempts = 0;

        for (model_idx, model) in models_to_try.iter().enumerate() {
            let is_fallback = model_idx > 0;

            if is_fallback {
                self.fallback_triggered_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                tracing::info!(
                    original_model = %model_name,
                    fallback_model = %model,
                    fallback_index = model_idx,
                    error_type = %last_error.as_ref().map_or("unknown".to_string(), |e| format!("{e}")),
                    "fallback triggered, trying next model"
                );
            }

            match self
                .execute_with_retry_inner(model, operation.clone())
                .await
            {
                Ok((result, deployment_id, model_used, attempts, _latency_us)) => {
                    total_attempts += attempts;
                    let total_latency_us = start.elapsed().as_micros() as u64;

                    return Ok(build_execution_result(
                        result,
                        deployment_id,
                        total_attempts,
                        model_used,
                        is_fallback,
                        total_latency_us,
                    ));
                }
                Err((err, attempts)) => {
                    total_attempts += attempts;
                    last_error = Some(err);
                }
            }
        }

        if let Some(err) = last_error {
            Err(provider_error_to_router_error(err, model_name))
        } else {
            Err(RouterError::NoAvailableDeployment(model_name.to_string()))
        }
    }

    /// Execute a request with full retry and fallback support.
    ///
    /// Prefer `execute_with_selected_deployment`; ID-only callbacks can re-read
    /// a newer snapshot with the same deployment ID.
    #[deprecated(
        since = "0.5.0",
        note = "Use execute_with_selected_deployment so callbacks receive the selected snapshot deployment"
    )]
    pub async fn execute<T, F, Fut>(
        &self,
        model_name: &str,
        operation: F,
    ) -> Result<ExecutionResult<T>, RouterError>
    where
        F: Fn(DeploymentId) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        self.execute_with_selected_deployment(model_name, move |deployment| {
            operation(deployment.id.clone())
        })
        .await
    }

    /// Execute a request once without retry or fallback
    ///
    /// This is a simplified execution method for testing or scenarios where
    /// retry/fallback is not desired.
    pub async fn execute_once_with_selected_deployment<T, F, Fut>(
        &self,
        model_name: &str,
        operation: F,
    ) -> Result<ExecutionResult<T>, RouterError>
    where
        F: FnOnce(Arc<Deployment>) -> Fut,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        let start = std::time::Instant::now();

        let deployment_lease = self.select_deployment_lease(model_name)?;
        let selected_deployment = deployment_lease.clone_deployment();
        let deployment_id = selected_deployment.id.clone();

        let result = operation(selected_deployment.clone()).await;

        let latency_us = start.elapsed().as_micros() as u64;

        match result {
            Ok((value, tokens_used)) => {
                let model_used = selected_deployment.model.clone();
                self.record_success_for_deployment(
                    deployment_lease.deployment(),
                    tokens_used,
                    latency_us,
                );
                drop(deployment_lease);

                Ok(build_execution_result(
                    value,
                    deployment_id,
                    1,
                    model_used,
                    false,
                    latency_us,
                ))
            }
            Err(err) => {
                let cooldown_reason = infer_cooldown_reason(&err);
                self.record_failure_with_reason_for_deployment(
                    deployment_lease.deployment(),
                    cooldown_reason,
                );
                drop(deployment_lease);

                Err(provider_error_to_router_error(err, model_name))
            }
        }
    }

    /// Execute a request once without retry or fallback.
    ///
    /// Prefer `execute_once_with_selected_deployment`; ID-only callbacks can
    /// re-read a newer snapshot with the same deployment ID.
    #[deprecated(
        since = "0.5.0",
        note = "Use execute_once_with_selected_deployment so callbacks receive the selected snapshot deployment"
    )]
    pub async fn execute_once<T, F, Fut>(
        &self,
        model_name: &str,
        operation: F,
    ) -> Result<ExecutionResult<T>, RouterError>
    where
        F: FnOnce(DeploymentId) -> Fut,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        self.execute_once_with_selected_deployment(model_name, move |deployment| {
            operation(deployment.id.clone())
        })
        .await
    }
}
