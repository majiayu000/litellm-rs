use super::*;

pub(super) async fn run_probe_loop(group: ProbeGroup) {
    let mut consecutive_failures = 0_u32;
    let mut replacement_failures: HashMap<DeploymentId, (Arc<Deployment>, u32)> = HashMap::new();

    loop {
        let (originals, replacements) = current_probe_targets(&group);
        let mut delays = Vec::new();

        if !originals.is_empty() {
            let result = match tokio::time::timeout(
                Duration::from_secs(group.timeout_secs),
                execute_probe(
                    &group.provider,
                    &group.policy,
                    group.custom_client.as_deref(),
                ),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(ProbeFailure::Timeout),
            };

            let delay = match result {
                Ok(()) => {
                    if consecutive_failures > 0 {
                        tracing::info!(
                            provider = %group.policy.provider_name,
                            "provider health probe recovered"
                        );
                    }
                    apply_probe_result(&group, true, &mut consecutive_failures)
                }
                Err(failure) => {
                    let delay = apply_probe_result(&group, false, &mut consecutive_failures);
                    log_probe_failure(&group.policy, consecutive_failures, &failure);
                    delay
                }
            };
            delays.push(delay);
        }

        for replacement in replacements {
            let id = replacement.id.clone();
            let reset_failures = replacement_failures
                .get(&id)
                .is_none_or(|(observed, _)| !Arc::ptr_eq(observed, &replacement));
            if reset_failures {
                replacement_failures.insert(id.clone(), (Arc::clone(&replacement), 0));
            }

            let Some((policy, result)) = execute_current_probe(&replacement).await else {
                replacement_failures.remove(&id);
                delays.push(Duration::from_secs(group.policy.interval_secs));
                continue;
            };
            if !is_current_deployment(&group, &replacement) {
                replacement_failures.remove(&id);
                continue;
            }

            let Some((_, failures)) = replacement_failures.get_mut(&id) else {
                continue;
            };
            let delay = match result {
                Ok(()) => {
                    if *failures > 0 {
                        tracing::info!(
                            provider = %policy.provider_name,
                            deployment_id = %id,
                            "replacement deployment health probe recovered"
                        );
                    }
                    apply_probe_result_to(
                        &policy,
                        true,
                        failures,
                        std::slice::from_ref(&replacement),
                    )
                }
                Err(failure) => {
                    let delay = apply_probe_result_to(
                        &policy,
                        false,
                        failures,
                        std::slice::from_ref(&replacement),
                    );
                    log_probe_failure(&policy, *failures, &failure);
                    delay
                }
            };
            delays.push(delay);
        }

        let delay = delays
            .into_iter()
            .min()
            .unwrap_or_else(|| Duration::from_secs(group.policy.interval_secs));
        tokio::time::sleep(delay).await;
    }
}

fn current_probe_targets(group: &ProbeGroup) -> (Vec<Arc<Deployment>>, Vec<Arc<Deployment>>) {
    let snapshot = group.routing_snapshot.load();
    let mut originals = Vec::new();
    let mut replacements = Vec::new();

    for (id, original) in &group.deployments {
        let Some(current) = snapshot.deployments.get(id) else {
            continue;
        };
        if Arc::ptr_eq(current, original) {
            originals.push(Arc::clone(current));
        } else {
            replacements.push(Arc::clone(current));
        }
    }

    (originals, replacements)
}

fn is_current_deployment(group: &ProbeGroup, candidate: &Arc<Deployment>) -> bool {
    group
        .routing_snapshot
        .load()
        .deployments
        .get(&candidate.id)
        .is_some_and(|current| Arc::ptr_eq(current, candidate))
}

async fn execute_current_probe(
    deployment: &Deployment,
) -> Option<(HealthCheckPolicy, Result<(), ProbeFailure>)> {
    let policy = deployment.config.health_check_policy.clone()?;
    let timeout = Duration::from_secs(deployment.config.timeout_secs);
    let custom_client = match policy.endpoint.as_ref() {
        Some(endpoint) => match BaseHttpClient::new_for_provider_no_redirect(
            "health_probe",
            BaseConfig {
                api_base: Some(endpoint.to_string()),
                endpoint_access: policy.endpoint_access,
                timeout: timeout.as_secs(),
                ..Default::default()
            },
        ) {
            Ok(client) => Some(client),
            Err(error) => return Some((policy, Err(ProbeFailure::Request(error.to_string())))),
        },
        None => None,
    };
    let result = match tokio::time::timeout(
        timeout,
        execute_probe(&deployment.provider, &policy, custom_client.as_ref()),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(ProbeFailure::Timeout),
    };
    Some((policy, result))
}

pub(super) async fn execute_probe(
    provider: &Provider,
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
        match provider.health_check().await {
            ProviderHealthStatus::Healthy => Ok(()),
            status => Err(ProbeFailure::ProviderStatus(status)),
        }
    }
}

pub(super) fn apply_probe_result(
    group: &ProbeGroup,
    succeeded: bool,
    consecutive_failures: &mut u32,
) -> Duration {
    let (deployments, _) = current_probe_targets(group);
    apply_probe_result_to(&group.policy, succeeded, consecutive_failures, &deployments)
}

fn apply_probe_result_to(
    policy: &HealthCheckPolicy,
    succeeded: bool,
    consecutive_failures: &mut u32,
    deployments: &[Arc<Deployment>],
) -> Duration {
    if succeeded {
        *consecutive_failures = 0;
        for deployment in deployments {
            update_probe_health(deployment, HealthStatus::Healthy);
        }
        return Duration::from_secs(policy.interval_secs);
    }

    *consecutive_failures = consecutive_failures.saturating_add(1);
    let threshold_reached = *consecutive_failures >= policy.failure_threshold;
    let target = if threshold_reached {
        HealthStatus::Unhealthy
    } else {
        HealthStatus::Degraded
    };
    for deployment in deployments {
        update_probe_health(deployment, target);
    }

    let delay_secs = if threshold_reached {
        policy.recovery_timeout_secs
    } else {
        policy.interval_secs
    };
    Duration::from_secs(delay_secs)
}

pub(super) fn update_probe_health(deployment: &Deployment, target: HealthStatus) {
    deployment.state.set_probe_health_status(target);
    deployment
        .state
        .probe_unhealthy
        .store(target == HealthStatus::Unhealthy, Ordering::Relaxed);
    if deployment.is_in_cooldown() {
        return;
    }

    let mut current = deployment.state.health.load(Ordering::Relaxed);
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

        match deployment.state.health.compare_exchange_weak(
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
