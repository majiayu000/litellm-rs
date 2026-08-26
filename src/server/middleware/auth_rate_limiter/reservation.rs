use super::{AuthAttemptTracker, AuthRateLimiter};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const CAPACITY_RETRY_SECS: u64 = 1;
const MAX_QUEUED_ATTEMPTS_PER_CLIENT: u32 = 256;

/// One authentication attempt admitted by [`AuthRateLimiter`].
///
/// Dropping a live reservation releases its in-flight slot, so cancellation
/// and infrastructure errors cannot leak limiter capacity.
#[must_use = "the authentication attempt reservation must be settled or retained"]
pub(crate) struct AuthAttemptReservation {
    limiter: Option<Arc<AuthRateLimiter>>,
    client_id: String,
    permit: Option<OwnedSemaphorePermit>,
    generation: u64,
    active: bool,
}

impl AuthAttemptReservation {
    pub(crate) fn record_failure(mut self) {
        self.finish(true);
    }

    pub(crate) fn release(mut self) {
        self.finish(false);
    }

    fn finish(&mut self, failed: bool) {
        if !std::mem::replace(&mut self.active, false) {
            return;
        }

        if let Some(limiter) = self.limiter.as_ref() {
            limiter.finish_reservation(&self.client_id, self.generation, failed);
        }
        self.permit.take();
    }
}

impl Drop for AuthAttemptReservation {
    fn drop(&mut self) {
        self.finish(false);
    }
}

struct PendingAttempt {
    limiter: Arc<AuthRateLimiter>,
    client_id: String,
    active: bool,
}

impl PendingAttempt {
    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for PendingAttempt {
    fn drop(&mut self) {
        if self.active {
            self.limiter.cancel_waiter(&self.client_id);
        }
    }
}

impl AuthRateLimiter {
    /// Reserve an attempt only when the request has a trustworthy network key.
    pub(crate) async fn reserve_network_attempt(
        self: &Arc<Self>,
        client_id: Option<&str>,
    ) -> Result<AuthAttemptReservation, u64> {
        match client_id {
            Some(client_id) => self.reserve_attempt(client_id).await,
            None => Ok(AuthAttemptReservation {
                limiter: None,
                client_id: String::new(),
                permit: None,
                generation: 0,
                active: true,
            }),
        }
    }

    /// Queue an authentication attempt before asynchronous credential
    /// verification begins.
    ///
    /// At most `max_attempts` credential checks for one client run at once.
    /// Additional requests wait for a slot instead of being mistaken for
    /// failures; once a preceding batch establishes a lockout, queued requests
    /// are rejected before their credentials are checked.
    pub(crate) async fn reserve_attempt(
        self: &Arc<Self>,
        client_id: &str,
    ) -> Result<AuthAttemptReservation, u64> {
        let (mut pending, admission) = self.queue_attempt(client_id)?;
        let permit = match admission.acquire_owned().await {
            Ok(permit) => permit,
            Err(error) => {
                tracing::error!(
                    client_id,
                    %error,
                    "Authentication limiter admission semaphore closed unexpectedly"
                );
                return Err(CAPACITY_RETRY_SECS);
            }
        };

        let result = self.activate_waiter(client_id);
        pending.disarm();
        let generation = result?;

        Ok(AuthAttemptReservation {
            limiter: Some(Arc::clone(self)),
            client_id: client_id.to_string(),
            permit: Some(permit),
            generation,
            active: true,
        })
    }

    fn queue_attempt(
        self: &Arc<Self>,
        client_id: &str,
    ) -> Result<(PendingAttempt, Arc<Semaphore>), u64> {
        let now = Instant::now();
        let admission = if let Some(mut entry) = self.attempts.get_mut(client_id) {
            self.queue_on_tracker(entry.value_mut(), now)?
        } else {
            let _capacity_guard = self.capacity_admission_lock.lock();
            if let Some(mut entry) = self.attempts.get_mut(client_id) {
                self.queue_on_tracker(entry.value_mut(), now)?
            } else {
                if self.attempts.len() >= self.max_entries && !self.evict_one_inactive(now) {
                    self.blocked_count.fetch_add(1, Ordering::Relaxed);
                    return Err(CAPACITY_RETRY_SECS);
                }
                let mut tracker = self.new_tracker(now);
                tracker.waiting = 1;
                let admission = Arc::clone(&tracker.admission);
                self.attempts.insert(client_id.to_string(), tracker);
                admission
            }
        };

        Ok((
            PendingAttempt {
                limiter: Arc::clone(self),
                client_id: client_id.to_string(),
                active: true,
            },
            admission,
        ))
    }

