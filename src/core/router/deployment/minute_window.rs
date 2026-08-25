use super::*;

const MINUTE_WINDOW_SECS: u64 = 60;

struct MinuteResetGuard<'a>(&'a AtomicBool);

impl Drop for MinuteResetGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl DeploymentState {
    /// Reset per-minute counters.
    pub fn reset_minute(&self) {
        let _guard = self.acquire_minute_reset();
        self.finish_minute_reset(current_timestamp());
    }

    pub(in crate::core::router) fn reset_minute_if_elapsed(&self) -> u64 {
        let (now, ()) = self.with_current_minute(|_| ());
        now
    }

    pub(super) fn with_current_minute<T>(
        &self,
        operation: impl FnOnce(&DeploymentStateInner) -> T,
    ) -> (u64, T) {
        let now = current_timestamp();
        let _guard = self.acquire_minute_reset();
        let reset_at = self.minute_reset_at.load(Ordering::Acquire);
        if now < reset_at || now - reset_at >= MINUTE_WINDOW_SECS {
            self.finish_minute_reset(now);
        }
        (now, operation(&self.inner))
    }

    pub(in crate::core::router) fn minute_counters(&self) -> (u64, u64, u32) {
        self.with_current_minute(|state| {
            (
                state.tpm_current.load(Ordering::Relaxed),
                state.rpm_current.load(Ordering::Relaxed),
                state.fails_this_minute.load(Ordering::Relaxed),
            )
        })
        .1
    }

    fn acquire_minute_reset(&self) -> MinuteResetGuard<'_> {
        let mut spins = 0;
        loop {
            if self
                .reset_in_progress
                .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return MinuteResetGuard(&self.reset_in_progress);
            }
            if spins < 8 {
                std::hint::spin_loop();
                spins += 1;
            } else {
                std::thread::yield_now();
                spins = 0;
            }
        }
    }

    fn finish_minute_reset(&self, now: u64) {
        self.tpm_current.store(0, Ordering::Relaxed);
        self.rpm_current.store(0, Ordering::Relaxed);
        self.fails_this_minute.store(0, Ordering::Relaxed);
        self.minute_reset_at.store(now, Ordering::Release);
    }
}
