use crate::config::models::provider::ProviderConfig;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::Provider;
use crate::core::providers::anthropic::{AnthropicConfig, AnthropicProvider};
use crate::core::providers::openai::OpenAIProvider;
use crate::core::providers::openai::config::test_openai_config;
use crate::core::router::config::RouterConfig;
use crate::core::router::deployment::{
    Deployment, DeploymentConfig, HealthCheckPolicy, HealthStatus,
};
use crate::core::router::health_probe::tests::sequence_server;
use crate::core::router::unified::Router;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use url::Url;

async fn wait_for_request(requests: &mut mpsc::UnboundedReceiver<()>) {
    tokio::time::timeout(Duration::from_secs(2), requests.recv())
        .await
        .expect("probe should run before the timeout")
        .expect("probe server should report the request");
}

async fn wait_for_probe_health(deployment: &Deployment, expected: HealthStatus) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while deployment.state.probe_health_status() != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("probe health transition should complete");
}

async fn gated_status_server(
    status: u16,
) -> (
    Url,
    oneshot::Receiver<()>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("gated probe server should bind");
    let address = listener
        .local_addr()
        .expect("gated probe address should exist");
    let (request_seen_tx, request_seen_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("gated probe should connect");
        let mut request = [0_u8; 2048];
        let bytes_read = stream
            .read(&mut request)
            .await
            .expect("gated probe request should be readable");
        assert!(bytes_read > 0, "gated probe request should not be empty");
        request_seen_tx
            .send(())
            .expect("gated probe request should be observable");
        release_rx
            .await
            .expect("gated probe response should be released");
        let response =
            format!("HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        stream
            .write_all(response.as_bytes())
            .await
            .expect("gated probe response should be writable");
    });
    let endpoint = Url::parse(&format!("http://{address}/health")).expect("probe URL should parse");
    (endpoint, request_seen_rx, release_tx, task)
}

async fn controlled_status_server(
    status: u16,
) -> (
    Url,
    mpsc::UnboundedReceiver<oneshot::Sender<()>>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("controlled probe server should bind");
    let address = listener
        .local_addr()
        .expect("controlled probe address should exist");
    let (request_tx, request_rx) = mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("controlled probe should connect");
            let mut request = [0_u8; 2048];
            let bytes_read = stream
                .read(&mut request)
                .await
                .expect("controlled probe request should be readable");
            assert!(
                bytes_read > 0,
                "controlled probe request should not be empty"
            );
            let (release_tx, release_rx) = oneshot::channel();
            request_tx
                .send(release_tx)
                .expect("controlled probe request should be observable");
            tokio::spawn(async move {
                release_rx
                    .await
                    .expect("controlled probe response should be released");
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("controlled probe response should be writable");
            });
        }
    });
    let endpoint = Url::parse(&format!("http://{address}/health")).expect("probe URL should parse");
    (endpoint, request_rx, task)
}

fn policy_for(endpoint: Url, provider_name: &str, interval_secs: u64) -> HealthCheckPolicy {
    policy_with_schedule(endpoint, provider_name, interval_secs, interval_secs)
}

fn policy_with_schedule(
    endpoint: Url,
    provider_name: &str,
    interval_secs: u64,
    recovery_timeout_secs: u64,
) -> HealthCheckPolicy {
    HealthCheckPolicy {
        provider_name: provider_name.to_string(),
        interval_secs,
        failure_threshold: 1,
        recovery_timeout_secs,
        endpoint: Some(endpoint),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        expected_codes: vec![204],
    }
}

fn deployment_with_policy(id: &str, provider: Provider, policy: HealthCheckPolicy) -> Deployment {
    Deployment::new(
        id.to_string(),
        provider,
        format!("{id}-model"),
        format!("{id}-model"),
    )
    .with_config(DeploymentConfig {
        timeout_secs: 2,
        health_check_policy: Some(policy),
        ..DeploymentConfig::default()
    })
}

