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
        self.cleanup_old_entries_at(now);

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

        let _lockout_secs = failed
            .then(|| self.apply_failure(client_id, tracker, now))
            .flatten();
        drop(entry);
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

        let tracker = limiter.attempts.get("successful-client").unwrap();
        assert_eq!(tracker.in_flight, 0);
        assert_eq!(tracker.failure_count, 0);
        assert_eq!(tracker.lockout_until, None);
        drop(tracker);
        assert!(limiter.reserve_attempt("successful-client").is_ok());
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
