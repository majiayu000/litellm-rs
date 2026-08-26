use super::{AuthAttemptTracker, AuthRateLimiter};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::time::Instant;

const MAX_EVICTION_PROBES: usize = 16;

pub(super) struct EvictionCandidate {
    client_id: String,
    entry_id: u64,
    token: u64,
    deadline: Option<Instant>,
}

pub(super) struct EvictionQueues {
    pub(super) ready: BTreeMap<(u64, u64), String>,
    pub(super) delayed: BTreeMap<(Instant, u64, u64), String>,
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
        now: Instant,
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
        self.eviction_token = self.eviction_token.wrapping_add(1);
        self.eviction_deadline = self.lockout_until.filter(|deadline| *deadline > now);
        Some(EvictionCandidate {
            client_id: client_id.to_string(),
            entry_id: self.entry_id,
            token: self.eviction_token,
            deadline: self.eviction_deadline,
        })
    }

    pub(super) fn retire_eviction_candidate(
        &mut self,
        client_id: &str,
    ) -> Option<EvictionCandidate> {
        if !self.eviction_queued {
            return None;
        }
        self.eviction_queued = false;
        Some(EvictionCandidate {
            client_id: client_id.to_string(),
            entry_id: self.entry_id,
            token: self.eviction_token,
            deadline: self.eviction_deadline.take(),
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
            let mut queues = self.eviction_queues.lock();
            let Some(tracker) = self.attempts.get(&candidate.client_id) else {
                return;
            };
            if tracker.entry_id != candidate.entry_id
                || !tracker.eviction_queued
                || tracker.eviction_token != candidate.token
                || tracker.eviction_deadline != candidate.deadline
            {
                return;
            }
            if let Some(deadline) = candidate.deadline {
                queues.delayed.insert(
                    (deadline, candidate.entry_id, candidate.token),
                    candidate.client_id,
                );
            } else {
                queues
                    .ready
                    .insert((candidate.entry_id, candidate.token), candidate.client_id);
            }
            drop(tracker);
        }
    }

    pub(super) fn remove_eviction_candidate(&self, candidate: Option<EvictionCandidate>) {
        if let Some(candidate) = candidate {
            let mut queues = self.eviction_queues.lock();
            queues.ready.remove(&(candidate.entry_id, candidate.token));
            if let Some(deadline) = candidate.deadline {
                queues
                    .delayed
                    .remove(&(deadline, candidate.entry_id, candidate.token));
            }
        }
    }

    pub(super) fn evict_one_inactive(&self, now: Instant) -> bool {
        let mut live_probes = 0;
        while live_probes < MAX_EVICTION_PROBES {
            let Some(candidate) = self.pop_eviction_candidate(now) else {
                return false;
            };
            if self
                .attempts
                .remove_if(&candidate.client_id, |_, tracker| {
                    tracker.entry_id == candidate.entry_id
                        && tracker.eviction_queued
                        && tracker.eviction_token == candidate.token
                        && tracker.is_evictable(now)
                })
                .is_some()
            {
                return true;
            }

            let candidate_state =
                self.attempts
                    .get_mut(&candidate.client_id)
                    .and_then(|mut tracker| {
                        if tracker.entry_id != candidate.entry_id
                            || !tracker.eviction_queued
                            || tracker.eviction_token != candidate.token
                        {
                            return None;
                        }
                        let active = tracker.waiting > 0 || tracker.in_flight > 0;
                        if active && tracker.lockout_until.is_none_or(|deadline| deadline <= now) {
                            // Settlement or cancellation will register a fresh
                            // candidate when this tracker becomes inactive.
                            tracker.eviction_queued = false;
                            tracker.eviction_deadline = None;
                        } else if let Some(deadline) =
                            tracker.lockout_until.filter(|deadline| *deadline > now)
                        {
                            tracker.eviction_deadline = Some(deadline);
                        } else {
                            tracker.eviction_deadline = None;
                        }
                        Some((tracker.lockout_until, active))
                    });
            match candidate_state {
                Some((Some(deadline), _)) if deadline > now => {
                    self.delay_eviction_candidate(candidate, deadline);
                }
                Some((_, true)) => {}
                Some((_, false)) => {
                    self.enqueue_eviction_candidate(Some(candidate));
                    live_probes += 1;
                }
                None => {}
            }
        }
        false
    }

    fn pop_eviction_candidate(&self, now: Instant) -> Option<EvictionCandidate> {
        let mut queues = self.eviction_queues.lock();
        if queues
            .delayed
            .first_key_value()
            .is_some_and(|((deadline, _, _), _)| *deadline <= now)
        {
            let ((_, entry_id, token), client_id) = queues.delayed.pop_first()?;
            return Some(EvictionCandidate {
                client_id,
                entry_id,
                token,
                deadline: None,
            });
        }
        let ((entry_id, token), client_id) = queues.ready.pop_first()?;
        Some(EvictionCandidate {
            client_id,
            entry_id,
            token,
            deadline: None,
        })
    }

    fn delay_eviction_candidate(&self, mut candidate: EvictionCandidate, deadline: Instant) {
        candidate.deadline = Some(deadline);
        self.enqueue_eviction_candidate(Some(candidate));
    }

    pub(super) fn prune_eviction_candidates(&self) {
        let mut queues = self.eviction_queues.lock();
        queues.ready.retain(|(entry_id, token), client_id| {
            self.attempts.get(client_id).is_some_and(|tracker| {
                tracker.entry_id == *entry_id
                    && tracker.eviction_queued
                    && tracker.eviction_token == *token
            })
        });
        queues
            .delayed
            .retain(|(deadline, entry_id, token), client_id| {
                self.attempts.get(client_id).is_some_and(|tracker| {
                    tracker.entry_id == *entry_id
                        && tracker.eviction_queued
                        && tracker.eviction_token == *token
                        && tracker.eviction_deadline == Some(*deadline)
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
                .eviction_queues
                .lock()
                .ready
                .insert((100 + stale as u64, 1), format!("stale-{stale}"));
        }

        limiter.record_failure("live-client");

        assert!(limiter.evict_one_inactive(Instant::now()));
        assert!(!limiter.attempts.contains_key("live-client"));
    }

    #[test]
    fn stale_retirement_cannot_remove_a_new_candidate() {
        let limiter = AuthRateLimiter::with_max_entries(5, 300, 60, 1);
        limiter.record_failure("client");
        let (retired, replacement) = {
            let mut tracker = limiter.attempts.get_mut("client").unwrap();
            let retired = tracker.retire_eviction_candidate("client");
            let replacement = tracker.mark_evictable("client", Instant::now());
            (retired, replacement)
        };

        limiter.enqueue_eviction_candidate(replacement);
        limiter.remove_eviction_candidate(retired);

        assert_eq!(limiter.eviction_queues.lock().ready.len(), 1);
        assert!(limiter.evict_one_inactive(Instant::now()));
    }

    #[test]
    fn late_enqueue_cannot_resurrect_a_retired_candidate() {
        let limiter = AuthRateLimiter::with_max_entries(5, 300, 60, 1);
        limiter.record_failure("client");
        let (old_candidate, late_candidate) = {
            let mut tracker = limiter.attempts.get_mut("client").unwrap();
            let old_candidate = tracker.retire_eviction_candidate("client");
            let late_candidate = tracker.mark_evictable("client", Instant::now());
            (old_candidate, late_candidate)
        };
        limiter.remove_eviction_candidate(old_candidate);
        let retired_candidate = {
            let mut tracker = limiter.attempts.get_mut("client").unwrap();
            tracker.retire_eviction_candidate("client")
        };
        limiter.remove_eviction_candidate(retired_candidate);

        limiter.enqueue_eviction_candidate(late_candidate);

        assert!(limiter.eviction_queues.lock().ready.is_empty());
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
        assert_eq!(limiter.attempts.get("locked-client").unwrap().waiting, 1);
        assert!(!waiting.is_finished());

        first.record_failure();
        second.record_failure();
        assert!(waiting.await.unwrap().is_err());

        assert!(limiter.eviction_queues.lock().ready.is_empty());
        assert_eq!(limiter.eviction_queues.lock().delayed.len(), 1);
        assert!(!limiter.evict_one_inactive(Instant::now()));
        assert!(limiter.evict_one_inactive(Instant::now() + std::time::Duration::from_secs(61)));
        assert!(!limiter.attempts.contains_key("locked-client"));
    }

    #[test]
    fn active_lockouts_do_not_consume_ready_probe_budget() {
        let limiter = AuthRateLimiter::with_max_entries(2, 300, 60, 18);
        for client in 0..17 {
            let client_id = format!("locked-{client}");
            assert_eq!(limiter.record_failure(&client_id), None);
            assert_eq!(limiter.record_failure(&client_id), Some(60));
        }
        assert_eq!(limiter.record_failure("ready-client"), None);

        assert!(limiter.evict_one_inactive(Instant::now()));
        assert!(!limiter.attempts.contains_key("ready-client"));
        assert_eq!(limiter.eviction_queues.lock().delayed.len(), 17);

        assert!(limiter.evict_one_inactive(Instant::now() + std::time::Duration::from_secs(61)));
        assert_eq!(limiter.eviction_queues.lock().delayed.len(), 16);
        assert!(limiter.eviction_queues.lock().ready.is_empty());
    }

    #[tokio::test]
    async fn active_trackers_do_not_consume_ready_probe_budget() {
        let limiter = std::sync::Arc::new(AuthRateLimiter::with_max_entries(2, 300, 60, 18));
        let mut active = Vec::new();
        for client in 0..17 {
            let client_id = format!("active-{client}");
            assert_eq!(limiter.record_failure(&client_id), None);
            active.push(limiter.reserve_attempt(&client_id).await.unwrap());
        }
        assert_eq!(limiter.record_failure("ready-client"), None);

        assert!(limiter.evict_one_inactive(Instant::now()));
        assert!(!limiter.attempts.contains_key("ready-client"));
        assert!(limiter.eviction_queues.lock().ready.is_empty());

        drop(active);
        assert_eq!(limiter.eviction_queues.lock().ready.len(), 17);
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
            assert_eq!(limiter.attempts.get(&client_id).unwrap().waiting, 1);
            assert!(!waiting.is_finished());

            active.release();
            waiting.await.unwrap().unwrap().release();
        }

        assert!(limiter.is_empty());
        assert!(limiter.eviction_queues.lock().ready.is_empty());
        assert!(limiter.eviction_queues.lock().delayed.is_empty());
    }
}
