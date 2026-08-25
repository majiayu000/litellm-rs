use super::{Deployment, HealthStatus};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[derive(Debug)]
pub(super) struct ProbeLifecycle {
    generation: Mutex<u64>,
}

impl ProbeLifecycle {
    pub(super) fn new() -> Self {
        Self {
            generation: Mutex::new(0),
        }
    }

    pub(super) fn next_generation(&self) -> u64 {
        let mut generation = self.generation.lock();
        *generation = generation
            .checked_add(1)
            .expect("probe generation space exhausted");
        *generation
    }
}

impl Deployment {
    pub(crate) fn probe_generation(&self) -> u64 {
        self.state.probe_generation
    }

    #[cfg(test)]
    pub(crate) fn publish_probe_health(&self, target: HealthStatus) -> bool {
        let generation = self.state.probe_lifecycle.generation.lock();
        if *generation != self.state.probe_generation {
            return false;
        }
        self.publish_probe_health_unchecked(target);
        true
    }

    fn publish_probe_health_unchecked(&self, target: HealthStatus) {
        self.state.set_probe_health_status(target);
        self.state
            .probe_unhealthy
            .store(target == HealthStatus::Unhealthy, Ordering::Relaxed);
        if self.is_in_cooldown() {
            return;
        }

        let mut current = self.state.health.load(Ordering::Relaxed);
        loop {
            let current_status = HealthStatus::from(current);
            let next = match (target, current_status) {
                (_, HealthStatus::Cooldown) => return,
                (HealthStatus::Degraded, HealthStatus::Unhealthy) => return,
                (HealthStatus::Healthy, _)
                | (HealthStatus::Degraded, _)
                | (HealthStatus::Unhealthy, _) => target,
                (HealthStatus::Unknown | HealthStatus::Cooldown, _) => return,
            };
            if current_status == next {
                return;
            }

            match self.state.health.compare_exchange_weak(
                current,
                next as u8,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }
}

/// Atomically validate and publish a group result against every deployment's
/// lifecycle. Replacement advances the same lifecycle before snapshot install,
/// so either the whole old group publishes first or none of it publishes.
pub(crate) fn publish_probe_group(deployments: &[Arc<Deployment>], target: HealthStatus) -> bool {
    let mut unique = Vec::with_capacity(deployments.len());
    let mut seen = HashSet::with_capacity(deployments.len());
    for deployment in deployments {
        let identity = Arc::as_ptr(&deployment.state.probe_lifecycle) as usize;
        if seen.insert(identity) {
            unique.push((identity, deployment.as_ref()));
        }
    }
    unique.sort_unstable_by_key(|(identity, _)| *identity);

    let guards = unique
        .iter()
        .map(|(_, deployment)| deployment.state.probe_lifecycle.generation.lock())
        .collect::<Vec<_>>();
    if unique
        .iter()
        .zip(&guards)
        .any(|((_, deployment), generation)| **generation != deployment.state.probe_generation)
        || deployments.iter().any(|deployment| {
            let lifecycle = Arc::as_ptr(&deployment.state.probe_lifecycle) as usize;
            unique
                .iter()
                .zip(&guards)
                .any(|((identity, _), generation)| {
                    *identity == lifecycle && **generation != deployment.state.probe_generation
                })
        })
    {
        return false;
    }

    for deployment in deployments {
        deployment.publish_probe_health_unchecked(target);
    }
    true
}
