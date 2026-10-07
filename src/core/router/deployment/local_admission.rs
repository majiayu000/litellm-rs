//! Local token reservations are separate from observed usage statistics.

use super::{DeploymentState, current_timestamp};
use parking_lot::Mutex;
use std::sync::atomic::Ordering;

#[derive(Debug, Default)]
pub(super) struct LocalTokenLedger {
    pub(super) active_estimate: u64,
    pub(super) retained_unobserved: u64,
}

#[derive(Clone, Copy, Debug)]
enum Completion {
    Cancel,
    Settle,
    Retain(u64),
}

/// One reservation, shared by completion guards without duplicating ownership.
#[derive(Debug)]
pub(crate) struct LocalAdmissionHold {
    state: DeploymentState,
    estimate: u64,
    completion: Mutex<Option<Completion>>,
}

impl DeploymentState {
    /// Admission includes in-flight estimates and unknown settled usage, while
    /// `minute_counters` continues to expose only observed usage.
    pub(crate) fn admission_tpm(&self, now: u64) -> u64 {
        self.with_current_minute(now, || {
            let ledger = self.local_admission.lock();
            self.observed_minute_counters()
                .tpm
                .saturating_add(ledger.active_estimate)
                .saturating_add(ledger.retained_unobserved)
        })
    }

    pub(crate) fn reserve_local_tokens(
        &self,
        estimate: u64,
        limit: u64,
    ) -> Option<LocalAdmissionHold> {
        self.with_current_minute(current_timestamp(), || {
            // Every ledger operation follows minute gate -> ledger. Rollover
            // cannot clear new-window usage or an outstanding reservation.
            let mut ledger = self.local_admission.lock();
            let total = u128::from(self.tpm_current.load(Ordering::Relaxed))
                + u128::from(ledger.active_estimate)
                + u128::from(ledger.retained_unobserved)
                + u128::from(estimate);
            if total > u128::from(limit) {
                return None;
            }
            ledger.active_estimate += estimate;
            Some(LocalAdmissionHold {
                state: self.clone(),
                estimate,
                completion: Mutex::new(Some(Completion::Cancel)),
            })
        })
    }
}

impl LocalAdmissionHold {
    fn prepare(&self, desired: Completion) {
        let mut completion = self.completion.lock();
        if completion.is_some() {
            *completion = Some(desired);
        }
    }

    pub(crate) fn prepare_settlement(&self) {
        self.prepare(Completion::Settle);
    }

    pub(crate) fn prepare_retention(&self, minimum: u64) {
        self.prepare(Completion::Retain(minimum));
    }

    #[cfg(feature = "gateway")]
    pub(crate) fn prepare_cancellation(&self) {
        self.prepare(Completion::Cancel);
    }

    fn apply(&self, ledger: &mut LocalTokenLedger, completion: Completion, observed: u64) {
        ledger.active_estimate -= self.estimate;
        if let Completion::Retain(minimum) = completion {
            // The caller records observed tokens separately. Retain only the
            // missing portion so admission becomes max(estimate, known prefix).
            let extra = self.estimate.max(minimum).saturating_sub(observed);
            ledger.retained_unobserved = ledger.retained_unobserved.saturating_add(extra);
        }
    }

    /// Called inside the matching deployment's minute gate. Recording actual
    /// usage and replacing its estimate happen under one ledger lock, before
    /// any asynchronous circuit/accounting wait can cross a minute boundary.
    pub(crate) fn record_observation<T>(&self, observed: u64, record: impl FnOnce() -> T) -> T {
        let mut completion = self.completion.lock();
        let mut ledger = self.state.local_admission.lock();
        let result = record();
        if let Some(completion) = completion.take() {
            self.apply(&mut ledger, completion, observed);
        }
        result
    }

    pub(crate) fn finish(&self, op: &str, minimum: u64) {
        let desired = match op {
            "settle" => Completion::Settle,
            "retain" => Completion::Retain(minimum),
            _ => Completion::Cancel,
        };
        let completion = {
            let mut completion = self.completion.lock();
            completion.take().map(|_| desired)
        };
        self.finish_prepared(completion);
    }

    fn finish_prepared(&self, completion: Option<Completion>) {
        let Some(completion) = completion else {
            return;
        };
        // Never hold the completion lock while acquiring the minute gate.
        self.state.with_current_minute(current_timestamp(), || {
            let observed = match completion {
                Completion::Retain(minimum) => minimum,
                _ => 0,
            };
            self.apply(&mut self.state.local_admission.lock(), completion, observed);
        });
    }
}