#[tokio::test]
async fn running_probe_publishes_to_replacement_deployment() {
    let (endpoint, mut requests, server) = sequence_server(vec![500, 204, 204]).await;
    let provider = Provider::OpenAI(
        OpenAIProvider::new(test_openai_config(
            endpoint.to_string(),
            "sk-health-probe-test",
        ))
        .await
        .expect("test provider should be valid"),
    );
    let policy = HealthCheckPolicy {
        provider_name: "openai-primary".to_string(),
        interval_secs: 1,
        failure_threshold: 1,
        recovery_timeout_secs: 1,
        endpoint: Some(endpoint.clone()),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        expected_codes: vec![204],
    };
    let deployment = Deployment::new(
        "replacement".to_string(),
        provider,
        "gpt-test".to_string(),
        "gpt-test".to_string(),
    )
    .with_config(DeploymentConfig {
        timeout_secs: 2,
        health_check_policy: Some(policy),
        ..DeploymentConfig::default()
    });
    let router = Router::new(RouterConfig::default());
    router.add_deployment(deployment);
    let original = router
        .get_deployment("replacement")
        .expect("original deployment should exist");

    assert_eq!(router.start_configured_health_checks().unwrap(), 1);
    wait_for_request(&mut requests).await;
    wait_for_probe_health(&original, HealthStatus::Unhealthy).await;

    router.add_deployment(original.as_ref().clone());
    let replacement = router
        .get_deployment("replacement")
        .expect("replacement deployment should exist");
    assert_eq!(
        replacement.state.probe_health_status(),
        HealthStatus::Unknown
    );
    wait_for_request(&mut requests).await;
    wait_for_probe_health(&replacement, HealthStatus::Healthy).await;

    router.set_model_list(vec![replacement.as_ref().clone()]);
    let bulk_replacement = router
        .get_deployment("replacement")
        .expect("bulk replacement deployment should exist");
    assert_eq!(
        bulk_replacement.state.probe_health_status(),
        HealthStatus::Unknown
    );
    wait_for_request(&mut requests).await;
    wait_for_probe_health(&bulk_replacement, HealthStatus::Healthy).await;

    drop(router);
    server.await.expect("test server should stop");
}

#[tokio::test]
async fn stale_success_is_not_published_to_changed_same_id_replacement() {
    let (old_endpoint, old_request, release_old, old_server) = gated_status_server(204).await;
    let old_provider = Provider::OpenAI(
        OpenAIProvider::new(test_openai_config(
            old_endpoint.to_string(),
            "sk-old-health-probe-credential",
        ))
        .await
        .expect("old provider should be valid"),
    );
    let old_policy = HealthCheckPolicy {
        provider_name: "openai-primary".to_string(),
        interval_secs: 1,
        failure_threshold: 1,
        recovery_timeout_secs: 1,
        endpoint: Some(old_endpoint),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        expected_codes: vec![204],
    };
    let router = Router::new(RouterConfig::default());
    router.add_deployment(
        Deployment::new(
            "replacement".to_string(),
            old_provider,
            "gpt-old".to_string(),
            "gpt-test".to_string(),
        )
        .with_config(DeploymentConfig {
            timeout_secs: 2,
            health_check_policy: Some(old_policy),
            ..DeploymentConfig::default()
        }),
    );
    assert_eq!(router.start_configured_health_checks().unwrap(), 1);
    tokio::time::timeout(Duration::from_secs(1), old_request)
        .await
        .expect("old probe should start")
        .expect("old probe should be observed");

    let (new_endpoint, new_request, release_new, new_server) = gated_status_server(500).await;
    let new_provider = Provider::Anthropic(
        AnthropicProvider::new(AnthropicConfig::new_test(
            "sk-ant-replacement-health-credential",
        ))
        .expect("replacement provider should be valid"),
    );
    let new_policy = HealthCheckPolicy {
        provider_name: "anthropic-replacement".to_string(),
        interval_secs: 1,
        failure_threshold: 1,
        recovery_timeout_secs: 1,
        endpoint: Some(new_endpoint),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        expected_codes: vec![204],
    };
    router.add_deployment(
        Deployment::new(
            "replacement".to_string(),
            new_provider,
            "claude-new".to_string(),
            "gpt-test".to_string(),
        )
        .with_config(DeploymentConfig {
            timeout_secs: 2,
            health_check_policy: Some(new_policy),
            ..DeploymentConfig::default()
        }),
    );
    let replacement = router
        .get_deployment("replacement")
        .expect("replacement deployment should exist");
    assert_eq!(replacement.provider.name(), "anthropic");
    assert_eq!(
        replacement.state.probe_health_status(),
        HealthStatus::Unknown
    );

    release_old
        .send(())
        .expect("old successful response should be released");
    tokio::time::timeout(Duration::from_secs(3), new_request)
        .await
        .expect("replacement should be reprobed with its current policy")
        .expect("replacement probe should be observed");
    assert_eq!(
        replacement.state.probe_health_status(),
        HealthStatus::Unknown,
        "the old provider's success must not publish to the replacement"
    );

    release_new
        .send(())
        .expect("replacement failure response should be released");
    wait_for_probe_health(&replacement, HealthStatus::Unhealthy).await;

    drop(router);
    old_server.await.expect("old probe server should stop");
    new_server
        .await
        .expect("replacement probe server should stop");
}

