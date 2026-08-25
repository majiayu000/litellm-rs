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

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn model_specific_probes_isolate_models_and_singleflight_same_model_clones() {
    use crate::core::providers::gemini::{GeminiConfig, GeminiProvider};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::mpsc;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("model-aware probe listener should bind");
    let address = listener
        .local_addr()
        .expect("model-aware probe address should exist");
    let (request_tx, mut request_rx) = mpsc::unbounded_channel();
    let server = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener
                .accept()
                .await
                .expect("model-aware probe should connect");
            let request_tx = request_tx.clone();
            tokio::spawn(async move {
                let mut request = [0_u8; 8192];
                let bytes_read = socket
                    .read(&mut request)
                    .await
                    .expect("model-aware probe request should be readable");
                let request = String::from_utf8_lossy(&request[..bytes_read]);
                let request_line = request
                    .lines()
                    .next()
                    .expect("probe request line should exist")
                    .to_string();
                let is_flash = request_line.contains("/models/gemini-2.5-flash:");
                let (status, reason, body) = if is_flash {
                    (
                        200,
                        "OK",
                        concat!(
                            r#"{"candidates":[{"content":{"parts":[{"text":"ok"}]},"finishReason":"STOP"}],"#,
                            r#""usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":1,"totalTokenCount":2}}"#
                        ),
                    )
                } else {
                    (500, "Internal Server Error", "{}")
                };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("model-aware probe response should be writable");
                request_tx
                    .send(request_line)
                    .expect("probe request should be observable");
            });
        }
    });

    let mut config = GeminiConfig::new_google_ai("test-model-aware-probe-key");
    config.base_url = format!("http://{address}");
    config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = Provider::Gemini(Arc::new(
        GeminiProvider::new(config).expect("test Gemini provider should be valid"),
    ));
    let provider_instance = ProviderInstanceIdentity::new();
    let policy = HealthCheckPolicy {
        provider_name: "gemini-shared".to_string(),
        interval_secs: 30,
        failure_threshold: 1,
        recovery_timeout_secs: 30,
        endpoint: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        expected_codes: vec![200],
    };
    let router = Router::new(RouterConfig::default());
    for (id, model) in [
        ("flash-a", "gemini-2.5-flash"),
        ("flash-b", "gemini-2.5-flash"),
        ("pro", "gemini-2.5-pro"),
    ] {
        router.add_deployment(
            Deployment::new_with_provider_instance(
                id.to_string(),
                provider.clone(),
                model.to_string(),
                model.to_string(),
                provider_instance.clone(),
            )
            .with_config(DeploymentConfig {
                timeout_secs: 2,
                health_check_policy: Some(policy.clone()),
                ..DeploymentConfig::default()
            }),
        );
    }

    assert_eq!(router.start_configured_health_checks().unwrap(), 1);
    let mut requests = Vec::new();
    for _ in 0..2 {
        requests.push(
            tokio::time::timeout(Duration::from_secs(2), request_rx.recv())
                .await
                .expect("each configured model should be probed")
                .expect("probe server should report each request"),
        );
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(300), request_rx.recv())
            .await
            .is_err(),
        "same-model clones must share one model-specific probe"
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.contains("/models/gemini-2.5-flash:"))
            .count(),
        1
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.contains("/models/gemini-2.5-pro:"))
            .count(),
        1
    );

    let flash_a = router
        .get_deployment("flash-a")
        .expect("first flash deployment should exist");
    let flash_b = router
        .get_deployment("flash-b")
        .expect("second flash deployment should exist");
    let pro = router
        .get_deployment("pro")
        .expect("pro deployment should exist");
    tokio::time::timeout(Duration::from_secs(2), async {
        while flash_a.state.probe_health_status() != HealthStatus::Healthy
            || flash_b.state.probe_health_status() != HealthStatus::Healthy
            || pro.state.probe_health_status() != HealthStatus::Unhealthy
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("model-specific probe results should publish only to matching deployments");

    drop(router);
    server.abort();
}
