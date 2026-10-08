//! Completion facts owned by the exact routing attempt.
//!
//! The scope covers only its operation future. A usage observation alone is
//! neutral: provider success and a validated accounting wait are separate facts.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use parking_lot::Mutex;

use crate::core::budget::{BudgetReservationError, BudgetStatus, UnifiedBudgetReservation};
use crate::core::router::admission::AdmissionHold;
use crate::core::router::deployment::Deployment;
use crate::core::router::selection::DeploymentLease;
use crate::core::router::{CooldownReason, UnifiedRouter};

tokio::task_local! {
    pub(super) static CURRENT: Arc<UnaryCompletion>;
}

pub(super) struct UnaryCompletion {
    deployment: Arc<Deployment>,
    hold: Option<AdmissionHold>,
    started_at: Instant,
    provider_succeeded: AtomicBool,
    recorded: AtomicBool,
    usage: Mutex<Option<u64>>,
    // Native response facts are authoritative even when usage is unknown.
    // Legacy budget hooks must not replace None (or a known count) with zero.
    native_usage: Option<Option<u64>>,
    terminal: Option<TerminalOutcome>,
}

#[derive(Clone)]
pub(super) enum TerminalOutcome {
    Success,
    Failure(Arc<UnifiedRouter>, CooldownReason, bool),
    Interrupted,
}

pub(super) fn retain_failure_admission(tokens: u64, interrupted: bool) -> bool {
    tokens > 0 || interrupted
}

impl UnaryCompletion {
    pub(super) fn new(lease: &DeploymentLease, started_at: Instant) -> Arc<Self> {
        Arc::new(Self {
            deployment: lease.clone_deployment(),
            hold: lease.clone_admission_hold(),
            started_at,
            provider_succeeded: AtomicBool::new(false),
            recorded: AtomicBool::new(false),
            usage: Mutex::new(None),
            native_usage: None,
            terminal: None,
        })
    }

    pub(super) fn terminal(
        deployment: Arc<Deployment>,
        hold: Option<AdmissionHold>,
        started_at: Instant,
        tokens: u64,
        outcome: TerminalOutcome,
    ) -> Arc<Self> {
        let completion = Arc::new(Self {
            deployment,
            hold,
            started_at,
            provider_succeeded: AtomicBool::new(matches!(outcome, TerminalOutcome::Success)),
            recorded: AtomicBool::new(false),
            usage: Mutex::new(Some(tokens)),
            native_usage: None,
            terminal: Some(outcome),
        });
        completion.prepare_admission_usage(tokens);
        completion
    }

    pub(super) fn native_terminal(
        deployment: Arc<Deployment>,
        hold: Option<AdmissionHold>,
        started_at: Instant,
        usage: Option<u64>,
        outcome: TerminalOutcome,
    ) -> Arc<Self> {
        let completion = Arc::new(Self {
            deployment,
            hold,
            started_at,
            provider_succeeded: AtomicBool::new(matches!(outcome, TerminalOutcome::Success)),
            recorded: AtomicBool::new(false),
            usage: Mutex::new(None),
            native_usage: Some(usage),
            terminal: Some(outcome),
        });
        completion.prepare_admission_usage(usage.unwrap_or(0));
        completion
    }

    fn prepare_admission_usage(&self, tokens: u64) {
        if let Some(usage) = self.native_usage {
            if let Some(hold) = &self.hold {
                match usage {
                    Some(tokens) => hold.prepare_settlement(tokens),
                    None => hold.prepare_retention(0),
                }
            }
            return;
        }
        let retain = match &self.terminal {
            Some(TerminalOutcome::Failure(_, _, interrupted)) => {
                retain_failure_admission(tokens, *interrupted)
            }
            _ => true,
        };
        if let Some(hold) = &self.hold {
            if retain {
                hold.prepare_settlement(tokens);
            } else {
                hold.prepare_cancellation();
            }
        }
    }

    pub(super) fn disarm(&self) {
        self.recorded.store(true, Ordering::Release);
    }

