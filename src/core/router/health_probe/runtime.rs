use super::*;
use crate::core::providers::NativeHealthProbeSemantics;
use crate::core::router::deployment::{ProviderInstanceIdentity, publish_probe_group};
use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::future::Future;
use std::pin::Pin;
use tokio::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ProbeScope {
    CustomEndpoint,
    NativeProvider(ProviderInstanceIdentity),
    NativeModel {
        provider_instance: ProviderInstanceIdentity,
        model: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DynamicProbeKey {
    scope: ProbeScope,
    provider_name: String,
    provider_kind: String,
    policy: HealthCheckPolicy,
    timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeMember {
    deployment_id: String,
    deployment_instance: usize,
    probe_generation: u64,
}

#[derive(Clone)]
struct ProbeTarget {
    key: DynamicProbeKey,
    provider: Provider,
    deployments: Vec<Arc<Deployment>>,
}

impl ProbeTarget {
    fn members(&self) -> Vec<ProbeMember> {
        let mut members = self
            .deployments
            .iter()
            .map(|deployment| ProbeMember {
                deployment_id: deployment.id.clone(),
                deployment_instance: Arc::as_ptr(deployment) as usize,
                probe_generation: deployment.probe_generation(),
            })
            .collect::<Vec<_>>();
        members.sort_unstable_by(|left, right| left.deployment_id.cmp(&right.deployment_id));
        members
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProbeRun {
    incarnation: u64,
    epoch: u64,
}

#[derive(Debug)]
struct ProbeSchedule {
    consecutive_failures: u32,
    next_run: Instant,
    running: Option<ProbeRun>,
    incarnation: u64,
    epoch: u64,
    members: Vec<ProbeMember>,
}

struct ProbeOutcome {
    key: DynamicProbeKey,
    run: ProbeRun,
    target: ProbeTarget,
    result: Result<(), ProbeFailure>,
}

type PendingProbe = Pin<Box<dyn Future<Output = ProbeOutcome> + Send>>;

pub(super) fn validate_probe_snapshot(snapshot: &RoutingSnapshot) -> Result<usize, RouterError> {
    let mut provider_policies = HashMap::new();
    let targets = current_probe_groups(snapshot);
    for target in &targets {
        match provider_policies.entry(target.key.provider_name.clone()) {
            Entry::Vacant(entry) => {
                entry.insert((target.key.policy.clone(), target.key.timeout_secs));
            }
            Entry::Occupied(entry)
                if entry.get() != &(target.key.policy.clone(), target.key.timeout_secs) =>
            {
                return Err(RouterError::InvalidConfiguration(format!(
                    "provider '{}' has conflicting health probe policies",
                    target.key.provider_name
                )));
            }
            Entry::Occupied(_) => {}
        }
        build_custom_client(&target.key.policy, target.key.timeout_secs).map_err(|failure| {
            RouterError::InvalidConfiguration(format!(
                "provider '{}' health probe client failed to initialize: {failure:?}",
                target.key.provider_name
            ))
        })?;
    }
    Ok(targets.len())
}

pub(super) async fn run_probe_loop(supervisor: ProbeSupervisor) {
    let mut schedules = HashMap::new();
    let mut pending: FuturesUnordered<PendingProbe> = FuturesUnordered::new();
    let mut next_incarnation = 0;

    loop {
        let snapshot = supervisor.routing_snapshot.load_full();
        let targets = current_probe_groups(snapshot.as_ref())
            .into_iter()
            .map(|target| (target.key.clone(), target))
            .collect::<HashMap<_, _>>();
        reconcile_schedules(&targets, &mut schedules, &mut next_incarnation);
        launch_due_probes(&targets, &mut schedules, &mut pending);

        let next_run = schedules
            .values()
            .filter(|schedule| schedule.running.is_none())
            .map(|schedule| schedule.next_run)
            .min();
        let outcome = match (pending.is_empty(), next_run) {
            (false, Some(deadline)) => tokio::select! {
                outcome = pending.next() => outcome,
                () = tokio::time::sleep_until(deadline) => None,
                () = supervisor.wakeup.notified() => None,
            },
            (false, None) => tokio::select! {
                outcome = pending.next() => outcome,
                () = supervisor.wakeup.notified() => None,
            },
            (true, Some(deadline)) => {
                tokio::select! {
                    () = tokio::time::sleep_until(deadline) => {},
                    () = supervisor.wakeup.notified() => {},
                }
                None
            }
            (true, None) => {
                supervisor.wakeup.notified().await;
                None
            }
        };
        if let Some(outcome) = outcome {
            apply_probe_outcome(&supervisor, &mut schedules, outcome);
        }
    }
}

fn current_probe_groups(snapshot: &RoutingSnapshot) -> Vec<ProbeTarget> {
    let mut groups = HashMap::<DynamicProbeKey, ProbeTarget>::new();
    for deployment in snapshot.deployments.values() {
        let Some(policy) = deployment.config.health_check_policy.clone() else {
            continue;
        };
        let scope = if policy.endpoint.is_some() {
            ProbeScope::CustomEndpoint
        } else {
            let provider_instance = deployment.state.provider_instance_identity();
            match deployment
                .provider
                .native_health_probe_semantics(&deployment.model)
            {
                NativeHealthProbeSemantics::ModelSpecific => ProbeScope::NativeModel {
                    provider_instance,
                    model: deployment.model.clone(),
                },
                NativeHealthProbeSemantics::ModelIndependent => {
                    ProbeScope::NativeProvider(provider_instance)
                }
                #[cfg(any(feature = "providers-extended", feature = "providers-extra"))]
                NativeHealthProbeSemantics::Unsupported => {
                    ProbeScope::NativeProvider(provider_instance)
                }
            }
        };
        let key = DynamicProbeKey {
            scope,
            provider_name: policy.provider_name.clone(),
            provider_kind: deployment.provider.name().to_string(),
            policy,
            timeout_secs: deployment.config.timeout_secs,
        };
        groups
            .entry(key.clone())
            .and_modify(|target| target.deployments.push(Arc::clone(deployment)))
            .or_insert_with(|| ProbeTarget {
                key,
                provider: deployment.provider.clone(),
                deployments: vec![Arc::clone(deployment)],
            });
    }
    let mut groups = groups.into_values().collect::<Vec<_>>();
    for target in &mut groups {
        target
            .deployments
            .sort_unstable_by(|left, right| left.id.cmp(&right.id));
    }
    groups
}

fn reconcile_schedules(
    targets: &HashMap<DynamicProbeKey, ProbeTarget>,
    schedules: &mut HashMap<DynamicProbeKey, ProbeSchedule>,
    next_incarnation: &mut u64,
) {
    schedules.retain(|key, _| targets.contains_key(key));
    let now = Instant::now();
    for (key, target) in targets {
        let members = target.members();
        match schedules.entry(key.clone()) {
            Entry::Vacant(entry) => {
                let incarnation = *next_incarnation;
                *next_incarnation = next_incarnation.wrapping_add(1);
                entry.insert(ProbeSchedule {
                    consecutive_failures: 0,
                    next_run: now,
                    running: None,
                    incarnation,
                    epoch: 0,
                    members,
                });
            }
            Entry::Occupied(mut entry) if entry.get().members != members => {
                let schedule = entry.get_mut();
                schedule.members = members;
                schedule.epoch = schedule.epoch.wrapping_add(1);
                schedule.consecutive_failures = 0;
                schedule.running = None;
                schedule.next_run = now;
            }
            Entry::Occupied(_) => {}
        }
    }
}

fn launch_due_probes(
    targets: &HashMap<DynamicProbeKey, ProbeTarget>,
    schedules: &mut HashMap<DynamicProbeKey, ProbeSchedule>,
    pending: &mut FuturesUnordered<PendingProbe>,
) {
    let now = Instant::now();
    for (key, schedule) in schedules {
        if schedule.running.is_some() || schedule.next_run > now {
            continue;
        }
        let Some(target) = targets.get(key).cloned() else {
            continue;
        };
        let run = ProbeRun {
            incarnation: schedule.incarnation,
            epoch: schedule.epoch,
        };
        schedule.running = Some(run);
        pending.push(Box::pin(async move {
            let result = execute_target_probe(&target).await;
            ProbeOutcome {
                key: target.key.clone(),
                run,
                target,
                result,
            }
        }));
    }
}

fn apply_probe_outcome(
    supervisor: &ProbeSupervisor,
    schedules: &mut HashMap<DynamicProbeKey, ProbeSchedule>,
    outcome: ProbeOutcome,
) {
    let Some(schedule) = schedules.get_mut(&outcome.key) else {
        return;
    };
    if schedule.running != Some(outcome.run)
        || schedule.incarnation != outcome.run.incarnation
        || schedule.epoch != outcome.run.epoch
        || schedule.members != outcome.target.members()
    {
        return;
    }
    schedule.running = None;

    let snapshot = supervisor.routing_snapshot.load();
    let target_is_current = outcome.target.deployments.iter().all(|observed| {
        snapshot
            .deployments
            .get(&observed.id)
            .is_some_and(|current| Arc::ptr_eq(current, observed))
    });
    if !target_is_current {
        schedule.next_run = Instant::now();
        return;
    }
    if matches!(
        &outcome.result,
        Err(ProbeFailure::ProviderStatus(ProviderHealthStatus::Unknown))
    ) {
        schedule.next_run =
            Instant::now() + Duration::from_secs(outcome.target.key.policy.interval_secs);
        return;
    }

    let had_failures = schedule.consecutive_failures > 0;
    let delay = match outcome.result {
        Ok(()) => apply_probe_result_to(
            &outcome.target.key.policy,
            true,
            &mut schedule.consecutive_failures,
            &outcome.target.deployments,
        ),
        Err(failure) => {
            let delay = apply_probe_result_to(
                &outcome.target.key.policy,
                false,
                &mut schedule.consecutive_failures,
                &outcome.target.deployments,
            );
            if delay.is_some() {
                log_probe_failure(
                    &outcome.target.key.policy,
                    schedule.consecutive_failures,
                    &failure,
                );
            }
            delay
        }
    };
    let Some(delay) = delay else {
        schedule.next_run = Instant::now();
        return;
    };
    if had_failures && schedule.consecutive_failures == 0 {
        tracing::info!(
            provider = %outcome.target.key.provider_name,
            "provider health probe recovered"
        );
    }
    schedule.next_run = Instant::now() + delay;
}

async fn execute_target_probe(target: &ProbeTarget) -> Result<(), ProbeFailure> {
    let custom_client = build_custom_client(&target.key.policy, target.key.timeout_secs)?;
    let model = target
        .deployments
        .first()
        .map(|deployment| deployment.model.as_str())
        .ok_or(ProbeFailure::ClientUnavailable)?;
    match tokio::time::timeout(
        Duration::from_secs(target.key.timeout_secs),
        execute_probe(
            &target.provider,
            model,
            &target.key.policy,
            custom_client.as_deref(),
        ),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(ProbeFailure::Timeout),
    }
}

fn build_custom_client(
    policy: &HealthCheckPolicy,
    timeout_secs: u64,
) -> Result<Option<Arc<BaseHttpClient>>, ProbeFailure> {
    policy
        .endpoint
        .as_ref()
        .map(|endpoint| {
            BaseHttpClient::new_for_provider_no_redirect(
                "health_probe",
                BaseConfig {
                    api_base: Some(endpoint.to_string()),
                    endpoint_access: policy.endpoint_access,
                    timeout: timeout_secs,
                    ..Default::default()
                },
            )
            .map(Arc::new)
            .map_err(|error| ProbeFailure::Request(error.to_string()))
        })
        .transpose()
}

pub(super) async fn execute_probe(
    provider: &Provider,
    model: &str,
    policy: &HealthCheckPolicy,
    custom_client: Option<&BaseHttpClient>,
) -> Result<(), ProbeFailure> {
    if let Some(endpoint) = &policy.endpoint {
        let client = custom_client.ok_or(ProbeFailure::ClientUnavailable)?;
        let response = client
            .get(endpoint.clone())
            .map_err(|error| ProbeFailure::Request(error.to_string()))?
            .send()
            .await
            .map_err(|error| ProbeFailure::Request(error.to_string()))?;
        let status = response.status().as_u16();
        if policy.expected_codes.contains(&status) {
            Ok(())
        } else {
            Err(ProbeFailure::UnexpectedStatus(status))
        }
    } else {
        match provider.health_check_for_model(model).await {
            ProviderHealthStatus::Healthy => Ok(()),
            status => Err(ProbeFailure::ProviderStatus(status)),
        }
    }
}

#[cfg(test)]
pub(super) fn apply_probe_result(
    policy: &HealthCheckPolicy,
    deployments: &[Arc<Deployment>],
    succeeded: bool,
    consecutive_failures: &mut u32,
) -> Duration {
    apply_probe_result_to(policy, succeeded, consecutive_failures, deployments)
        .expect("test deployments should own their probe generations")
}

fn apply_probe_result_to(
    policy: &HealthCheckPolicy,
    succeeded: bool,
    consecutive_failures: &mut u32,
    deployments: &[Arc<Deployment>],
) -> Option<Duration> {
    let previous_failures = *consecutive_failures;
    let next_failures = if succeeded {
        0
    } else {
        previous_failures.saturating_add(1)
    };
    let threshold_reached = next_failures >= policy.failure_threshold;
    let target = if succeeded {
        HealthStatus::Healthy
    } else if threshold_reached {
        HealthStatus::Unhealthy
    } else {
        HealthStatus::Degraded
    };
    if !publish_probe_group(deployments, target) {
        return None;
    }
    *consecutive_failures = next_failures;
    let delay_secs = if !succeeded && threshold_reached {
        policy.recovery_timeout_secs
    } else {
        policy.interval_secs
    };
    Some(Duration::from_secs(delay_secs))
}

#[cfg(test)]
pub(super) fn update_probe_health(deployment: &Deployment, target: HealthStatus) -> bool {
    deployment.publish_probe_health(target)
}

fn log_probe_failure(
    policy: &HealthCheckPolicy,
    consecutive_failures: u32,
    failure: &ProbeFailure,
) {
    if consecutive_failures >= policy.failure_threshold {
        tracing::error!(
            provider = %policy.provider_name,
            consecutive_failures,
            failure = ?failure,
            "provider health probe marked deployments unhealthy"
        );
    } else {
        tracing::warn!(
            provider = %policy.provider_name,
            consecutive_failures,
            failure = ?failure,
            "provider health probe failed"
        );
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
