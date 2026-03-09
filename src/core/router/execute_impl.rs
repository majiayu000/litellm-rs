//! Execution implementation for Router
//!
//! This module contains the execute, execute_once, and execute_with_retry methods.

use super::deployment::DeploymentId;
use super::error::{CooldownReason, RouterError};
use super::execution::{
    build_execution_result, calculate_retry_delay, infer_cooldown_reason, is_retryable_error,
    provider_error_to_router_error, router_error_to_provider_error,
};
use super::fallback::{ExecutionResult, FallbackType};
use super::unified::Router;
use crate::core::providers::unified_provider::ProviderError;

impl Router {
    /// Execute a request for a single model with retry logic
    ///
    /// Attempts to execute the operation with retry on transient failures.
    pub async fn execute_with_retry<T, F, Fut>(
        &self,
        model_name: &str,
        operation: F,
    ) -> Result<(T, DeploymentId, u32, u64), (ProviderError, u32)>
    where
        F: Fn(DeploymentId) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        let max_attempts = self.config.num_retries + 1;
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            let start = std::time::Instant::now();

            // Try to select a deployment
            let deployment_id = match self.select_deployment(model_name) {
                Ok(id) => id,
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

            // Execute the operation
            let result = operation(deployment_id.clone()).await;

            let latency_us = start.elapsed().as_micros() as u64;

            match result {
                Ok((value, tokens_used)) => {
                    self.release_deployment(&deployment_id);
                    self.record_success(&deployment_id, tokens_used, latency_us);
                    return Ok((value, deployment_id, attempt, latency_us));
                }
                Err(err) => {
                    self.release_deployment(&deployment_id);

                    if is_retryable_error(&err) && attempt < max_attempts {
                        // Use ConsecutiveFailures so the deployment only enters
                        // cooldown after exceeding allowed_fails threshold,
                        // giving retries a chance to succeed.
                        self.record_failure_with_reason(
                            &deployment_id,
                            CooldownReason::ConsecutiveFailures,
                        );
                        let delay = calculate_retry_delay(&self.config, attempt);
                        last_error = Some(err);
                        tokio::time::sleep(delay).await;
                        continue;
                    } else {
                        let cooldown_reason = infer_cooldown_reason(&err);
                        self.record_failure_with_reason(&deployment_id, cooldown_reason);
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

    /// Execute a request with full retry and fallback support
    ///
    /// This is the main execution method that implements the complete flow:
    /// 1. Try the original model with retries
    /// 2. On failure, try fallback models with retries
    /// 3. Respect max_fallbacks limit
    pub async fn execute<T, F, Fut>(
        &self,
        model_name: &str,
        operation: F,
    ) -> Result<ExecutionResult<T>, RouterError>
    where
        F: Fn(DeploymentId) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        let start = std::time::Instant::now();

        // Get all models to try (original + fallbacks)
        let models_to_try = self.get_models_with_fallbacks(model_name, FallbackType::General);

        // Limit fallback attempts
        let max_models = 1 + self.config.max_fallbacks as usize;
        let models_to_try: Vec<_> = models_to_try.into_iter().take(max_models).collect();

        let mut last_error = None;
        let mut total_attempts = 0;

        for (model_idx, model) in models_to_try.iter().enumerate() {
            let is_fallback = model_idx > 0;

            match self.execute_with_retry(model, operation.clone()).await {
                Ok((result, deployment_id, attempts, _latency_us)) => {
                    total_attempts += attempts;
                    let total_latency_us = start.elapsed().as_micros() as u64;

                    let model_used = if let Some(deployment) = self.get_deployment(&deployment_id) {
                        deployment.model.clone()
                    } else {
                        model.clone()
                    };

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

    /// Execute a request once without retry or fallback
    ///
    /// This is a simplified execution method for testing or scenarios where
    /// retry/fallback is not desired.
    pub async fn execute_once<T, F, Fut>(
        &self,
        model_name: &str,
        operation: F,
    ) -> Result<ExecutionResult<T>, RouterError>
    where
        F: FnOnce(DeploymentId) -> Fut,
        Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    {
        let start = std::time::Instant::now();

        let deployment_id = self.select_deployment(model_name)?;

        let result = operation(deployment_id.clone()).await;

        let latency_us = start.elapsed().as_micros() as u64;

        self.release_deployment(&deployment_id);

        match result {
            Ok((value, tokens_used)) => {
                self.record_success(&deployment_id, tokens_used, latency_us);

                let model_used = if let Some(deployment) = self.get_deployment(&deployment_id) {
                    deployment.model.clone()
                } else {
                    model_name.to_string()
                };

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
                self.record_failure_with_reason(&deployment_id, cooldown_reason);

                Err(provider_error_to_router_error(err, model_name))
            }
        }
    }
}
