use crate::core::providers::{Provider, ProviderError};
use crate::core::router::admission::{AdmissionBackend, AdmissionHold};
use crate::core::router::deployment::Deployment;
use crate::core::router::error::CooldownReason;
use crate::core::router::execution::{infer_cooldown_reason, router_error_to_provider_error};
use crate::core::router::retry_policy::{RequestIdempotency, RetryContext, RetryPolicy};
use crate::core::router::{RouterError, UnifiedRouter};
use crate::core::types::model::ProviderCapability;
use crate::utils::error::gateway_error::GatewayError;
use std::collections::HashSet;
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;
#[path = "execution_completion.rs"]
pub(super) mod completion;
#[path = "execution_observability.rs"]
pub(super) mod observability;
pub(super) struct StreamingDeploymentLease {
    router: Arc<UnifiedRouter>,
    deployment: Arc<Deployment>,
    started_at: Instant,
    finalized: bool,
    admission: AdmissionBackend,
    hold: Option<AdmissionHold>,
}

/// Cancellation ends the selected response even when the caller retains its
/// lease and later retries a terminal method through the same &mut reference.
struct TerminalSettlementGuard<'a> {
    lease: &'a mut StreamingDeploymentLease,
    completion: Arc<completion::UnaryCompletion>,
    finished: bool,
}

impl TerminalSettlementGuard<'_> {
    fn finish(mut self) {
        self.completion.disarm();
        self.finished = true;
    }
}

impl Drop for TerminalSettlementGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.completion.record_cancelled();
            self.lease.release();
        }
    }
}

impl StreamingDeploymentLease {
    fn new(
        router: Arc<UnifiedRouter>,
        deployment: Arc<Deployment>,
        started_at: Instant,
        admission: AdmissionBackend,
        hold: Option<AdmissionHold>,
    ) -> Self {
        Self {
            router,
            deployment,
            started_at,
            finalized: false,
            admission,
            hold,
        }
    }