#[tokio::test]
async fn distinct_provider_instances_receive_distinct_native_probes() {
    let (first_endpoint, mut first_requests, first_server) = controlled_status_server(204).await;
    let (second_endpoint, mut second_requests, second_server) = controlled_status_server(204).await;
    let first_provider = Provider::OpenAI(
        OpenAIProvider::new(test_openai_config(
            first_endpoint.to_string(),
            "sk-first-health-probe-credential",
        ))
        .await
        .expect("first provider should be valid"),
    );
    let second_provider = Provider::OpenAI(
        OpenAIProvider::new(test_openai_config(
            second_endpoint.to_string(),
            "sk-second-health-probe-credential",
        ))
        .await
        .expect("second provider should be valid"),
    );
    let policy = HealthCheckPolicy {
        provider_name: "openai-primary".to_string(),
        interval_secs: 30,
        failure_threshold: 1,
        recovery_timeout_secs: 30,
        endpoint: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        expected_codes: vec![204],
    };
    let router = Router::new(RouterConfig::default());
    router.add_deployment(deployment_with_policy(
        "model-a",
        first_provider,
        policy.clone(),
    ));
    router.add_deployment(deployment_with_policy("model-b", second_provider, policy));
    assert_eq!(router.start_configured_health_checks().unwrap(), 1);
    let release_first = tokio::time::timeout(Duration::from_secs(1), first_requests.recv())
        .await
        .expect("first native probe should start")
        .expect("first native probe should be observed");
    let release_second = tokio::time::timeout(Duration::from_secs(1), second_requests.recv())
        .await
        .expect("second native probe must not share the first provider instance")
        .expect("second native probe should be observed");
    release_first
        .send(())
        .expect("first native probe should be released");
    release_second
        .send(())
        .expect("second native probe should be released");

    drop(router);
    first_server.abort();
    second_server.abort();
}

#[tokio::test]
async fn default_gateway_health_config_probes_shared_provider_once() {
    let (endpoint, mut requests, server) = controlled_status_server(204).await;
    let provider = ProviderConfig {
        name: "default-probe".to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-default-health-probe".to_string(),
        base_url: Some(endpoint.to_string()),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        models: vec!["model-a".to_string(), "model-b".to_string()],
        ..ProviderConfig::default()
    };

    let router = Router::from_gateway_config(&[provider], None)
        .await
        .expect("default provider health configuration should start");
    let release = tokio::time::timeout(Duration::from_secs(1), requests.recv())
        .await
        .expect("default native probe should run")
        .expect("default native probe should be observed");
    assert!(
        tokio::time::timeout(Duration::from_millis(300), requests.recv())
            .await
            .is_err(),
        "models cloned from one provider instance must share one native probe"
    );
    release
        .send(())
        .expect("default native probe should be released");

    let first = router
        .get_deployment("default-probe-model-a")
        .expect("first configured model should exist");
    let second = router
        .get_deployment("default-probe-model-b")
        .expect("second configured model should exist");
    wait_for_probe_health(&first, HealthStatus::Healthy).await;
    wait_for_probe_health(&second, HealthStatus::Healthy).await;

    drop(router);
    server.abort();
}

#[tokio::test]
async fn unrelated_snapshot_update_does_not_reject_in_flight_probe() {
    let (endpoint, request, release, server) = gated_status_server(204).await;
    let provider = Provider::OpenAI(
        OpenAIProvider::new(test_openai_config(
            endpoint.to_string(),
            "sk-unrelated-snapshot-update",
        ))
        .await
        .expect("provider should be valid"),
    );
    let router = Router::new(RouterConfig::default());
    router.add_deployment(deployment_with_policy(
        "stable",
        provider,
        policy_for(endpoint, "stable-provider", 30),
    ));
    let deployment = router
        .get_deployment("stable")
        .expect("stable deployment should exist");
    assert_eq!(router.start_configured_health_checks().unwrap(), 1);
    tokio::time::timeout(Duration::from_secs(1), request)
        .await
        .expect("probe should start")
        .expect("probe should be observed");

    router.publish_current_snapshot();
    release.send(()).expect("probe response should be released");
    wait_for_probe_health(&deployment, HealthStatus::Healthy).await;

    drop(router);
    server.await.expect("probe server should stop");
}