    fn queue_on_tracker(
        &self,
        tracker: &mut AuthAttemptTracker,
        now: Instant,
    ) -> Result<Arc<Semaphore>, u64> {
        if let Some(lockout_until) = tracker.lockout_until {
            if now < lockout_until {
                let remaining = lockout_until.duration_since(now).as_secs().max(1);
                self.blocked_count.fetch_add(1, Ordering::Relaxed);
                return Err(remaining);
            }
            tracker.lockout_until = None;
        }

        let window_duration = Duration::from_secs(self.window_secs);
        if now.saturating_duration_since(tracker.window_start) > window_duration {
            tracker.failure_count = 0;
            tracker.generation = tracker.generation.wrapping_add(1);
            tracker.window_start = now;
        }

        if tracker.waiting >= MAX_QUEUED_ATTEMPTS_PER_CLIENT {
            self.blocked_count.fetch_add(1, Ordering::Relaxed);
            return Err(CAPACITY_RETRY_SECS);
        }
        tracker.waiting = tracker.waiting.saturating_add(1);
        Ok(Arc::clone(&tracker.admission))
    }

    fn activate_waiter(&self, client_id: &str) -> Result<u64, u64> {
        let now = Instant::now();
        let Some(mut entry) = self.attempts.get_mut(client_id) else {
            tracing::error!(
                client_id,
                "Authentication attempt waiter lost its tracker before admission"
            );
            return Err(CAPACITY_RETRY_SECS);
        };
        let tracker = entry.value_mut();
        let Some(waiting) = tracker.waiting.checked_sub(1) else {
            tracing::error!(
                client_id,
                "Authentication attempt waiter was admitted more than once"
            );
            return Err(CAPACITY_RETRY_SECS);
        };
        tracker.waiting = waiting;

        if let Some(lockout_until) = tracker.lockout_until {
            if now < lockout_until {
                let remaining = lockout_until.duration_since(now).as_secs().max(1);
                self.blocked_count.fetch_add(1, Ordering::Relaxed);
                return Err(remaining);
            }
            tracker.lockout_until = None;
        }

        let window_duration = Duration::from_secs(self.window_secs);
        if now.saturating_duration_since(tracker.window_start) > window_duration {
            tracker.failure_count = 0;
            tracker.generation = tracker.generation.wrapping_add(1);
            tracker.window_start = now;
        }
        let occupied_attempts = tracker.failure_count.saturating_add(tracker.in_flight);
        if occupied_attempts >= self.max_attempts.max(1) {
            self.blocked_count.fetch_add(1, Ordering::Relaxed);
            return Err(CAPACITY_RETRY_SECS);
        }
        tracker.in_flight = tracker.in_flight.saturating_add(1);
        Ok(tracker.generation)
    }

    fn cancel_waiter(&self, client_id: &str) {
        let Some(mut entry) = self.attempts.get_mut(client_id) else {
            tracing::error!(
                client_id,
                "Cancelled authentication attempt lost its tracker"
            );
            return;
        };
        let Some(waiting) = entry.waiting.checked_sub(1) else {
            tracing::error!(
                client_id,
                "Authentication attempt waiter was cancelled more than once"
            );
            return;
        };
        entry.waiting = waiting;
        let disposable = entry.is_disposable_success();
        drop(entry);
        if disposable {
            self.remove_disposable_success(client_id);
        }
    }