impl Drop for LocalAdmissionHold {
    fn drop(&mut self) {
        let completion = self.completion.get_mut().take();
        self.finish_prepared(completion);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::{Provider, openai::OpenAIProvider};
    use crate::core::router::admission::AdmissionHold;
    use crate::core::router::{Deployment, RouterConfig, UnifiedRouter};
    use std::sync::{Arc, Barrier};

    async fn deployment(key: &str) -> Deployment {
        let provider = Provider::OpenAI(OpenAIProvider::with_api_key(key).await.unwrap());
        let mut deployment = Deployment::new(
            "same-id".into(),
            provider,
            "wire-model".into(),
            "public-model".into(),
        );
        deployment.config.tpm_limit = Some(10);
        deployment
    }

    #[tokio::test]
    async fn local_token_reservations_roll_without_losing_live_holds_or_double_finishing() {
        let deployment = deployment("sk-local-window").await;
        let local = Arc::new(deployment.state.reserve_local_tokens(6, 10).unwrap());
        let hold = AdmissionHold::InProcess(local.clone());
        let snapshot = deployment.state.for_snapshot_insertion();
        assert!(snapshot.reserve_local_tokens(5, 10).is_none());
        deployment.state.reset_minute();
        assert!(
            snapshot.reserve_local_tokens(5, 10).is_none(),
            "live work crosses the reset"
        );
        hold.prepare_retention(0);
        deployment.record_success_with_admission(0, 1, Some(&hold));
        assert_eq!(deployment.state.minute_counters(current_timestamp()).tpm, 0);
        assert!(
            snapshot.reserve_local_tokens(5, 10).is_none(),
            "unknown completed work stays charged"
        );
        local.finish("retain", 1_000);
        local.finish("cancel", 0);
        drop(hold);
        drop(local);
        assert_eq!(
            snapshot.admission_tpm(current_timestamp()),
            6,
            "completion is one-shot"
        );
        snapshot.reset_minute();
        let next = snapshot.reserve_local_tokens(10, 10).unwrap();
        drop(next);
        assert_eq!(snapshot.admission_tpm(current_timestamp()), 0);

        for (estimate, observed, retain, expected) in [
            (6, 0, false, 0),
            (6, 3, false, 3),
            (6, 3, true, 6),
            (6, 9, true, 9),
        ] {
            deployment.state.reset_minute();
            let local = Arc::new(deployment.state.reserve_local_tokens(estimate, 10).unwrap());
            let hold = AdmissionHold::InProcess(local.clone());
            if retain {
                hold.prepare_retention(observed);
            } else {
                hold.prepare_settlement(observed);
            }
            deployment.record_success_with_admission(observed, 1, Some(&hold));
            assert_eq!(
                deployment.state.minute_counters(current_timestamp()).tpm,
                observed
            );
            assert_eq!(
                deployment.state.admission_tpm(current_timestamp()),
                expected
            );
            // Async bookkeeping that resumes in the next window cannot bring
            // already completed estimates or old observed tokens back.
            deployment.state.reset_minute();
            local.finish(if retain { "retain" } else { "settle" }, observed);
            drop(hold);
            drop(local);
            assert_eq!(deployment.state.admission_tpm(current_timestamp()), 0);
        }
    }

    #[test]
    fn local_token_admission_is_atomic_for_competing_reservations_and_overflow() {
        let state = DeploymentState::new();
        let ready = Arc::new(Barrier::new(16));
        let admitted = Arc::new(Barrier::new(16));
        let holds = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..16)
                .map(|_| {
                    let state = state.clone();
                    let ready = ready.clone();
                    let admitted = admitted.clone();
                    scope.spawn(move || {
                        ready.wait();
                        let hold = state.reserve_local_tokens(6, 10);
                        admitted.wait();
                        hold
                    })
                })
                .collect();
            workers
                .into_iter()
                .filter_map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(
            holds.len(),
            1,
            "only one six-token request fits a ten-token limit"
        );
        assert_eq!(state.admission_tpm(current_timestamp()), 6);
        drop(holds);
        assert_eq!(state.admission_tpm(current_timestamp()), 0);
        let huge = state.reserve_local_tokens(u64::MAX, u64::MAX).unwrap();
        assert!(
            state.reserve_local_tokens(1, u64::MAX).is_none(),
            "sum overflow must fail closed"
        );
        drop(huge);
        assert!(state.reserve_local_tokens(1, u64::MAX).is_some());
    }

    #[tokio::test]
    async fn local_token_holds_follow_the_snapshot_resource_across_id_reuse() {
        let router = UnifiedRouter::new(RouterConfig {
            num_retries: 0,
            ..Default::default()
        });
        router.add_deployment(deployment("sk-local-original").await);
        let old_snapshot = router.load_routing_snapshot();
        let old = router
            .select_deployment_lease_with_tokens("public-model", 6)
            .unwrap();
        assert!(router.remove_deployment("same-id").is_some());
        router.add_deployment(deployment("sk-local-replacement").await);
        let replacement = router
            .select_deployment_lease_with_tokens("public-model", 6)
            .unwrap();
        drop(old);
        assert!(
            router
                .select_deployment_lease_with_tokens("public-model", 6)
                .is_err(),
            "retired completion must not release the replacement reservation"
        );
        let retired = old_snapshot.deployments.get("same-id").unwrap();
        assert!(
            retired.state.reserve_local_tokens(10, 10).is_some(),
            "the retired resource released its own hold"
        );
        drop(replacement);
        assert!(
            router
                .select_deployment_lease_with_tokens("public-model", 10)
                .is_ok()
        );
    }
}
