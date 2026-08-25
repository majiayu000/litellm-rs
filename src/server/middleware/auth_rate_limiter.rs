//! Authentication rate limiter for brute force protection

mod reservation;

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Default maximum number of client trackers retained by one limiter.
pub const DEFAULT_MAX_ENTRIES: usize = 10_000;

const CLEANUP_INTERVAL_SECS: u64 = 60;

/// Brute force protection for authentication endpoints
pub struct AuthRateLimiter {
    /// Map of client identifier -> tracker
    attempts: DashMap<String, AuthAttemptTracker>,
    /// Maximum failed attempts before lockout
    max_attempts: u32,
    /// Time window for counting failures (seconds)
    window_secs: u64,
    /// Lockout duration (seconds) - uses exponential backoff
    base_lockout_secs: u64,
    /// Maximum retained client trackers
    max_entries: usize,
    /// Total blocked attempts counter for monitoring
    blocked_count: AtomicU64,
}

/// Tracks authentication attempts for a single client
struct AuthAttemptTracker {
    failure_count: u32,
    in_flight: u32,
    window_start: Instant,
    lockout_until: Option<Instant>,
    lockout_count: u32,
}

impl Default for AuthRateLimiter {
    fn default() -> Self {
        Self::new(5, 300, 60)
    }
}

impl AuthRateLimiter {
    pub fn new(max_attempts: u32, window_secs: u64, base_lockout_secs: u64) -> Self {
        Self::with_max_entries(
            max_attempts,
            window_secs,
            base_lockout_secs,
            DEFAULT_MAX_ENTRIES,
        )
    }

    pub fn with_max_entries(
        max_attempts: u32,
        window_secs: u64,
        base_lockout_secs: u64,
        max_entries: usize,
    ) -> Self {
        Self {
            attempts: DashMap::new(),
            max_attempts,
            window_secs,
            base_lockout_secs,
            max_entries: max_entries.max(1),
            blocked_count: AtomicU64::new(0),
        }
    }

    pub fn check_allowed(&self, client_id: &str) -> Result<(), u64> {
        let now = Instant::now();

        let Some(mut entry) = self.attempts.get_mut(client_id) else {
            return Ok(());
        };
        let tracker = entry.value_mut();

        if let Some(lockout_until) = tracker.lockout_until {
            if now < lockout_until {
                let remaining = lockout_until.duration_since(now).as_secs();
                self.blocked_count.fetch_add(1, Ordering::Relaxed);
                return Err(remaining);
            }
            tracker.lockout_until = None;
        }

        let window_duration = Duration::from_secs(self.window_secs);
        if now.duration_since(tracker.window_start) > window_duration {
            tracker.failure_count = 0;
            tracker.window_start = now;
        }

        Ok(())
    }

    pub fn record_failure(&self, client_id: &str) -> Option<u64> {
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
        let lockout_secs = self.apply_failure(client_id, tracker, now);

        drop(entry);
        self.enforce_capacity(now);
        lockout_secs
    }

    pub fn record_success(&self, client_id: &str) {
        if let Some(mut entry) = self.attempts.get_mut(client_id) {
            entry.failure_count = 0;
        }
    }

    pub fn blocked_attempts(&self) -> u64 {
        self.blocked_count.load(Ordering::Relaxed)
    }

    pub fn len(&self) -> usize {
        self.attempts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.attempts.is_empty()
    }

    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    pub fn cleanup_old_entries(&self) {
        let now = Instant::now();
        self.cleanup_old_entries_at(now);
        self.enforce_capacity(now);
    }

    fn cleanup_old_entries_at(&self, now: Instant) {
        let max_age = Duration::from_secs(self.window_secs.saturating_mul(2));
        self.attempts.retain(|_, tracker| {
            tracker.in_flight > 0
                || now.duration_since(tracker.window_start) < max_age
                || tracker.lockout_until.is_some_and(|until| until > now)
        });
    }

    fn apply_failure(
        &self,
        client_id: &str,
        tracker: &mut AuthAttemptTracker,
        now: Instant,
    ) -> Option<u64> {
        // A direct failure may establish a lockout while an admitted request
        // is still authenticating. Settle that reservation without extending
        // the active deadline or counting it again in the next cycle.
        if tracker
            .lockout_until
            .is_some_and(|lockout_until| now < lockout_until)
        {
            return None;
        }

        if tracker.failure_count == 0 {
            tracker.window_start = now;
        }
        tracker.failure_count = tracker.failure_count.saturating_add(1);
        if tracker.failure_count < self.max_attempts {
            return None;
        }

        let lockout_multiplier = 2u64.pow(tracker.lockout_count);
        let lockout_secs = self.base_lockout_secs.saturating_mul(lockout_multiplier);
        let lockout_duration = Duration::from_secs(lockout_secs);

        tracker.lockout_until = Some(now + lockout_duration);
        tracker.lockout_count += 1;
        tracker.failure_count = 0;

        tracing::warn!(
            "Client {} locked out for {} seconds (lockout #{})",
            client_id,
            lockout_secs,
            tracker.lockout_count
        );

        Some(lockout_secs)
    }

