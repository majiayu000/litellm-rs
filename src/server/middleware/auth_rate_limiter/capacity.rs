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
        now: Instant,
    ) -> Option<EvictionCandidate> {
        if self.eviction_queued || !self.is_evictable(now) {
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
        for _ in 0..MAX_EVICTION_PROBES {
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
