use super::{AuthAttemptTracker, AuthRateLimiter};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const IN_FLIGHT_RETRY_SECS: u64 = 1;

/// One authentication attempt admitted by [`AuthRateLimiter`].
///
/// Dropping a live reservation releases its in-flight slot, so cancellation
/// and infrastructure errors cannot leak limiter capacity.
#[must_use = "the authentication attempt reservation must be settled or retained"]
pub(crate) struct AuthAttemptReservation {
    limiter: Arc<AuthRateLimiter>,
    client_id: String,
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

        self.limiter.finish_reservation(&self.client_id, failed);
    }
}

impl Drop for AuthAttemptReservation {
    fn drop(&mut self) {
        self.finish(false);
    }
}

impl AuthRateLimiter {
    /// Atomically reserves one place in the failed-attempt window before
    /// asynchronous credential verification begins.
    pub(crate) fn reserve_attempt(
        self: &Arc<Self>,
        client_id: &str,
    ) -> Result<AuthAttemptReservation, u64> {
        let now = Instant::now();

        let mut entry = self
            .attempts
            .entry(client_id.to_string())
            .or_insert_with(|| AuthAttemptTracker {
                failure_count: 0,
                in_flight: 0,
                window_start: now,
                lockout_until: None,
                lockout_count: 0,
            });
        let tracker = entry.value_mut();

        if let Some(lockout_until) = tracker.lockout_until {
            if now < lockout_until {
                let remaining = lockout_until.duration_since(now).as_secs().max(1);
                self.blocked_count.fetch_add(1, Ordering::Relaxed);
                drop(entry);
                self.enforce_capacity(now);
                return Err(remaining);
            }
            tracker.lockout_until = None;
        }

        let window_duration = Duration::from_secs(self.window_secs);
        if now.duration_since(tracker.window_start) > window_duration {
            tracker.failure_count = 0;
            tracker.window_start = now;
        }

        let occupied_attempts = tracker.failure_count.saturating_add(tracker.in_flight);
        if occupied_attempts >= self.max_attempts.max(1) {
            self.blocked_count.fetch_add(1, Ordering::Relaxed);
            drop(entry);
            self.enforce_capacity(now);
            return Err(IN_FLIGHT_RETRY_SECS);
        }

        tracker.in_flight = tracker.in_flight.saturating_add(1);
        drop(entry);
        self.enforce_capacity(now);

        Ok(AuthAttemptReservation {
            limiter: Arc::clone(self),
            client_id: client_id.to_string(),
            active: true,
        })
    }

