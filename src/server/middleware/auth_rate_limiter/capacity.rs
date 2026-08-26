use super::{AuthAttemptTracker, AuthRateLimiter};
use std::sync::atomic::Ordering;
use std::time::Instant;

const MAX_EVICTION_PROBES: usize = 16;

pub(super) struct EvictionCandidate {
    client_id: String,
    entry_id: u64,
}

impl AuthAttemptTracker {
    pub(super) fn is_evictable(&self, now: Instant) -> bool {
        self.waiting == 0
            && self.in_flight == 0
            && self.lockout_until.is_none_or(|until| until <= now)
    }

    pub(super) fn mark_evictable(
        &mut self,
        client_id: &str,
        _now: Instant,
    ) -> Option<EvictionCandidate> {
        // Active lockouts are retained until their deadline, but still need a
        // candidate so they become discoverable for eviction after expiry.
        let has_persistent_history =
            self.failure_count > 0 || self.lockout_until.is_some() || self.lockout_count > 0;
        if self.eviction_queued
            || self.in_flight > 0
            || (self.waiting > 0 && !has_persistent_history)
        {
            return None;
        }
        self.eviction_queued = true;
        Some(EvictionCandidate {
            client_id: client_id.to_string(),
            entry_id: self.entry_id,
        })
    }
}

impl AuthRateLimiter {
    pub(super) fn new_tracker(&self, now: Instant) -> AuthAttemptTracker {
        let entry_id = self.next_entry_id.fetch_add(1, Ordering::Relaxed);
        AuthAttemptTracker::new(now, self.max_attempts.max(1) as usize, entry_id)
    }

    pub(super) fn enqueue_eviction_candidate(&self, candidate: Option<EvictionCandidate>) {
        if let Some(candidate) = candidate {
            self.eviction_candidates.lock().push_back(candidate);
        }
    }

    pub(super) fn evict_one_inactive(&self, now: Instant) -> bool {
        let mut live_probes = 0;
        while live_probes < MAX_EVICTION_PROBES {
            let Some(candidate) = self.eviction_candidates.lock().pop_front() else {
                return false;
            };
            if self
                .attempts
                .remove_if(&candidate.client_id, |_, tracker| {
                    tracker.entry_id == candidate.entry_id
                        && tracker.eviction_queued
                        && tracker.is_evictable(now)
                })
                .is_some()
            {
                return true;
            }

            let should_requeue = self
                .attempts
                .get(&candidate.client_id)
                .is_some_and(|tracker| {
                    tracker.entry_id == candidate.entry_id && tracker.eviction_queued
                });
            if should_requeue {
                self.eviction_candidates.lock().push_back(candidate);
                live_probes += 1;
            }
        }
        false
    }

    pub(super) fn prune_eviction_candidates(&self) {
        self.eviction_candidates.lock().retain(|candidate| {
            self.attempts
                .get(&candidate.client_id)
                .is_some_and(|tracker| {
                    tracker.entry_id == candidate.entry_id && tracker.eviction_queued
                })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_candidates_do_not_consume_the_live_probe_budget() {
        let limiter = AuthRateLimiter::with_max_entries(5, 300, 60, 1);
        for stale in 0..=MAX_EVICTION_PROBES {
            limiter
                .eviction_candidates
                .lock()
                .push_back(EvictionCandidate {
                    client_id: format!("stale-{stale}"),
                    entry_id: stale as u64,
                });
        }

        limiter.record_failure("live-client");

        assert!(limiter.evict_one_inactive(Instant::now()));
        assert!(!limiter.attempts.contains_key("live-client"));
    }

    #[tokio::test]
    async fn concurrent_lockout_becomes_evictable_after_expiry() {
        let limiter = std::sync::Arc::new(AuthRateLimiter::with_max_entries(2, 300, 60, 1));
        let first = limiter.reserve_attempt("locked-client").await.unwrap();
        let second = limiter.reserve_attempt("locked-client").await.unwrap();
        let waiting_limiter = std::sync::Arc::clone(&limiter);
        let waiting =
            tokio::spawn(async move { waiting_limiter.reserve_attempt("locked-client").await });
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());

        first.record_failure();
        second.record_failure();
        assert!(waiting.await.unwrap().is_err());

        assert_eq!(limiter.eviction_candidates.lock().len(), 1);
        assert!(!limiter.evict_one_inactive(Instant::now()));
        assert!(limiter.evict_one_inactive(Instant::now() + std::time::Duration::from_secs(61)));
        assert!(!limiter.attempts.contains_key("locked-client"));
    }

    #[tokio::test]
    async fn queued_successes_do_not_leave_stale_candidates() {
        let limiter = std::sync::Arc::new(AuthRateLimiter::new(1, 300, 60));
        for client in 0..20 {
            let client_id = format!("successful-client-{client}");
            let active = limiter.reserve_attempt(&client_id).await.unwrap();
            let waiting_limiter = std::sync::Arc::clone(&limiter);
            let waiting_client_id = client_id.clone();
            let waiting =
                tokio::spawn(
                    async move { waiting_limiter.reserve_attempt(&waiting_client_id).await },
                );
            tokio::task::yield_now().await;
            assert!(!waiting.is_finished());

            active.release();
            waiting.await.unwrap().unwrap().release();
        }

        assert!(limiter.is_empty());
        assert!(limiter.eviction_candidates.lock().is_empty());
    }
}