    /// Preserve a known terminal outcome only while accounting is pending.
    /// Normal completion stays owned by finish_success/finish_failure; dropping
    /// this wait retains the original lease's outcome and admission usage.
    pub(super) fn settle_terminal<'a, F: std::future::Future + 'a>(
        &'a mut self,
        tokens: u64,
        error: Option<&'a ProviderError>,
        settlement: F,
    ) -> impl std::future::Future<Output = F::Output> + 'a {
        let outcome = match error {
            Some(error) => self.failure_outcome(error, false),
            None => completion::TerminalOutcome::Success,
        };
        self.settle_with_outcome(tokens, outcome, settlement)
    }

    pub(super) fn settle_interrupted<'a, F: std::future::Future + 'a>(
        &'a mut self,
        tokens: u64,
        error: Option<&'a ProviderError>,
        settlement: F,
    ) -> impl std::future::Future<Output = F::Output> + 'a {
        let outcome = match error {
            Some(error) => self.failure_outcome(error, true),
            None => completion::TerminalOutcome::Interrupted,
        };
        self.settle_with_outcome(tokens, outcome, settlement)
    }

    fn failure_outcome(
        &self,
        error: &ProviderError,
        interrupted: bool,
    ) -> completion::TerminalOutcome {
        let reason = match infer_cooldown_reason(error) {
            reason @ (CooldownReason::RateLimit
            | CooldownReason::AuthError
            | CooldownReason::NotFound) => reason,
            _ => CooldownReason::ConsecutiveFailures,
        };
        completion::TerminalOutcome::Failure(self.router.clone(), reason, interrupted)
    }

    fn settle_with_outcome<'a, F: std::future::Future + 'a>(
        &'a mut self,
        tokens: u64,
        outcome: completion::TerminalOutcome,
        settlement: F,
    ) -> impl std::future::Future<Output = F::Output> + 'a {
        // Box before constructing any wrapper future. Async entry points would
        // still capture the entire F before their first poll, inflating every
        // enclosing route and stream future even for an unselected branch.
        let settlement = Box::pin(settlement);
        async move {
            if self.finalized {
                return settlement.await;
            }
            let completion = completion::UnaryCompletion::terminal(
                self.deployment.clone(),
                self.hold.clone(),
                self.started_at,
                tokens,
                outcome,
            );
            let guard = TerminalSettlementGuard {
                lease: self,
                completion,
                finished: false,
            };
            let result = completion::CURRENT
                .scope(guard.completion.clone(), settlement)
                .await;
            guard.finish();
            result
        }
    }

    /// Native streams have completed or accepted upstream work even when their
    /// final usage is unavailable. Preserve that distinction through accounting
    /// cancellation and finish the exact lease on both normal and dropped waits.
    pub(super) fn settle_native_stream<'a, F: std::future::Future + 'a>(
        &'a mut self,
        usage: Option<u64>,
        terminal: bool,
        error: Option<&'a ProviderError>,
        settlement: F,
    ) -> impl std::future::Future<Output = F::Output> + 'a {
        let outcome = match error {
            Some(error) => self.failure_outcome(error, true),
            None if terminal => completion::TerminalOutcome::Success,
            None => completion::TerminalOutcome::Interrupted,
        };
        let settlement = Box::pin(settlement);
        async move {
            if self.finalized {
                return settlement.await;
            }
            let completion = completion::UnaryCompletion::native_terminal(
                self.deployment.clone(),
                self.hold.clone(),
                self.started_at,
                usage,
                outcome.clone(),
            );
            let guard = TerminalSettlementGuard {
                lease: self,
                completion,
                finished: false,
            };
            let result = completion::CURRENT
                .scope(guard.completion.clone(), settlement)
                .await;
            // The same one-shot recorder handles a cancelled accounting wait.
            // Retention and actual metrics are replaced under the minute gate.
            guard.completion.record_cancelled();
            let hold = guard.lease.hold.take();
            guard.lease.release();
            guard.finish();
            // Local outcome and release precede shared I/O. The owned hold's
            // prepared cleanup survives cancellation during these awaits.
            match outcome {
                completion::TerminalOutcome::Success => {
                    self.router
                        .record_success_circuit_for_deployment_async(&self.deployment)
                        .await;
                }
                completion::TerminalOutcome::Failure(router, reason, _) => {
                    router
                        .record_failure_circuit_for_deployment_async(&self.deployment, reason)
                        .await;
                }
                completion::TerminalOutcome::Interrupted => {}
            }
            if let Some(hold) = hold {
                match usage {
                    Some(tokens) => self.admission.settle_async(&hold, tokens).await,
                    None => self.admission.retain_async(&hold, 0).await,
                }
            }
            result
        }
    }

    pub(super) async fn finish_success(mut self, tokens_used: u64) {
        self.complete_response(tokens_used, None).await;
    }

    pub(super) async fn complete_response(
        &mut self,
        tokens_used: u64,
        error: Option<&ProviderError>,
    ) {
        if self.finalized {
            return;
        }
        if let Some(error) = error {
            self.complete_failure(error, tokens_used, false).await;
            return;
        }
        let latency_us = self.started_at.elapsed().as_micros() as u64;
        let hold = self.hold.take();
        if let Some(hold) = &hold {
            hold.prepare_settlement(tokens_used);
        }
        self.deployment
            .record_success_with_admission(tokens_used, latency_us, hold.as_ref());
        // Mark the local outcome before yielding. A cancelled completion may
        // be retried by a caller holding &mut self; it must never count twice.
        self.release();
        self.router
            .record_success_circuit_for_deployment_async(&self.deployment)
            .await;
        if let Some(hold) = hold {
            self.admission.settle_async(&hold, tokens_used).await;
        }
    }

    pub(super) async fn finish_failure(self, error: &ProviderError) {
        self.finish_failure_with_tokens(error, 0).await;
    }

    pub(super) async fn finish_failure_with_tokens(
        mut self,
        error: &ProviderError,
        tokens_used: u64,
    ) {
        self.complete_failure(error, tokens_used, false).await;
    }

    async fn complete_failure(
        &mut self,
        error: &ProviderError,
        tokens_used: u64,
        interrupted: bool,
    ) {
        if self.finalized {
            return;
        }
        let inferred = infer_cooldown_reason(error);
        let cooldown_reason = match inferred {
            CooldownReason::RateLimit | CooldownReason::AuthError | CooldownReason::NotFound => {
                inferred
            }
            _ => CooldownReason::ConsecutiveFailures,
        };
        let retain_admission = completion::retain_failure_admission(tokens_used, interrupted);
        let hold = self.hold.take();
        if let Some(hold) = &hold {
            if retain_admission {
                hold.prepare_settlement(tokens_used);
            } else {
                hold.prepare_cancellation();
            }
        }
        if interrupted {
            self.deployment
                .record_interrupted_usage_with_admission(tokens_used, hold.as_ref());
        } else {
            self.deployment
                .record_partial_tokens_with_admission(tokens_used, hold.as_ref());
        }
        self.router
            .record_local_failure(&self.deployment, cooldown_reason);
        self.release();
        self.router
            .record_failure_circuit_for_deployment_async(&self.deployment, cooldown_reason)
            .await;
        if let Some(hold) = hold {
            if retain_admission {
                self.admission.settle_async(&hold, tokens_used).await;
            } else {
                self.admission.cancel_async(&hold).await;
            }
        }
    }

    #[cfg(feature = "websockets")]
    pub(super) async fn record_provider_event_failure(&mut self, error: &ProviderError) {
        let inferred = infer_cooldown_reason(error);
        let reason = match inferred {
            CooldownReason::RateLimit | CooldownReason::AuthError | CooldownReason::NotFound => {
                inferred
            }
            _ => CooldownReason::ConsecutiveFailures,
        };
        self.router.record_local_failure(&self.deployment, reason);
        self.router
            .record_failure_circuit_for_deployment_async(&self.deployment, reason)
            .await;
    }

    #[cfg(feature = "websockets")]
    pub(super) async fn refresh_realtime_deployment(
        &mut self,
        router: Arc<UnifiedRouter>,
    ) -> Result<(), ProviderError> {
        let deployment = router.get_deployment(&self.deployment.id).ok_or_else(|| {
            ProviderError::configuration(
                "openai",
                "Realtime deployment was removed or disabled; reconnect",
            )
        })?;
        if Arc::ptr_eq(&router, &self.router) && Arc::ptr_eq(&deployment, &self.deployment) {
            return Ok(());
        }
        let changed = || {
            ProviderError::configuration(
                "openai",
                "Realtime deployment configuration changed; reconnect",
            )
        };
        let (Provider::OpenAI(previous), Provider::OpenAI(current)) =
            (&self.deployment.provider, &deployment.provider)
        else {
            return Err(changed());
        };
        if deployment.model != self.deployment.model
            || deployment.model_name != self.deployment.model_name
            || serde_json::to_value(&previous.config).map_err(|_| changed())?
                != serde_json::to_value(&current.config).map_err(|_| changed())?
        {
            return Err(changed());
        }
        // Transport/account/model are unchanged. Admission and health now belong
        // to the live router, including its current RPM/TPM/parallel policy.
        self.cancel_admission().await;
        self.release();
        self.router = router;
        self.deployment = deployment;
        Ok(())
    }

    #[cfg(feature = "websockets")]
    pub(super) async fn begin_response(
        &mut self,
        estimated_tokens: u64,
    ) -> Result<(), ProviderError> {
        // The handshake and each generation are separate admission boundaries.
        // Idle sockets do not hold a generation's parallel-request slot.
        self.cancel_admission().await;
        self.release();
        let mut selected = self
            .router
            .select_pinned_response_lease_async(&self.deployment, estimated_tokens)
            .await
            .map_err(router_error_to_provider_error)?;
        (self.admission, self.hold) = selected.take_admission();
        let _ = selected.into_deployment_id();
        self.started_at = Instant::now();
        self.finalized = false;
        Ok(())
    }

    #[cfg(feature = "websockets")]
    pub(super) async fn finish_neutral(&mut self, tokens_used: u64) {
        if self.finalized {
            return;
        }
        let hold = self.hold.take();
        if let Some(hold) = &hold {
            hold.prepare_settlement(tokens_used);
        }
        self.deployment
            .record_partial_tokens_with_admission(tokens_used, hold.as_ref());
        self.release();
        if let Some(hold) = hold {
            self.admission.settle_async(&hold, tokens_used).await;
        }
    }

    #[cfg(feature = "websockets")]
    pub(super) async fn cancel_response(&mut self) {
        self.cancel_admission().await;
        self.release();
    }

    #[cfg(feature = "websockets")]
    pub(super) async fn finish_interrupted(
        &mut self,
        tokens_used: u64,
        error: Option<&ProviderError>,
    ) {
        if self.finalized {
            return;
        }
        if let Some(error) = error {
            // complete_failure owns tokens and the known failure before I/O.
            self.complete_failure(error, tokens_used, true).await;
        } else {
            let hold = self.hold.take();
            if let Some(hold) = &hold {
                hold.prepare_settlement(tokens_used);
            }
            self.deployment
                .record_interrupted_usage_with_admission(tokens_used, hold.as_ref());
            self.release();
            if let Some(hold) = hold {
                self.admission.settle_async(&hold, tokens_used).await;
            }
        }
    }

    pub(super) fn deployment_id(&self) -> &str {
        self.deployment.id.as_str()
    }

    fn release(&mut self) {
        if !self.finalized {
            UnifiedRouter::release_selected_deployment(&self.deployment);
            // The hold's RAII cleanup uses the bounded background queue. Drop
            // must never wait for Redis on an HTTP worker.
            self.hold.take();
            self.finalized = true;
        }
    }

    #[cfg(feature = "websockets")]
    async fn cancel_admission(&mut self) {
        if let Some(hold) = self.hold.take() {
            self.admission.cancel_async(&hold).await;
        }
    }
}