    pub(super) fn complete_success(&self, tokens: u64) {
        if self.native_usage.is_some() {
            self.record_cancelled();
            return;
        }
        if !self.recorded.swap(true, Ordering::AcqRel) {
            if let Some(hold) = &self.hold {
                hold.prepare_settlement(tokens);
            }
            self.deployment.record_success_with_admission(
                tokens,
                self.started_at.elapsed().as_micros() as u64,
                self.hold.as_ref(),
            );
        }
    }

    pub(super) async fn finish_admission(&self, lease: &mut DeploymentLease) {
        let usage = *self.usage.lock();
        if let Some(tokens) = usage {
            lease.commit_admission_async(tokens).await;
        } else {
            lease.cancel_admission_async().await;
        }
    }
}

impl UnaryCompletion {
    pub(super) fn record_cancelled(&self) {
        if !self.recorded.swap(true, Ordering::AcqRel)
            && let Some(tokens) = self
                .native_usage
                .map(|usage| usage.unwrap_or(0))
                .or_else(|| *self.usage.lock())
        {
            // Local postprocessing failure or cancellation before a validated
            // settlement must retain usage without inventing provider success.
            self.prepare_admission_usage(tokens);
            match &self.terminal {
                Some(TerminalOutcome::Success)
                    if self.provider_succeeded.load(Ordering::Acquire) =>
                {
                    self.deployment.record_success_with_admission(
                        tokens,
                        self.started_at.elapsed().as_micros() as u64,
                        self.hold.as_ref(),
                    );
                }
                Some(TerminalOutcome::Failure(router, reason, interrupted)) => {
                    if *interrupted {
                        self.deployment
                            .record_interrupted_usage_with_admission(tokens, self.hold.as_ref());
                    } else {
                        self.deployment
                            .record_partial_tokens_with_admission(tokens, self.hold.as_ref());
                    }
                    router.record_local_failure(&self.deployment, *reason);
                }
                _ => self
                    .deployment
                    .record_interrupted_usage_with_admission(tokens, self.hold.as_ref()),
            }
        }
    }
}

impl Drop for UnaryCompletion {
    fn drop(&mut self) {
        self.record_cancelled();
    }
}

pub(in crate::server::routes::ai) fn provider_succeeded() {
    let _ = CURRENT.try_with(|completion| {
        completion.provider_succeeded.store(true, Ordering::Release);
    });
}

pub(in crate::server::routes::ai) fn observe_usage(tokens: u64) {
    let _ = CURRENT.try_with(|completion| {
        if completion.native_usage.is_none() {
            *completion.usage.lock() = Some(tokens);
            completion.prepare_admission_usage(tokens);
        }
    });
}

pub(in crate::server::routes::ai) async fn settle_budget(
    reservation: UnifiedBudgetReservation,
    cost: f64,
) -> Result<(Option<BudgetStatus>, Option<BudgetStatus>), BudgetReservationError> {
    let wait = BudgetSettlementWait::new();
    let result = reservation.settle_async(cost).await;
    wait.finish();
    result
}

/// Only a completed provider response may claim success in this wait. Finishing
/// it normally leaves outcome recording to the router, so a later local error
/// cannot retrospectively turn an early counter increment into a false success.
pub(super) struct BudgetSettlementWait {
    completion: Option<Arc<UnaryCompletion>>,
    finished: bool,
}

impl BudgetSettlementWait {
    pub(super) fn new() -> Self {
        Self {
            completion: CURRENT
                .try_with(|completion| {
                    completion
                        .provider_succeeded
                        .load(Ordering::Acquire)
                        .then(|| Arc::clone(completion))
                })
                .ok()
                .flatten(),
            finished: false,
        }
    }

    pub(super) fn finish(mut self) {
        self.finished = true;
    }
}

impl Drop for BudgetSettlementWait {
    fn drop(&mut self) {
        if !self.finished
            && let Some(completion) = &self.completion
        {
            let tokens = completion.usage.lock().unwrap_or_default();
            completion.complete_success(tokens);
        }
    }
}
