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