    fn finish_reservation(&self, client_id: &str, generation: u64, failed: bool) {
        let now = Instant::now();
        let Some(mut entry) = self.attempts.get_mut(client_id) else {
            tracing::error!(
                client_id,
                "Authentication attempt reservation lost its tracker before settlement"
            );
            return;
        };
        let tracker = entry.value_mut();

        let Some(in_flight) = tracker.in_flight.checked_sub(1) else {
            tracing::error!(
                client_id,
                "Authentication attempt reservation was settled more than once"
            );
            return;
        };
        tracker.in_flight = in_flight;

        // A separate admitted request may have established a lockout while
        // this credential check was in flight. Settle this reservation without
        // extending that active deadline or counting it in the next cycle.
        let should_count_failure = failed
            && tracker.generation == generation
            && tracker
                .lockout_until
                .is_none_or(|lockout_until| now >= lockout_until);
        let _lockout_secs = should_count_failure
            .then(|| self.apply_failure(client_id, tracker, now))
            .flatten();
        let disposable = !failed && tracker.is_disposable_success();
        let candidate = if disposable {
            None
        } else {
            tracker.mark_evictable(client_id, now)
        };
        drop(entry);
        self.enqueue_eviction_candidate(candidate);
        if disposable {
            self.remove_disposable_success(client_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn successful_concurrent_requests_queue_without_false_lockout() {
        const MAX_ATTEMPTS: usize = 5;
        const REQUESTS: usize = 32;
        let limiter = Arc::new(AuthRateLimiter::new(MAX_ATTEMPTS as u32, 300, 60));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::with_capacity(REQUESTS);

        for _ in 0..REQUESTS {
            let limiter = Arc::clone(&limiter);
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            tasks.push(tokio::spawn(async move {
                let reservation = limiter.reserve_attempt("shared-client").await.unwrap();
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                tokio::task::yield_now().await;
                active.fetch_sub(1, Ordering::SeqCst);
                reservation.release();
            }));
        }

        for task in tasks {
            task.await.unwrap();
        }
        assert!(peak.load(Ordering::SeqCst) <= MAX_ATTEMPTS);
        assert_eq!(limiter.blocked_attempts(), 0);
        assert!(limiter.is_empty());
    }

    #[tokio::test]
    async fn queued_guesses_stop_after_first_batch_establishes_lockout() {
        const MAX_ATTEMPTS: usize = 5;
        let limiter = Arc::new(AuthRateLimiter::new(MAX_ATTEMPTS as u32, 300, 60));
        let mut admitted = Vec::new();
        for _ in 0..MAX_ATTEMPTS {
            admitted.push(limiter.reserve_attempt("attacker").await.unwrap());
        }

        let queued_limiter = Arc::clone(&limiter);
        let queued = tokio::spawn(async move { queued_limiter.reserve_attempt("attacker").await });
        tokio::task::yield_now().await;
        assert!(!queued.is_finished());

        for reservation in admitted {
            reservation.record_failure();
        }
        let retry_after = match queued.await.unwrap() {
            Ok(_) => panic!("queued attacker request must be rejected after lockout"),
            Err(retry_after) => retry_after,
        };
        assert!((1..=60).contains(&retry_after));

        let tracker = limiter.attempts.get("attacker").unwrap();
        assert_eq!(tracker.in_flight, 0);
        assert_eq!(tracker.waiting, 0);
        assert_eq!(tracker.lockout_count, 1);
    }

    #[tokio::test]
    async fn queued_guess_cannot_replace_a_settled_failure_before_lockout() {
        let limiter = Arc::new(AuthRateLimiter::new(3, 300, 60));
        let first = limiter
            .reserve_attempt("interleaved-attacker")
            .await
            .unwrap();
        let second = limiter
            .reserve_attempt("interleaved-attacker")
            .await
            .unwrap();
        let third = limiter
            .reserve_attempt("interleaved-attacker")
            .await
            .unwrap();
        let queued_limiter = Arc::clone(&limiter);
        let queued =
            tokio::spawn(
                async move { queued_limiter.reserve_attempt("interleaved-attacker").await },
            );
        tokio::task::yield_now().await;
        assert!(!queued.is_finished());

        first.record_failure();
        assert_eq!(queued.await.unwrap().err(), Some(CAPACITY_RETRY_SECS));
        assert!(limiter.check_allowed("interleaved-attacker").is_ok());

        second.release();
        third.release();
        assert!(
            limiter
                .reserve_attempt("interleaved-attacker")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn successful_and_cancelled_attempts_release_without_lockout() {
        let limiter = Arc::new(AuthRateLimiter::new(2, 300, 60));
        let first = limiter.reserve_attempt("successful-client").await.unwrap();
        let cancelled = limiter.reserve_attempt("successful-client").await.unwrap();
        first.release();
        drop(cancelled);

        assert!(!limiter.attempts.contains_key("successful-client"));
        assert!(limiter.reserve_attempt("successful-client").await.is_ok());
    }

    #[tokio::test]
    async fn peerless_attempts_do_not_share_or_create_a_limiter_bucket() {
        let limiter = Arc::new(AuthRateLimiter::new(1, 300, 60));
        limiter
            .reserve_network_attempt(None)
            .await
            .unwrap()
            .record_failure();
        limiter
            .reserve_network_attempt(None)
            .await
            .unwrap()
            .record_failure();

        assert!(limiter.is_empty());
        assert_eq!(limiter.blocked_attempts(), 0);
    }

    #[tokio::test]
    async fn high_cardinality_successes_do_not_accumulate_trackers() {
        let limiter = Arc::new(AuthRateLimiter::with_max_entries(5, 300, 60, 2));
        for client in 0..100 {
            limiter
                .reserve_attempt(&format!("successful-client-{client}"))
                .await
                .unwrap()
                .release();
        }
        assert!(limiter.is_empty());
        assert!(limiter.eviction_candidates.lock().is_empty());
    }

    #[tokio::test]
    async fn active_trackers_obey_the_hard_capacity_limit() {
        let limiter = Arc::new(AuthRateLimiter::with_max_entries(5, 300, 60, 1));
        let first = limiter.reserve_attempt("first-client").await.unwrap();

        assert_eq!(
            limiter.reserve_attempt("second-client").await.err(),
            Some(CAPACITY_RETRY_SECS)
        );
        assert_eq!(limiter.len(), 1);
        assert!(limiter.attempts.contains_key("first-client"));

        first.release();
        assert!(limiter.is_empty());
    }

    #[tokio::test]
    async fn idle_failure_trackers_cannot_poison_new_identity_capacity() {
        let limiter = Arc::new(AuthRateLimiter::with_max_entries(5, 300, 60, 2));
        assert_eq!(limiter.record_failure("idle-first"), None);
        assert_eq!(limiter.record_failure("idle-second"), None);
        assert_eq!(limiter.len(), 2);

        let replacement = limiter.reserve_attempt("new-client").await.unwrap();
        assert_eq!(limiter.len(), 2);
        assert!(limiter.attempts.contains_key("new-client"));
        assert!(
            !limiter.attempts.contains_key("idle-first")
                || !limiter.attempts.contains_key("idle-second")
        );
        replacement.release();
    }

    #[tokio::test]
    async fn cancelled_waiter_does_not_leak_tracker_capacity() {
        let limiter = Arc::new(AuthRateLimiter::with_max_entries(1, 300, 60, 1));
        let active = limiter.reserve_attempt("client").await.unwrap();
        let waiting_limiter = Arc::clone(&limiter);
        let waiting = tokio::spawn(async move { waiting_limiter.reserve_attempt("client").await });
        tokio::task::yield_now().await;
        waiting.abort();
        assert!(waiting.await.is_err());
        active.release();

        assert!(limiter.is_empty());
        assert!(limiter.reserve_attempt("replacement").await.is_ok());
    }

    #[tokio::test]
    async fn late_reserved_failures_do_not_cross_into_a_new_lockout_generation() {
        let limiter = Arc::new(AuthRateLimiter::new(2, 300, 60));
        let first = limiter.reserve_attempt("late-client").await.unwrap();
        let second = limiter.reserve_attempt("late-client").await.unwrap();

        assert_eq!(limiter.record_failure("late-client"), None);
        assert_eq!(limiter.record_failure("late-client"), Some(60));
        let original_deadline = limiter
            .attempts
            .get("late-client")
            .and_then(|tracker| tracker.lockout_until)
            .unwrap();
        let expired_deadline = Instant::now() - Duration::from_secs(1);
        limiter
            .attempts
            .get_mut("late-client")
            .unwrap()
            .lockout_until = Some(expired_deadline);

        first.record_failure();
        second.record_failure();

        let tracker = limiter.attempts.get("late-client").unwrap();
        assert_eq!(tracker.in_flight, 0);
        assert_eq!(tracker.failure_count, 0);
        assert_eq!(tracker.lockout_count, 1);
        assert!(original_deadline > expired_deadline);
        assert_eq!(tracker.lockout_until, Some(expired_deadline));
    }

    #[tokio::test]
    async fn first_failure_after_success_starts_a_fresh_window() {
        let limiter = Arc::new(AuthRateLimiter::new(3, 300, 60));
        let success = limiter.reserve_attempt("window-client").await.unwrap();
        {
            let mut tracker = limiter.attempts.get_mut("window-client").unwrap();
            tracker.window_start = Instant::now() - Duration::from_secs(120);
        }
        success.release();

        let first_failure_started = Instant::now();
        limiter
            .reserve_attempt("window-client")
            .await
            .unwrap()
            .record_failure();
        let first_window = {
            let tracker = limiter.attempts.get("window-client").unwrap();
            assert_eq!(tracker.failure_count, 1);
            assert!(tracker.window_start >= first_failure_started);
            tracker.window_start
        };

        limiter
            .reserve_attempt("window-client")
            .await
            .unwrap()
            .record_failure();
        let tracker = limiter.attempts.get("window-client").unwrap();
        assert_eq!(tracker.failure_count, 2);
        assert_eq!(tracker.window_start, first_window);
    }

    #[tokio::test]
    async fn reservation_tolerates_tracker_created_with_later_timestamp() {
        let limiter = Arc::new(AuthRateLimiter::new(5, 300, 60));
        let client = "future-reservation-client";
        assert_eq!(limiter.record_failure(client), None);
        {
            let mut tracker = limiter.attempts.get_mut(client).unwrap();
            tracker.window_start = Instant::now() + Duration::from_secs(1);
        }

        limiter.reserve_attempt(client).await.unwrap().release();
        assert!(limiter.attempts.contains_key(client));
    }
}