    fn enforce_capacity(&self, now: Instant) {
        let overflow = self.attempts.len().saturating_sub(self.max_entries);
        if overflow == 0 {
            return;
        }

        let mut candidates: Vec<_> = self
            .attempts
            .iter()
            .filter_map(|entry| {
                let tracker = entry.value();
                if tracker.in_flight > 0 {
                    return None;
                }
                let locked = tracker.lockout_until.is_some_and(|until| until > now);
                Some((locked, tracker.window_start, entry.key().clone()))
            })
            .collect();
        candidates.sort_by_key(|(locked, window_start, _)| (*locked, *window_start));

        for (_, _, client_id) in candidates.into_iter().take(overflow) {
            self.attempts.remove(&client_id);
        }
    }
}

/// Global auth rate limiter
static AUTH_RATE_LIMITER: std::sync::OnceLock<Arc<AuthRateLimiter>> = std::sync::OnceLock::new();
static AUTH_RATE_LIMITER_CLEANUP: std::sync::OnceLock<()> = std::sync::OnceLock::new();

/// Get or initialize the global auth rate limiter
pub fn get_auth_rate_limiter() -> Arc<AuthRateLimiter> {
    AUTH_RATE_LIMITER
        .get_or_init(|| Arc::new(AuthRateLimiter::default()))
        .clone()
}

