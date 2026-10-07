//! Completion ownership for runtime-backed SDK streams.

use super::execution::infer_cooldown_reason;
use super::selection::DeploymentLease;
use super::{CooldownReason, RuntimeHandle, UnifiedRouter};
use crate::core::providers::ProviderError;
use crate::core::types::{chat::ChatRequest, responses::ChatChunk};
use crate::utils::ai::counter::token_counter::{TokenCounter, TokenizerIdentity};
use std::{sync::Arc, time::Instant};

pub(crate) struct RuntimeStreamCompletion {
    router: Arc<UnifiedRouter>,
    lease: Option<DeploymentLease>,
    started_at: Instant,
    usage: Option<u64>,
    usage_covers_output: bool,
    output_observed: bool,
    terminal_failure: Option<CooldownReason>,
    outcome_recorded: bool,
}

impl RuntimeHandle {
    pub(crate) fn estimated_stream_tokens(
        request: &ChatRequest,
    ) -> crate::utils::error::gateway_error::Result<u64> {
        let input = request.estimate_input_tokens();
        let output = TokenCounter::new().estimate_output_tokens(
            request.max_completion_tokens.or(request.max_tokens),
            input,
            &TokenizerIdentity::approximate("runtime", &request.model),
        )?;
        Ok(u64::from(input).saturating_add(u64::from(output)))
    }

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
            usage_covers_output: false,
            output_observed: false,
            terminal_failure: None,
            outcome_recorded: false,
        }
    }
}

impl RuntimeStreamCompletion {
    pub(crate) fn observe_chunk(&mut self, chunk: &ChatChunk) {
        if chunk_has_output(chunk) {
            self.observe_output();
        }
        if let Some(usage) = &chunk.usage {
            self.observe_usage(u64::from(usage.total_tokens));
        }
    }

    pub(crate) fn observe_output(&mut self) {
        self.output_observed = true;
        // A prior usage snapshot cannot settle output generated after it.
        // Keep that observed count locally, but retain the shared reservation
        // until another usage record covers the newly observed payload.
        self.usage_covers_output = false;
        if let Some(lease) = &self.lease {
            lease.preserve_admission_reservation(self.usage.unwrap_or_default());
        }
    }

    pub(crate) fn observe_usage(&mut self, tokens: u64) {
        self.usage = Some(tokens);
        self.usage_covers_output = true;
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
        if self.usage_covers_output {
            lease.preserve_admission_usage(tokens);
        } else {
            lease.preserve_admission_reservation(self.usage.unwrap_or_default());
        }
        self.router
            .record_success_circuit_for_deployment_async(lease.deployment())
            .await;
        if self.usage_covers_output {
            lease.commit_admission_async(tokens).await;
        } else {
            lease
                .retain_admission_async(self.usage.unwrap_or_default())
                .await;
        }
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
            lease.deployment().record_interrupted_usage(tokens);
        } else if self.output_observed {
            lease.deployment().record_interrupted_usage(0);
        }
        self.router
            .record_failure_with_reason_for_deployment_async(lease.deployment(), reason)
            .await;
        self.outcome_recorded = true;
        if self.usage_covers_output {
            lease
                .commit_admission_async(self.usage.unwrap_or_default())
                .await;
        } else if self.output_observed {
            lease
                .retain_admission_async(self.usage.unwrap_or_default())
                .await;
        } else {
            lease.cancel_admission_async().await;
        }
        self.lease.take();
    }
}

fn chunk_has_output(chunk: &ChatChunk) -> bool {
    let nonempty = |value: &Option<String>| value.as_ref().is_some_and(|value| !value.is_empty());
    let function_output = |function: &crate::core::types::responses::FunctionCallDelta| {
        nonempty(&function.name) || nonempty(&function.arguments)
    };
    chunk.choices.iter().any(|choice| {
        let delta = &choice.delta;
        nonempty(&delta.content)
            || delta
                .thinking_content()
                .is_some_and(|value| !value.is_empty())
            || delta.function_call.as_ref().is_some_and(function_output)
            || delta.tool_calls.as_ref().is_some_and(|calls| {
                calls
                    .iter()
                    .any(|call| call.function.as_ref().is_some_and(function_output))
            })
            || delta
                .audio
                .as_ref()
                .is_some_and(|audio| nonempty(&audio.data) || nonempty(&audio.transcript))
    })
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
            } else if self.output_observed {
                // RPM is known; the retained distributed estimate is not an
                // observed token count and must not enter actual local TPM.
                lease.deployment().record_interrupted_usage(0);
            }
        }
        // DeploymentLease releases local active requests. AdmissionHold has
        // already been marked with usage before the corresponding yield and
        // queues the known settlement without blocking the dropping thread.
    }
}