    fn finish_reservation(&self, client_id: &str, failed: bool) {
        let now = Instant::now();
        let Some(mut entry) = self.attempts.get_mut(client_id) else {
            tracing::error!(
                client_id = client_id,
                "Authentication attempt reservation lost its tracker before settlement"
            );
            return;
        };
        let tracker = entry.value_mut();

        let Some(in_flight) = tracker.in_flight.checked_sub(1) else {
            tracing::error!(
                client_id = client_id,
                "Authentication attempt reservation was settled more than once"
            );
            return;
        };
        tracker.in_flight = in_flight;

        // A separate admitted request may have established a lockout while
        // this credential check was in flight. Settle this reservation without
        // extending that active deadline or counting it in the next cycle.
        let should_count_failure = failed
            && tracker
                .lockout_until
                .is_none_or(|lockout_until| now >= lockout_until);
        let _lockout_secs = should_count_failure
            .then(|| self.apply_failure(client_id, tracker, now))
            .flatten();
        drop(entry);
        if !failed {
            self.remove_disposable_success(client_id);
        }
        self.enforce_capacity(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[test]
    fn concurrent_reservations_bound_credential_checks_before_lockout() {
        const MAX_ATTEMPTS: usize = 5;
        const REQUESTS: usize = 32;
        let limiter = Arc::new(AuthRateLimiter::new(MAX_ATTEMPTS as u32, 300, 60));
        let barrier = Arc::new(Barrier::new(REQUESTS + 1));
        let admitted = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(REQUESTS);

        for _ in 0..REQUESTS {
            let limiter = Arc::clone(&limiter);
            let barrier = Arc::clone(&barrier);
            let admitted = Arc::clone(&admitted);
            handles.push(thread::spawn(move || {
                let reservation = limiter.reserve_attempt("shared-client").ok();
                if reservation.is_some() {
                    admitted.fetch_add(1, Ordering::Relaxed);
                }
                barrier.wait();
                if let Some(reservation) = reservation {
                    reservation.record_failure();
                }
            }));
        }

        barrier.wait();
        assert_eq!(admitted.load(Ordering::Relaxed), MAX_ATTEMPTS);
        assert_eq!(limiter.blocked_attempts(), (REQUESTS - MAX_ATTEMPTS) as u64);

        for handle in handles {
            handle.join().unwrap();
        }

        let tracker = limiter.attempts.get("shared-client").unwrap();
        assert_eq!(tracker.in_flight, 0);
        assert_eq!(tracker.failure_count, 0);
        assert_eq!(tracker.lockout_count, 1);
        assert!(
            tracker
                .lockout_until
                .is_some_and(|until| until > Instant::now())
        );
    }

    #[test]
    fn successful_and_cancelled_attempts_release_without_lockout() {
        let limiter = Arc::new(AuthRateLimiter::new(2, 300, 60));
        let first = limiter.reserve_attempt("successful-client").unwrap();
        let cancelled = limiter.reserve_attempt("successful-client").unwrap();

        assert_eq!(limiter.reserve_attempt("successful-client").err(), Some(1));
        first.release();
        drop(cancelled);

        assert!(!limiter.attempts.contains_key("successful-client"));
        assert!(limiter.reserve_attempt("successful-client").is_ok());
    }

    #[test]
    fn high_cardinality_successes_do_not_accumulate_trackers() {
        let limiter = Arc::new(AuthRateLimiter::with_max_entries(5, 300, 60, 2));

        for client in 0..100 {
            limiter
                .reserve_attempt(&format!("successful-client-{client}"))
                .unwrap()
                .release();
        }

        assert!(limiter.is_empty());
    }

    #[test]
    fn late_reserved_failures_do_not_extend_an_active_lockout() {
        let limiter = Arc::new(AuthRateLimiter::new(2, 300, 60));
        let first = limiter.reserve_attempt("late-client").unwrap();
        let second = limiter.reserve_attempt("late-client").unwrap();

        assert_eq!(limiter.record_failure("late-client"), None);
        assert_eq!(limiter.record_failure("late-client"), Some(60));
        let original_deadline = limiter
            .attempts
            .get("late-client")
            .and_then(|tracker| tracker.lockout_until)
            .unwrap();

        first.record_failure();
        second.record_failure();

        let tracker = limiter.attempts.get("late-client").unwrap();
        assert_eq!(tracker.in_flight, 0);
        assert_eq!(tracker.failure_count, 0);
        assert_eq!(tracker.lockout_count, 1);
        assert_eq!(tracker.lockout_until, Some(original_deadline));
    }

    #[test]
    fn capacity_eviction_cannot_remove_a_newly_active_tracker() {
        let limiter = Arc::new(AuthRateLimiter::with_max_entries(5, 300, 60, 1));
        assert_eq!(limiter.record_failure("candidate"), None);
        let snapshot = limiter
            .attempts
            .get("candidate")
            .and_then(|tracker| tracker.eviction_snapshot())
            .unwrap();
        let (reserved_tx, reserved_rx) = std::sync::mpsc::channel();
        let (settle_tx, settle_rx) = std::sync::mpsc::channel();
        let worker_limiter = Arc::clone(&limiter);
        let worker = thread::spawn(move || {
            let reservation = worker_limiter.reserve_attempt("candidate").unwrap();
            reserved_tx.send(()).unwrap();
            settle_rx.recv().unwrap();
            reservation.record_failure();
        });

        reserved_rx.recv().unwrap();
        assert!(!limiter.remove_if_unchanged("candidate", snapshot));
        assert_eq!(limiter.attempts.get("candidate").unwrap().in_flight, 1);

        settle_tx.send(()).unwrap();
        worker.join().unwrap();
        let tracker = limiter.attempts.get("candidate").unwrap();
        assert_eq!(tracker.in_flight, 0);
        assert_eq!(tracker.failure_count, 2);
    }

    #[test]
    fn first_failure_after_success_starts_a_fresh_window() {
        let limiter = Arc::new(AuthRateLimiter::new(3, 300, 60));
        let success = limiter.reserve_attempt("window-client").unwrap();
        {
            let mut tracker = limiter.attempts.get_mut("window-client").unwrap();
            tracker.window_start = Instant::now() - Duration::from_secs(120);
        }
        success.release();

        let first_failure_started = Instant::now();
        limiter
            .reserve_attempt("window-client")
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
            .unwrap()
            .record_failure();
        let tracker = limiter.attempts.get("window-client").unwrap();
        assert_eq!(tracker.failure_count, 2);
        assert_eq!(tracker.window_start, first_window);
    }

    #[test]
    fn capacity_cleanup_retains_in_flight_trackers_until_release() {
        let limiter = Arc::new(AuthRateLimiter::with_max_entries(5, 300, 60, 1));
        let first = limiter.reserve_attempt("first-client").unwrap();
        let second = limiter.reserve_attempt("second-client").unwrap();

        limiter.cleanup_old_entries();
        assert_eq!(limiter.len(), 2);

        first.release();
        assert!(limiter.attempts.contains_key("second-client"));
        second.release();
        assert!(limiter.len() <= limiter.max_entries());
    }
}
