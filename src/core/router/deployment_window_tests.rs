//! Complementary tests for lazy per-minute window rolling.
//!
//! The elapsed-window reset case lives in `deployment.rs` (`mod tests`).
//! Production never starts a background reset task; readers and writers of
//! the per-minute counters call [`super::DeploymentStateInner::roll_minute_window`]
//! before touching them so TPM/RPM limits and cooldown windows keep their
//! per-minute semantics over the process lifetime.

use super::{DeploymentState, current_timestamp};
use std::sync::atomic::Ordering;

#[test]
fn active_window_preserves_per_minute_counters() {
    let state = DeploymentState::new();
    state.tpm_current.store(1000, Ordering::Relaxed);
    state.rpm_current.store(50, Ordering::Relaxed);
    state.fails_this_minute.store(5, Ordering::Relaxed);

    // `DeploymentState::new` starts an active window; rolling must not
    // reset anything while it has not elapsed.
    state.roll_minute_window();

    assert_eq!(state.tpm_current.load(Ordering::Relaxed), 1000);
    assert_eq!(state.rpm_current.load(Ordering::Relaxed), 50);
    assert_eq!(state.fails_this_minute.load(Ordering::Relaxed), 5);
}

#[test]
fn roll_is_idempotent_within_one_window() {
    let state = DeploymentState::new();
    state.tpm_current.store(10, Ordering::Relaxed);

    let stale = current_timestamp().saturating_sub(61);
    state.minute_reset_at.store(stale, Ordering::Relaxed);

    state.roll_minute_window();
    state.roll_minute_window();

    assert_eq!(state.tpm_current.load(Ordering::Relaxed), 0);
    assert!(state.minute_reset_at.load(Ordering::Relaxed) >= stale + 60);
}