impl Drop for StreamingDeploymentLease {
    fn drop(&mut self) {
        self.release();
    }
}

pub(super) fn settle_stream_terminal<'a, F: std::future::Future + 'a>(
    lease: Option<&'a mut StreamingDeploymentLease>,
    tokens: u64,
    error: Option<&'a ProviderError>,
    settlement: F,
) -> impl std::future::Future<Output = F::Output> + 'a {
    match lease {
        Some(lease) => {
            futures::future::Either::Left(lease.settle_terminal(tokens, error, settlement))
        }
        None => futures::future::Either::Right(Box::pin(settlement)),
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
    execute_with_selected_deployment_matching(
        router,
        requested_model,
        capability,
        |_| true,
        operation,
    )
    .await
}

pub(super) async fn execute_with_selected_deployment_matching<T, F, Fut, P>(
    router: &UnifiedRouter,
    requested_model: &str,
    capability: ProviderCapability,
    is_candidate: P,
    operation: F,
) -> Result<T, GatewayError>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<(T, u64), ProviderError>>,
    P: Fn(&Deployment) -> bool,
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
        let mut deployment_lease = match router
            .select_deployment_lease_for_capability_matching_async(
                requested_model,
                &capability,
                |deployment| {
                    !excluded_budget_deployments.contains(deployment.id.as_str())
                        && !tried_deployments.contains(deployment.id.as_str())
                        && is_candidate(deployment)
                },
            )
            .await
        {
            Ok(lease) => lease,
            Err(RouterError::UnsupportedCapability { .. }) if !tried_deployments.is_empty() => {
                match router
                    .select_deployment_lease_for_capability_matching_async(
                        requested_model,
                        &capability,
                        |deployment| {
                            !excluded_budget_deployments.contains(deployment.id.as_str())
                                && is_candidate(deployment)
                        },
                    )
                    .await
                {
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
        let provider = deployment_lease.deployment().provider.clone();
        let selected_model = deployment_lease.deployment().model.clone();

        let completion = completion::UnaryCompletion::new(&deployment_lease, started_at);
        let result = completion::CURRENT
            .scope(
                completion.clone(),
                Box::pin(operation.clone()(
                    provider,
                    selected_model,
                    selected_deployment_id,
                )),
            )
            .await;
        match result {
            Ok((value, tokens_used)) => {
                completion.complete_success(tokens_used);
                router
                    .record_success_circuit_for_deployment_async(deployment_lease.deployment())
                    .await;
                deployment_lease.commit_admission_async(tokens_used).await;
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
                    completion.finish_admission(&mut deployment_lease).await;
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
                    router.record_local_failure(
                        deployment_lease.deployment(),
                        CooldownReason::ConsecutiveFailures,
                    );
                    router
                        .record_failure_circuit_for_deployment_async(
                            deployment_lease.deployment(),
                            crate::core::router::CooldownReason::ConsecutiveFailures,
                        )
                        .await;
                    // Do not pick this deployment again in this request while
                    // another candidate is available.
                    tried_deployments.insert(deployment_lease.clone_deployment_id());
                    completion.finish_admission(&mut deployment_lease).await;
                    drop(deployment_lease);
                    last_operation_error = Some(err);
                    attempt += 1;
                    if let Some(delay) = retry_decision.delay {
                        tokio::time::sleep(delay).await;
                    }
                    continue;
                }

                let cooldown_reason = infer_cooldown_reason(&err);
                router.record_local_failure(deployment_lease.deployment(), cooldown_reason);
                router
                    .record_failure_circuit_for_deployment_async(
                        deployment_lease.deployment(),
                        cooldown_reason,
                    )
                    .await;
                completion.finish_admission(&mut deployment_lease).await;
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
    execute_stream_with_selected_deployment_matching(
        router,
        requested_model,
        capability,
        |_| true,
        operation,
    )
    .await
}

pub(super) async fn execute_stream_with_selected_deployment_matching<T, F, Fut, P>(
    router: Arc<UnifiedRouter>,
    requested_model: &str,
    capability: ProviderCapability,
    is_candidate: P,
    operation: F,
) -> Result<(T, StreamingDeploymentLease), GatewayError>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
    P: Fn(&Deployment) -> bool,
{
    execute_stream_with_selected_deployment_matching_with_idempotency(
        router,
        requested_model,
        capability,
        is_candidate,
        RequestIdempotency::Idempotent,
        operation,
    )
    .await
}

/// Carry operation safety separately from whether response bytes were received.
/// A native creation may already exist upstream before its response headers arrive.
pub(super) async fn execute_stream_with_selected_deployment_matching_with_idempotency<
    T,
    F,
    Fut,
    P,
>(
    router: Arc<UnifiedRouter>,
    requested_model: &str,
    capability: ProviderCapability,
    is_candidate: P,
    idempotency: RequestIdempotency,
    operation: F,
) -> Result<(T, StreamingDeploymentLease), GatewayError>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
    P: Fn(&Deployment) -> bool,
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
        let mut deployment_lease = match router
            .select_deployment_lease_for_capability_matching_async(
                requested_model,
                &capability,
                |deployment| {
                    !excluded_budget_deployments.contains(deployment.id.as_str())
                        && !tried_deployments.contains(deployment.id.as_str())
                        && is_candidate(deployment)
                },
            )
            .await
        {
            Ok(lease) => lease,
            Err(RouterError::UnsupportedCapability { .. }) if !tried_deployments.is_empty() => {
                match router
                    .select_deployment_lease_for_capability_matching_async(
                        requested_model,
                        &capability,
                        |deployment| {
                            !excluded_budget_deployments.contains(deployment.id.as_str())
                                && is_candidate(deployment)
                        },
                    )
                    .await
                {
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
        let provider = deployment.provider.clone();
        let selected_model = deployment.model.clone();

        match operation.clone()(provider, selected_model, selected_deployment_id).await {
            Ok(stream) => {
                let (admission, hold) = deployment_lease.take_admission();
                let _deployment_id = deployment_lease.into_deployment_id();
                let lease = StreamingDeploymentLease::new(
                    router.clone(),
                    deployment,
                    started_at,
                    admission,
                    hold,
                );
                return Ok((stream, lease));
            }
            Err(err) => {
                if observability::is_budget_or_unpriced_fallback(
                    deployment_lease.deployment(),
                    &err,
                    true,
                ) {
                    excluded_budget_deployments.insert(deployment_lease.clone_deployment_id());
                    deployment_lease.cancel_admission_async().await;
                    drop(deployment_lease);
                    last_operation_error = Some(err);
                    continue;
                }

                let retry_decision = RetryPolicy.decide_for_deployment(
                    router.config(),
                    &deployment_lease.deployment().config,
                    &err,
                    RetryContext {
                        idempotency,
                        ..RetryContext::stream_pre_output(attempt, max_attempts)
                    },
                );
                if retry_decision.should_retry {
                    router
                        .record_failure_with_reason_for_deployment_async(
                            deployment_lease.deployment(),
                            crate::core::router::CooldownReason::ConsecutiveFailures,
                        )
                        .await;
                    // Do not pick this deployment again in this request while
                    // another candidate is available.
                    tried_deployments.insert(deployment_lease.clone_deployment_id());
                    deployment_lease.cancel_admission_async().await;
                    drop(deployment_lease);
                    last_operation_error = Some(err);
                    attempt += 1;
                    if let Some(delay) = retry_decision.delay {
                        tokio::time::sleep(delay).await;
                    }
                    continue;
                }

                let cooldown_reason = infer_cooldown_reason(&err);
                // Local caller policy errors do not describe provider health.
                if !matches!(err, ProviderError::InvalidRequest { .. }) {
                    router
                        .record_failure_with_reason_for_deployment_async(
                            deployment_lease.deployment(),
                            cooldown_reason,
                        )
                        .await;
                }
                deployment_lease.cancel_admission_async().await;
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
#[path = "execution_native_tests.rs"]
mod native_tests;

#[cfg(test)]
#[path = "execution_failover_tests.rs"]
mod failover_tests;

#[cfg(test)]
#[path = "execution_exclusion_tests.rs"]
mod exclusion_tests;

#[cfg(test)]
#[path = "execution_idempotency_tests.rs"]
mod idempotency_tests;
