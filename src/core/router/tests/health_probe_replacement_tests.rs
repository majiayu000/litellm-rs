use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::Provider;
use crate::core::providers::openai::OpenAIProvider;
use crate::core::providers::openai::config::test_openai_config;
use crate::core::router::config::RouterConfig;
use crate::core::router::deployment::{
    Deployment, DeploymentConfig, HealthCheckPolicy, HealthStatus,
};
use crate::core::router::health_probe::tests::sequence_server;
use crate::core::router::unified::Router;
use std::time::Duration;
use tokio::sync::mpsc;

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
