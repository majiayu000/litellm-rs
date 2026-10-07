//! Completion ownership for runtime-backed SDK streams.

use super::execution::infer_cooldown_reason;
use super::selection::DeploymentLease;
use super::{CooldownReason, RuntimeHandle, UnifiedRouter};
use crate::core::providers::ProviderError;
use std::{sync::Arc, time::Instant};

pub(crate) struct RuntimeStreamCompletion {
    router: Arc<UnifiedRouter>,
    lease: Option<DeploymentLease>,
    started_at: Instant,
    usage: Option<u64>,
    terminal_failure: Option<CooldownReason>,
    outcome_recorded: bool,
}

impl RuntimeHandle {
    pub(crate) fn stream_completion(
        &self,
        lease: DeploymentLease,
        started_at: Instant,
    ) -> RuntimeStreamCompletion {
        RuntimeStreamCompletion {
            router: self.binding.router.clone(),
            lease: Some(lease),
            started_at,
            usage: None,
            terminal_failure: None,
            outcome_recorded: false,
        }
    }
}

impl RuntimeStreamCompletion {
    pub(crate) fn observe_usage(&mut self, tokens: u64) {
        self.usage = Some(tokens);
        if let Some(lease) = &self.lease {
            lease.preserve_admission_usage(tokens);
        }
    }

    pub(crate) async fn finish_success(mut self) {
        let lease = self.lease.as_mut().expect("live stream completion");
        let tokens = self.usage.unwrap_or_default();
        lease
            .deployment()
            .record_success(tokens, self.started_at.elapsed().as_micros() as u64);
        self.outcome_recorded = true;
        lease.commit_admission_async(tokens).await;
        self.router
            .record_success_circuit_for_deployment_async(lease.deployment())
            .await;
        // EOF releases the exact snapshot lease even if the caller retains
        // the exhausted Stream object indefinitely.
        self.lease.take();
    }

    pub(crate) async fn finish_failure(mut self, error: &ProviderError) {
        let reason = match infer_cooldown_reason(error) {
            reason @ (CooldownReason::RateLimit
            | CooldownReason::AuthError
            | CooldownReason::NotFound) => reason,
            _ => CooldownReason::ConsecutiveFailures,
        };
        self.terminal_failure = Some(reason);
        let lease = self.lease.as_mut().expect("live stream completion");
        if let Some(tokens) = self.usage {
            lease.deployment().record_partial_tokens(tokens);
        }
        self.router
            .record_failure_with_reason_for_deployment_async(lease.deployment(), reason)
            .await;
        self.outcome_recorded = true;
        if let Some(tokens) = self.usage {
            lease.commit_admission_async(tokens).await;
        } else {
            lease.cancel_admission_async().await;
        }
        self.lease.take();
    }
}

impl Drop for RuntimeStreamCompletion {
    fn drop(&mut self) {
        let Some(lease) = self.lease.as_ref() else {
            return;
        };
        if !self.outcome_recorded {
            if let Some(reason) = self.terminal_failure {
                // Cancellation while the shared breaker is pending must not
                // erase the known provider error. Its normal path records
                // locally without another yield before setting outcome_recorded.
                self.router.record_local_failure(lease.deployment(), reason);
            } else if let Some(tokens) = self.usage {
                // Consumer cancellation or SDK conversion failure is neutral
                // for provider health, but observed usage is still real.
                lease.deployment().record_interrupted_usage(tokens);
            }
        }
        // DeploymentLease releases local active requests. AdmissionHold has
        // already been marked with usage before the corresponding yield and
        // queues the known settlement without blocking the dropping thread.
    }
}
