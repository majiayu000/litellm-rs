use super::*;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::openai::OpenAIProvider;
use crate::core::providers::openai::config::test_openai_config;
use crate::core::router::config::RouterConfig;
use crate::core::router::deployment::DeploymentConfig;
use url::Url;

#[tokio::test]
async fn stale_removed_schedule_outcome_cannot_clear_recreated_run() {
    let provider = Provider::OpenAI(
        OpenAIProvider::new(test_openai_config(
            "http://127.0.0.1:9".to_string(),
            "sk-schedule-incarnation",
        ))
        .await
        .expect("test provider should be valid"),
    );
    let policy = HealthCheckPolicy {
        provider_name: "schedule-incarnation".to_string(),
        interval_secs: 30,
        failure_threshold: 1,
        recovery_timeout_secs: 30,
        endpoint: Some(
            Url::parse("http://127.0.0.1:9/health").expect("schedule test endpoint should parse"),
        ),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        expected_codes: vec![204],
    };
    let deployment = Deployment::new(
        "stable".to_string(),
        provider,
        "stable-model".to_string(),
        "stable-model".to_string(),
    )
    .with_config(DeploymentConfig {
        health_check_policy: Some(policy),
        ..DeploymentConfig::default()
    });
    let router = Router::new(RouterConfig::default());
    router.add_deployment(deployment);
    let supervisor = ProbeSupervisor {
        routing_snapshot: Arc::clone(&router.routing_snapshot),
        wakeup: Arc::clone(&router.health_probe_wakeup),
    };
    let targets = current_probe_groups(router.routing_snapshot.load().as_ref())
        .into_iter()
        .map(|target| (target.key.clone(), target))
        .collect::<HashMap<_, _>>();
    let target = targets
        .values()
        .next()
        .expect("probe target should exist")
        .clone();
    let mut schedules = HashMap::new();
    let mut pending = FuturesUnordered::new();
    let mut next_incarnation = 0;

    reconcile_schedules(&targets, &mut schedules, &mut next_incarnation);
    launch_due_probes(&targets, &mut schedules, &mut pending);
    let old_run = schedules
        .get(&target.key)
        .and_then(|schedule| schedule.running)
        .expect("old schedule should be running");

    reconcile_schedules(&HashMap::new(), &mut schedules, &mut next_incarnation);
    reconcile_schedules(&targets, &mut schedules, &mut next_incarnation);
    launch_due_probes(&targets, &mut schedules, &mut pending);
    let new_run = schedules
        .get(&target.key)
        .and_then(|schedule| schedule.running)
        .expect("recreated schedule should be running");
    assert_ne!(old_run.incarnation, new_run.incarnation);

    apply_probe_outcome(
        &supervisor,
        &mut schedules,
        ProbeOutcome {
            key: target.key.clone(),
            run: old_run,
            target,
            result: Ok(()),
        },
    );

    assert_eq!(
        schedules
            .values()
            .next()
            .expect("recreated schedule should remain")
            .running,
        Some(new_run),
        "stale outcome must not clear the recreated schedule's active run"
    );
}

#[tokio::test]
async fn unknown_native_status_preserves_unknown_evidence_without_counting_failure() {
    let provider = Provider::OpenAI(
        OpenAIProvider::new(test_openai_config(
            "http://127.0.0.1:9".to_string(),
            "sk-unknown-evidence",
        ))
        .await
        .expect("test provider should be valid"),
    );
    let policy = HealthCheckPolicy {
        provider_name: "unknown-evidence".to_string(),
        interval_secs: 30,
        failure_threshold: 1,
        recovery_timeout_secs: 60,
        endpoint: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        expected_codes: vec![200],
    };
    let deployment = Deployment::new(
        "unknown".to_string(),
        provider,
        "configured-model".to_string(),
        "configured-model".to_string(),
    )
    .with_config(DeploymentConfig {
        health_check_policy: Some(policy),
        ..DeploymentConfig::default()
    });
    let router = Router::new(RouterConfig::default());
    router.add_deployment(deployment);
    let deployment = router
        .get_deployment("unknown")
        .expect("test deployment should exist");
    let supervisor = ProbeSupervisor {
        routing_snapshot: Arc::clone(&router.routing_snapshot),
        wakeup: Arc::clone(&router.health_probe_wakeup),
    };
    let target = current_probe_groups(router.routing_snapshot.load().as_ref())
        .into_iter()
        .next()
        .expect("probe target should exist");
    let targets = HashMap::from([(target.key.clone(), target.clone())]);
    let mut schedules = HashMap::new();
    let mut next_incarnation = 0;
    reconcile_schedules(&targets, &mut schedules, &mut next_incarnation);
    let schedule = schedules
        .get_mut(&target.key)
        .expect("probe schedule should exist");
    let run = ProbeRun {
        incarnation: schedule.incarnation,
        epoch: schedule.epoch,
    };
    schedule.running = Some(run);
    let before = Instant::now();

    apply_probe_outcome(
        &supervisor,
        &mut schedules,
        ProbeOutcome {
            key: target.key.clone(),
            run,
            target,
            result: Err(ProbeFailure::ProviderStatus(ProviderHealthStatus::Unknown)),
        },
    );

    let schedule = schedules
        .values()
        .next()
        .expect("probe schedule should remain");
    assert_eq!(schedule.consecutive_failures, 0);
    assert_eq!(schedule.running, None);
    assert!(schedule.next_run >= before + Duration::from_secs(30));
    assert_eq!(
        deployment.state.probe_health_status(),
        HealthStatus::Unknown
    );
    assert_eq!(deployment.state.probe_last_checked_at_millis(), None);
}