#[tokio::test]
async fn replacement_resets_probe_failure_streak() {
    let (endpoint, mut requests, server) = controlled_status_server(500).await;
    let provider = Provider::OpenAI(
        OpenAIProvider::new(test_openai_config(
            endpoint.to_string(),
            "sk-failure-generation-reset",
        ))
        .await
        .expect("provider should be valid"),
    );
    let policy = HealthCheckPolicy {
        provider_name: "reset-provider".to_string(),
        interval_secs: 1,
        failure_threshold: 3,
        recovery_timeout_secs: 1,
        endpoint: Some(endpoint),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        expected_codes: vec![204],
    };
    let router = Router::new(RouterConfig::default());
    router.add_deployment(deployment_with_policy("reset", provider, policy));
    let original = router
        .get_deployment("reset")
        .expect("original deployment should exist");
    assert_eq!(router.start_configured_health_checks().unwrap(), 1);

    for expected_generation in 1..=2 {
        let before = original.state.probe_last_checked_at_millis();
        let release = tokio::time::timeout(Duration::from_secs(2), requests.recv())
            .await
            .expect("original probe should run")
            .expect("original probe should be observed");
        release.send(()).expect("original probe should be released");
        tokio::time::timeout(Duration::from_secs(1), async {
            while original.state.probe_last_checked_at_millis() == before {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("probe generation {expected_generation} should publish"));
        assert_eq!(original.state.probe_health_status(), HealthStatus::Degraded);
    }

    router.add_deployment(original.as_ref().clone());
    let replacement = router
        .get_deployment("reset")
        .expect("replacement deployment should exist");
    assert_eq!(
        replacement.state.probe_health_status(),
        HealthStatus::Unknown
    );
    let release = tokio::time::timeout(Duration::from_secs(1), requests.recv())
        .await
        .expect("replacement should be probed immediately")
        .expect("replacement probe should be observed");
    release
        .send(())
        .expect("replacement probe should be released");
    wait_for_probe_health(&replacement, HealthStatus::Degraded).await;

    drop(router);
    server.abort();
}

#[tokio::test]
async fn probe_groups_keep_independent_recovery_deadlines() {
    let (slow_endpoint, mut slow_requests, slow_server) = controlled_status_server(500).await;
    let (fast_endpoint, mut fast_requests, fast_server) = controlled_status_server(500).await;
    let provider = Provider::OpenAI(
        OpenAIProvider::new(test_openai_config(
            slow_endpoint.to_string(),
            "sk-health-probe-schedule-test",
        ))
        .await
        .expect("provider should be valid"),
    );
    let router = Router::new(RouterConfig::default());
    router.add_deployment(deployment_with_policy(
        "slow",
        provider.clone(),
        policy_with_schedule(slow_endpoint, "slow-provider", 1, 3),
    ));
    router.add_deployment(deployment_with_policy(
        "fast",
        provider,
        policy_with_schedule(fast_endpoint, "fast-provider", 1, 1),
    ));

    assert_eq!(router.start_configured_health_checks().unwrap(), 1);
    let release_slow = tokio::time::timeout(Duration::from_secs(1), slow_requests.recv())
        .await
        .expect("slow probe should start")
        .expect("slow probe should be observed");
    let release_fast = tokio::time::timeout(Duration::from_secs(1), fast_requests.recv())
        .await
        .expect("fast probe should start concurrently")
        .expect("fast probe should be observed");
    release_slow
        .send(())
        .expect("slow probe should be released");
    release_fast
        .send(())
        .expect("fast probe should be released");

    let release_fast_retry = tokio::time::timeout(Duration::from_secs(2), fast_requests.recv())
        .await
        .expect("fast group should honor its one-second recovery deadline")
        .expect("fast retry should be observed");
    assert!(
        tokio::time::timeout(Duration::from_millis(600), slow_requests.recv())
            .await
            .is_err(),
        "the fast sibling must not accelerate the slow group's recovery timeout"
    );
    release_fast_retry
        .send(())
        .expect("fast retry should be released");

    drop(router);
    slow_server.abort();
    fast_server.abort();
}

#[tokio::test]
async fn replacement_generation_rejects_post_validation_publication() {
    let endpoint = Url::parse("http://127.0.0.1:9/health").expect("probe URL should parse");
    let provider = Provider::OpenAI(
        OpenAIProvider::new(test_openai_config(
            endpoint.to_string(),
            "sk-health-probe-generation-test",
        ))
        .await
        .expect("provider should be valid"),
    );
    let router = Router::new(RouterConfig::default());
    router.add_deployment(deployment_with_policy(
        "racing",
        provider,
        policy_for(endpoint, "racing-provider", 30),
    ));
    let original = router
        .get_deployment("racing")
        .expect("original deployment should exist");

    let validated = router
        .get_deployment("racing")
        .is_some_and(|current| std::sync::Arc::ptr_eq(&current, &original));
    assert!(
        validated,
        "the old result should validate before replacement"
    );
    router.add_deployment(original.as_ref().clone());
    let replacement = router
        .get_deployment("racing")
        .expect("replacement deployment should exist");

    assert!(
        !original.publish_probe_health(HealthStatus::Unhealthy),
        "the old generation must reject publication after replacement"
    );
    assert_eq!(
        replacement.state.health_status(),
        HealthStatus::Healthy,
        "a stale result must not mutate request health shared with the replacement"
    );
    assert_eq!(
        replacement.state.probe_health_status(),
        HealthStatus::Unknown
    );
    assert!(
        !replacement
            .state
            .probe_unhealthy
            .load(std::sync::atomic::Ordering::Relaxed)
    );
}