/// Start the global limiter cleanup loop once per process.
pub fn start_auth_rate_limiter_cleanup_task() {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::warn!("Auth rate limiter cleanup task was not started: no Tokio runtime");
        return;
    };

    if AUTH_RATE_LIMITER_CLEANUP.set(()).is_err() {
        return;
    }

    let limiter = get_auth_rate_limiter();
    handle.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));
        loop {
            interval.tick().await;
            limiter.cleanup_old_entries();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    // ==================== Constructor Tests ====================

    #[test]
    fn test_auth_rate_limiter_new() {
        let limiter = AuthRateLimiter::new(10, 600, 120);
        assert_eq!(limiter.max_attempts, 10);
        assert_eq!(limiter.window_secs, 600);
        assert_eq!(limiter.base_lockout_secs, 120);
        assert_eq!(limiter.max_entries, DEFAULT_MAX_ENTRIES);
        assert_eq!(limiter.blocked_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_auth_rate_limiter_default() {
        let limiter = AuthRateLimiter::default();
        assert_eq!(limiter.max_attempts, 5);
        assert_eq!(limiter.window_secs, 300);
        assert_eq!(limiter.base_lockout_secs, 60);
        assert_eq!(limiter.max_entries(), DEFAULT_MAX_ENTRIES);
    }

    #[test]
    fn test_auth_rate_limiter_with_max_entries() {
        let limiter = AuthRateLimiter::with_max_entries(5, 300, 60, 7);
        assert_eq!(limiter.max_entries(), 7);
    }

    // ==================== check_allowed Tests ====================

    #[test]
    fn test_check_allowed_new_client() {
        let limiter = AuthRateLimiter::new(5, 300, 60);
        let result = limiter.check_allowed("new_client");
        assert!(result.is_ok());
        assert_eq!(limiter.len(), 0);
        assert!(limiter.is_empty());
    }

    #[test]
    fn test_check_allowed_after_failures_below_threshold() {
        let limiter = AuthRateLimiter::new(5, 300, 60);
        let client = "test_client";

        // Record 4 failures (below threshold of 5)
        for _ in 0..4 {
            limiter.record_failure(client);
        }

        // Should still be allowed
        let result = limiter.check_allowed(client);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_allowed_blocked_after_max_attempts() {
        let limiter = AuthRateLimiter::new(3, 300, 60);
        let client = "locked_client";

        // Trigger lockout by reaching max attempts
        for _ in 0..3 {
            limiter.record_failure(client);
        }

        // Should be blocked
        let result = limiter.check_allowed(client);
        assert!(result.is_err());

        // Should return remaining lockout time
        let remaining = result.unwrap_err();
        assert!(remaining > 0);
        assert!(remaining <= 60);
    }

    // ==================== record_failure Tests ====================

    #[test]
    fn test_record_failure_increments_count() {
        let limiter = AuthRateLimiter::new(5, 300, 60);
        let client = "failure_client";

        // First 4 failures should not trigger lockout
        for _ in 0..4 {
            let result = limiter.record_failure(client);
            assert!(result.is_none());
        }
    }

    #[test]
    fn test_record_failure_triggers_lockout() {
        let limiter = AuthRateLimiter::new(3, 300, 60);
        let client = "lockout_client";

        // Record failures up to threshold
        limiter.record_failure(client);
        limiter.record_failure(client);
        let result = limiter.record_failure(client);

        // Third failure should trigger lockout
        assert!(result.is_some());
        assert_eq!(result.unwrap(), 60); // Base lockout
    }

    #[test]
    fn test_record_failure_does_not_escalate_active_lockout() {
        let limiter = AuthRateLimiter::new(1, 300, 60);
        let client = "active_lockout_client";

        assert_eq!(limiter.record_failure(client), Some(60));
        let (original_deadline, original_lockout_count) = {
            let entry = limiter.attempts.get(client).unwrap();
            (entry.lockout_until.unwrap(), entry.lockout_count)
        };

        for _ in 0..10 {
            assert_eq!(limiter.record_failure(client), None);
        }

        let entry = limiter.attempts.get(client).unwrap();
        assert_eq!(entry.lockout_until, Some(original_deadline));
        assert_eq!(entry.lockout_count, original_lockout_count);
        assert_eq!(entry.failure_count, 0);
    }

    #[test]
    fn test_record_failure_exponential_backoff() {
        let limiter = AuthRateLimiter::new(2, 300, 60);
        let client = "backoff_client";

        // First lockout: base * 2^0 = 60
        limiter.record_failure(client);
        let first = limiter.record_failure(client);
        assert_eq!(first, Some(60));

        // Simulate expiry without sleeping. Expired lockouts must still allow
        // the next failure cycle to apply exponential backoff.
        // The lockout_count should now be 1, so next lockout = 60 * 2^1 = 120
        if let Some(mut entry) = limiter.attempts.get_mut(client) {
            entry.lockout_until = Some(Instant::now() - Duration::from_secs(1));
        }

        limiter.record_failure(client);
        let second = limiter.record_failure(client);
        assert_eq!(second, Some(120)); // 60 * 2^1

        // Third lockout: 60 * 2^2 = 240
        if let Some(mut entry) = limiter.attempts.get_mut(client) {
            entry.lockout_until = Some(Instant::now() - Duration::from_secs(1));
        }

        limiter.record_failure(client);
        let third = limiter.record_failure(client);
        assert_eq!(third, Some(240)); // 60 * 2^2
    }

    // ==================== record_success Tests ====================

    #[test]
    fn test_record_success_resets_failure_count() {
        let limiter = AuthRateLimiter::new(5, 300, 60);
        let client = "success_client";

        // Record some failures
        limiter.record_failure(client);
        limiter.record_failure(client);

        // Record success
        limiter.record_success(client);

        // Verify failure count is reset by checking we can fail 4 more times without lockout
        for _ in 0..4 {
            let result = limiter.record_failure(client);
            assert!(result.is_none());
        }
    }

    #[test]
    fn test_record_success_nonexistent_client() {
        let limiter = AuthRateLimiter::new(5, 300, 60);

        // Should not panic for non-existent client
        limiter.record_success("nonexistent");
    }

    // ==================== blocked_attempts Tests ====================

    #[test]
    fn test_blocked_attempts_counter() {
        let limiter = AuthRateLimiter::new(2, 300, 60);
        let client = "blocked_client";

        assert_eq!(limiter.blocked_attempts(), 0);

        // Trigger lockout
        limiter.record_failure(client);
        limiter.record_failure(client);

        // Try to check while locked out
        let _ = limiter.check_allowed(client);
        assert_eq!(limiter.blocked_attempts(), 1);

        // Try again
        let _ = limiter.check_allowed(client);
        assert_eq!(limiter.blocked_attempts(), 2);
    }

    // ==================== cleanup_old_entries Tests ====================

    #[test]
    fn test_cleanup_old_entries_empty() {
        let limiter = AuthRateLimiter::new(5, 300, 60);
        limiter.cleanup_old_entries();
        // Should not panic on empty map
    }

    #[test]
    fn test_cleanup_retains_active_lockouts() {
        let limiter = AuthRateLimiter::new(2, 1, 60);
        let client = "active_lockout";

        // Trigger lockout
        limiter.record_failure(client);
        limiter.record_failure(client);

        // Cleanup should retain entry with active lockout
        limiter.cleanup_old_entries();

        // Entry should still exist
        assert!(limiter.attempts.contains_key(client));
    }

    #[test]
    fn test_cleanup_removes_expired_entries() {
        let limiter = AuthRateLimiter::new(5, 1, 60);
        let client = "expired_client";

        limiter.record_failure(client);
        if let Some(mut entry) = limiter.attempts.get_mut(client) {
            entry.window_start = Instant::now() - Duration::from_secs(3);
        }

        limiter.cleanup_old_entries();
        assert!(!limiter.attempts.contains_key(client));
    }

    #[test]
    fn test_max_entries_cap_enforced() {
        let limiter = AuthRateLimiter::with_max_entries(5, 300, 60, 3);

        for i in 0..10 {
            limiter.record_failure(&format!("client_{i}"));
        }

        assert!(limiter.len() <= 3);
    }

    #[test]
    fn test_capacity_prefers_retaining_active_lockouts() {
        let limiter = AuthRateLimiter::with_max_entries(2, 300, 60, 2);

        limiter.record_failure("old_1");
        limiter.record_failure("old_2");
        limiter.record_failure("locked_client");
        limiter.record_failure("locked_client");

        assert!(limiter.len() <= 2);
        assert!(limiter.attempts.contains_key("locked_client"));
    }

    // ==================== Window Reset Tests ====================

    #[test]
    fn test_window_reset_clears_failure_count() {
        // Use very short window for testing
        let limiter = AuthRateLimiter::new(5, 1, 60);
        let client = "window_client";

        // Record failures
        limiter.record_failure(client);
        limiter.record_failure(client);

        // Wait for window to expire
        thread::sleep(Duration::from_millis(1100));

        // Check should reset the window
        let result = limiter.check_allowed(client);
        assert!(result.is_ok());

        // Now we should be able to fail again without previous count
        for _ in 0..4 {
            let result = limiter.record_failure(client);
            assert!(result.is_none());
        }
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_zero_max_attempts() {
        let limiter = AuthRateLimiter::new(0, 300, 60);
        let client = "zero_attempts";

        // Even first check should trigger lockout after first failure
        // Actually max_attempts = 0 means first failure (count=1) >= 0 is true
        // But failure_count starts at 0 and increments to 1
        // 1 >= 0 is true, so immediate lockout
        let result = limiter.record_failure(client);
        assert!(result.is_some());
    }

    #[test]
    fn test_multiple_clients_independent() {
        let limiter = AuthRateLimiter::new(3, 300, 60);

        // Lock out client1
        for _ in 0..3 {
            limiter.record_failure("client1");
        }

        // client1 should be blocked
        assert!(limiter.check_allowed("client1").is_err());

        // client2 should still be allowed
        assert!(limiter.check_allowed("client2").is_ok());
    }

    #[test]
    fn test_lockout_expiry() {
        // Very short lockout for testing
        let limiter = AuthRateLimiter::new(2, 300, 1);
        let client = "expiry_client";

        // Trigger lockout
        limiter.record_failure(client);
        limiter.record_failure(client);

        // Should be blocked
        assert!(limiter.check_allowed(client).is_err());

        // Wait for lockout to expire
        thread::sleep(Duration::from_millis(1100));

        // Should be allowed again
        assert!(limiter.check_allowed(client).is_ok());
    }

    // ==================== Concurrency Tests ====================

    #[test]
    fn test_concurrent_access() {
        let limiter = Arc::new(AuthRateLimiter::new(100, 300, 60));
        let mut handles = vec![];

        for i in 0..10 {
            let limiter = Arc::clone(&limiter);
            let handle = thread::spawn(move || {
                let client = format!("client_{}", i);
                for _ in 0..10 {
                    let _ = limiter.check_allowed(&client);
                    limiter.record_failure(&client);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have 10 entries (one per client)
        assert_eq!(limiter.attempts.len(), 10);
    }

    #[test]
    fn test_same_client_concurrent_failures() {
        let limiter = Arc::new(AuthRateLimiter::new(50, 300, 60));
        let mut handles = vec![];

        for _ in 0..5 {
            let limiter = Arc::clone(&limiter);
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    limiter.record_failure("shared_client");
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have exactly one entry for the shared client
        assert_eq!(limiter.attempts.len(), 1);
    }

    #[test]
    fn test_concurrent_late_failures_do_not_escalate_active_lockout() {
        const THREADS: usize = 8;
        let limiter = Arc::new(AuthRateLimiter::new(1, 300, 60));
        let client = "concurrent_active_lockout_client";
        assert_eq!(limiter.record_failure(client), Some(60));
        let original_deadline = limiter
            .attempts
            .get(client)
            .and_then(|entry| entry.lockout_until)
            .unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(THREADS + 1));
        let mut handles = Vec::with_capacity(THREADS);

        for _ in 0..THREADS {
            let limiter = Arc::clone(&limiter);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                for _ in 0..10 {
                    assert_eq!(limiter.record_failure(client), None);
                }
            }));
        }

        barrier.wait();
        for handle in handles {
            handle.join().unwrap();
        }

        let entry = limiter.attempts.get(client).unwrap();
        assert_eq!(entry.lockout_until, Some(original_deadline));
        assert_eq!(entry.lockout_count, 1);
        assert_eq!(entry.failure_count, 0);
    }
}
