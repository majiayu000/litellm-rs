//! Active health probes for gateway-configured router deployments.

use super::deployment::{Deployment, HealthCheckPolicy, HealthStatus};
use super::error::RouterError;
use super::unified::{Router, RoutingSnapshot};
use crate::core::providers::Provider;
use crate::core::providers::base::{BaseConfig, BaseHttpClient};
use crate::core::types::health::HealthStatus as ProviderHealthStatus;
use arc_swap::ArcSwap;
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::Ordering;
use std::time::Duration;

#[derive(Debug)]
struct ProbeSupervisor {
    routing_snapshot: Arc<ArcSwap<RoutingSnapshot>>,
    wakeup: Arc<tokio::sync::Notify>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProbeFailure {
    Request(String),
    ClientUnavailable,
    Timeout,
    UnexpectedStatus(u16),
    ProviderStatus(ProviderHealthStatus),
}

impl Router {
    pub(crate) fn start_configured_health_checks(&self) -> Result<usize, RouterError> {
        if !self.config.enable_pre_call_checks {
            return Ok(0);
        }

        let snapshot = self.routing_snapshot.load_full();
        if runtime::validate_probe_snapshot(snapshot.as_ref())? == 0 {
            return Ok(0);
        }
        tokio::runtime::Handle::try_current().map_err(|_| {
            RouterError::InvalidConfiguration(
                "health probes require an active Tokio runtime".to_string(),
            )
        })?;

        let mut tasks = self.health_probe_tasks.lock();
        if !tasks.is_empty() {
            return Err(RouterError::InvalidConfiguration(
                "health probes have already been started for this router".to_string(),
            ));
        }

        tasks.insert(
            "health-probe-supervisor".to_string(),
            tokio::spawn(run_probe_loop(ProbeSupervisor {
                routing_snapshot: Arc::clone(&self.routing_snapshot),
                wakeup: Arc::clone(&self.health_probe_wakeup),
            })),
        );
        Ok(1)
    }

    #[cfg(test)]
    fn health_probe_task_count(&self) -> usize {
        self.health_probe_tasks.lock().len()
    }
}

#[path = "health_probe/runtime.rs"]
mod runtime;
use runtime::run_probe_loop;
#[cfg(test)]
use runtime::{apply_probe_result, execute_probe, update_probe_health};

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::core::net::ProviderEndpointAccess;
    use crate::core::providers::anthropic::{AnthropicConfig, AnthropicProvider};
    use crate::core::providers::openai::OpenAIProvider;
    use crate::core::providers::openai::config::test_openai_config;
    use crate::core::router::config::RouterConfig;
    use crate::core::router::deployment::{DeploymentConfig, DeploymentState};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;
    use url::Url;

    async fn test_provider(base_url: Option<String>) -> Provider {
        let config = match base_url {
            Some(base_url) => test_openai_config(base_url, "sk-health-probe-test"),
            None => {
                let mut config = crate::core::providers::openai::OpenAIConfig::default();
                config.base.api_key = Some("sk-health-probe-test".to_string());
                config
            }
        };
        Provider::OpenAI(
            OpenAIProvider::new(config)
                .await
                .expect("test provider should be valid"),
        )
    }

    fn test_policy(endpoint: Option<Url>) -> HealthCheckPolicy {
        let endpoint_access = if endpoint.is_some() {
            ProviderEndpointAccess::PrivateNetwork
        } else {
            ProviderEndpointAccess::PublicOnly
        };
        HealthCheckPolicy {
            provider_name: "openai-primary".to_string(),
            interval_secs: 30,
            failure_threshold: 2,
            recovery_timeout_secs: 60,
            endpoint,
            endpoint_access,
            expected_codes: vec![204],
        }
    }

    async fn status_server(status: u16) -> (Url, tokio::task::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let address = listener.local_addr().expect("address should be available");
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("request should connect");
            let mut request = [0_u8; 2048];
            let bytes_read = stream
                .read(&mut request)
                .await
                .expect("request should be readable");
            let (reason, body) = match status {
                200 => (
                    "OK",
                    r#"{"id":"msg_probe","type":"message","role":"assistant","content":[{"type":"text","text":"ok"}],"model":"probe-model","stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}}"#,
                ),
                204 => ("No Content", ""),
                _ => ("Internal Server Error", ""),
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("response should be writable");
            request[..bytes_read].to_vec()
        });
        let endpoint =
            Url::parse(&format!("http://{address}/health")).expect("test endpoint should parse");
        (endpoint, task)
    }

    pub(crate) async fn sequence_server(
        statuses: Vec<u16>,
    ) -> (
        Url,
        mpsc::UnboundedReceiver<()>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let address = listener.local_addr().expect("address should be available");
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            for status in statuses {
                let (mut stream, _) = listener.accept().await.expect("request should connect");
                let mut request = [0_u8; 2048];
                let bytes_read = stream
                    .read(&mut request)
                    .await
                    .expect("request should be readable");
                assert!(bytes_read > 0, "request should not be empty");
                request_tx.send(()).expect("request should be observable");
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("response should be writable");
            }
        });
        let endpoint =
            Url::parse(&format!("http://{address}/health")).expect("test endpoint should parse");
        (endpoint, request_rx, task)
    }

    async fn redirect_server() -> (
        Url,
        tokio::task::JoinHandle<()>,
        tokio::task::JoinHandle<bool>,
    ) {
        let target_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("redirect target should bind");
        let target_address = target_listener
            .local_addr()
            .expect("redirect target address should be available");
        let redirect_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("redirect server should bind");
        let redirect_address = redirect_listener
            .local_addr()
            .expect("redirect address should be available");

        let redirect_task = tokio::spawn(async move {
            let (mut stream, _) = redirect_listener
                .accept()
                .await
                .expect("redirect request should connect");
            let mut request = [0_u8; 2048];
            let bytes_read = stream
                .read(&mut request)
                .await
                .expect("redirect request should be readable");
            assert!(bytes_read > 0, "redirect request should not be empty");
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("redirect response should be writable");
        });

        let target_task = tokio::spawn(async move {
            let Ok(Ok((mut stream, _))) =
                tokio::time::timeout(Duration::from_millis(500), target_listener.accept()).await
            else {
                return false;
            };
            let mut request = [0_u8; 2048];
            let bytes_read = stream
                .read(&mut request)
                .await
                .expect("redirected request should be readable");
            assert!(bytes_read > 0, "redirected request should not be empty");
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .expect("redirect target response should be writable");
            true
        });

        let endpoint = Url::parse(&format!("http://{redirect_address}/health"))
            .expect("redirect endpoint should parse");
        (endpoint, redirect_task, target_task)
    }

    fn local_probe_client(endpoint: &Url) -> BaseHttpClient {
        BaseHttpClient::new_for_provider_no_redirect(
            "health_probe",
            BaseConfig {
                api_base: Some(endpoint.to_string()),
                endpoint_access: ProviderEndpointAccess::PrivateNetwork,
                timeout: 2,
                ..Default::default()
            },
        )
        .expect("local policy probe client should build")
    }

    async fn wait_for_health(deployment: &Deployment, expected: HealthStatus) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while deployment.state.health_status() != expected {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("health transition should complete");
    }

    async fn test_deployment(
        id: &str,
        provider: Provider,
        policy: HealthCheckPolicy,
    ) -> Deployment {
        Deployment::new(
            id.to_string(),
            provider,
            "gpt-test".to_string(),
            "gpt-test".to_string(),
        )
        .with_config(DeploymentConfig {
            timeout_secs: 2,
            health_check_policy: Some(policy),
            ..DeploymentConfig::default()
        })
    }

    #[tokio::test]
    async fn custom_endpoint_uses_expected_status_codes() {
        let provider = test_provider(None).await;
        let (healthy_endpoint, healthy_server) = status_server(204).await;
        let client = local_probe_client(&healthy_endpoint);
        assert_eq!(
            execute_probe(
                &provider,
                "gpt-test",
                &test_policy(Some(healthy_endpoint)),
                Some(&client),
            )
            .await,
            Ok(())
        );
        let request = healthy_server.await.expect("test server should stop");
        let request = String::from_utf8_lossy(&request).to_ascii_lowercase();
        assert!(!request.contains("authorization:"));

        let (failed_endpoint, failed_server) = status_server(500).await;
        let client = local_probe_client(&failed_endpoint);
        assert_eq!(
            execute_probe(
                &provider,
                "gpt-test",
                &test_policy(Some(failed_endpoint)),
                Some(&client),
            )
            .await,
            Err(ProbeFailure::UnexpectedStatus(500))
        );
        failed_server.await.expect("test server should stop");
    }

    #[tokio::test]
    async fn custom_endpoint_observes_redirect_status_without_following_it() {
        let provider = test_provider(None).await;
        let (endpoint, redirect_task, redirect_target) = redirect_server().await;
        let client = local_probe_client(&endpoint);
        let mut policy = test_policy(Some(endpoint));
        policy.expected_codes = vec![302];

        let result = execute_probe(&provider, "gpt-test", &policy, Some(&client)).await;
        redirect_task.await.expect("redirect server should stop");
        let target_was_requested = redirect_target.await.expect("redirect target should stop");

        assert_eq!(result, Ok(()));
        assert!(!target_was_requested);

        let (endpoint, redirect_task, redirect_target) = redirect_server().await;
        let client = local_probe_client(&endpoint);
        policy.endpoint = Some(endpoint);
        policy.expected_codes = vec![200];

        let result = execute_probe(&provider, "gpt-test", &policy, Some(&client)).await;
        redirect_task.await.expect("redirect server should stop");
        let target_was_requested = redirect_target.await.expect("redirect target should stop");

        assert_eq!(result, Err(ProbeFailure::UnexpectedStatus(302)));
        assert!(!target_was_requested);
    }

    #[tokio::test]
    async fn public_custom_endpoint_is_rejected_before_socket_connect() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("public probe listener should bind");
        let address = listener
            .local_addr()
            .expect("listener address should exist");
        let endpoint = Url::parse(&format!("http://{address}/health"))
            .expect("public probe endpoint should parse");
        let provider = test_provider(None).await;
        let mut policy = test_policy(Some(endpoint));
        policy.endpoint_access = ProviderEndpointAccess::PublicOnly;
        let router = Router::new(RouterConfig::default());
        router.add_deployment(test_deployment("public-probe", provider, policy).await);

        assert!(router.start_configured_health_checks().is_err());
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "rejected public probe must not establish a loopback connection"
        );
    }

    #[tokio::test]
    async fn custom_probe_preserves_policy_error_details() {
        let provider = test_provider(None).await;
        let client_endpoint = Url::parse("http://127.0.0.1:18080/health").unwrap();
        let client = local_probe_client(&client_endpoint);
        let policy = test_policy(Some(Url::parse("http://127.0.0.1:18081/health").unwrap()));

        let result = execute_probe(&provider, "gpt-test", &policy, Some(&client)).await;
        assert!(matches!(
            result,
            Err(ProbeFailure::Request(message)) if message.contains("authority")
        ));
    }

    #[tokio::test]
    async fn safe_native_probe_verifies_the_configured_chat_model() {
        let (endpoint, server) = status_server(200).await;
        let provider = Provider::Anthropic(
            AnthropicProvider::new(
                AnthropicConfig::new_test("sk-ant-native-probe")
                    .with_base_url(endpoint.origin().ascii_serialization())
                    .with_endpoint_access(ProviderEndpointAccess::PrivateNetwork)
                    .with_allow_unknown_models(true)
                    .with_configured_models(vec!["probe-model".to_string()]),
            )
            .expect("test Anthropic provider should be valid"),
        );
        let mut policy = test_policy(None);
        policy.expected_codes = vec![200];

        assert_eq!(
            execute_probe(&provider, "probe-model", &policy, None).await,
            Ok(())
        );
        let request = server.await.expect("test server should stop");
        let request = String::from_utf8(request).expect("probe request should be UTF-8");
        assert!(request.starts_with("POST /v1/messages "));
        assert!(request.contains(r#""model":"probe-model""#));
        assert!(request.contains(r#""max_tokens":1"#));
    }

    #[tokio::test]
    async fn threshold_recovery_and_cooldown_transitions_are_enforced() {
        let provider = test_provider(None).await;
        let router = Router::new(RouterConfig::default());
        router.add_deployment(test_deployment("one", provider.clone(), test_policy(None)).await);
        router.add_deployment(test_deployment("two", provider.clone(), test_policy(None)).await);
        let deployment = router.get_deployment("one").unwrap();
        let second = router.get_deployment("two").unwrap();
        let policy = test_policy(None);
        let deployments = vec![Arc::clone(&deployment), Arc::clone(&second)];
        let mut failures = 0;
        assert_eq!(
            apply_probe_result(&policy, &deployments, false, &mut failures),
            Duration::from_secs(30)
        );
        assert_eq!(deployment.state.health_status(), HealthStatus::Degraded);
        assert_eq!(second.state.health_status(), HealthStatus::Degraded);
        assert_eq!(
            deployment.state.probe_health_status(),
            HealthStatus::Degraded
        );
        assert_eq!(
            apply_probe_result(&policy, &deployments, false, &mut failures),
            Duration::from_secs(60)
        );
        assert_eq!(deployment.state.health_status(), HealthStatus::Unhealthy);
        assert_eq!(second.state.health_status(), HealthStatus::Unhealthy);
        assert_eq!(
            deployment.state.probe_health_status(),
            HealthStatus::Unhealthy
        );

        deployment.record_failure();
        assert_eq!(deployment.state.health_status(), HealthStatus::Unhealthy);
        assert_eq!(
            apply_probe_result(&policy, &deployments, true, &mut failures),
            Duration::from_secs(30)
        );
        assert_eq!(failures, 0);
        assert_eq!(deployment.state.health_status(), HealthStatus::Healthy);
        assert_eq!(second.state.health_status(), HealthStatus::Healthy);
        assert_eq!(
            deployment.state.probe_health_status(),
            HealthStatus::Healthy
        );
        assert_eq!(second.state.probe_health_status(), HealthStatus::Healthy);

        deployment.enter_cooldown(60);
        apply_probe_result(&policy, &deployments, false, &mut failures);
        apply_probe_result(&policy, &deployments, false, &mut failures);
        assert_eq!(deployment.state.health_status(), HealthStatus::Cooldown);
        assert!(deployment.state.probe_unhealthy.load(Ordering::Relaxed));
        deployment.state.cooldown_until.store(1, Ordering::Relaxed);
        assert!(!deployment.is_in_cooldown());
        assert_eq!(deployment.state.health_status(), HealthStatus::Unhealthy);

        deployment.enter_cooldown(60);
        apply_probe_result(&policy, &deployments, true, &mut failures);
        assert_eq!(deployment.state.health_status(), HealthStatus::Cooldown);
        assert!(!deployment.state.probe_unhealthy.load(Ordering::Relaxed));
        deployment.state.cooldown_until.store(1, Ordering::Relaxed);
        assert!(!deployment.is_in_cooldown());
        assert_eq!(deployment.state.health_status(), HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn same_probe_health_update_keeps_stable_states() {
        let provider = test_provider(None).await;

        for status in [
            HealthStatus::Healthy,
            HealthStatus::Degraded,
            HealthStatus::Unhealthy,
        ] {
            let deployment = test_deployment("stable", provider.clone(), test_policy(None)).await;
            deployment
                .state
                .health
                .store(status as u8, Ordering::Relaxed);

            update_probe_health(&deployment, status);

            assert_eq!(deployment.state.health_status(), status);
            assert_eq!(
                deployment.state.probe_unhealthy.load(Ordering::Relaxed),
                status == HealthStatus::Unhealthy
            );
        }
    }

    #[tokio::test]
    async fn probe_loop_runs_immediately_and_continues_after_failure() {
        let provider = test_provider(None).await;
        let (endpoint, mut requests, server) = sequence_server(vec![500, 204]).await;
        let mut policy = test_policy(Some(endpoint));
        policy.failure_threshold = 1;
        policy.interval_secs = 1;
        policy.recovery_timeout_secs = 1;
        let router = Router::new(RouterConfig::default());
        router.add_deployment(test_deployment("one", provider.clone(), policy.clone()).await);
        let deployment = router.get_deployment("one").unwrap();
        let supervisor = ProbeSupervisor {
            routing_snapshot: Arc::clone(&router.routing_snapshot),
            wakeup: Arc::clone(&router.health_probe_wakeup),
        };
        let probe_task = tokio::spawn(run_probe_loop(supervisor));
        tokio::time::timeout(Duration::from_millis(500), requests.recv())
            .await
            .expect("first probe should run immediately")
            .expect("first probe should be observed");
        wait_for_health(&deployment, HealthStatus::Unhealthy).await;
        tokio::time::timeout(Duration::from_secs(2), requests.recv())
            .await
            .expect("probe loop should continue after failure")
            .expect("second probe should be observed");
        wait_for_health(&deployment, HealthStatus::Healthy).await;

        probe_task.abort();
        server.await.expect("test server should stop");
    }

    #[tokio::test]
    async fn router_groups_probe_tasks_and_aborts_them_on_drop() {
        let provider = test_provider(None).await;
        let endpoint = Url::parse("http://127.0.0.1:9/health").expect("test endpoint should parse");
        let policy = test_policy(Some(endpoint));
        let router = Router::new(RouterConfig::default());
        router.add_deployment(test_deployment("one", provider.clone(), policy.clone()).await);
        router.add_deployment(test_deployment("two", provider, policy).await);

        assert_eq!(router.start_configured_health_checks().unwrap(), 1);
        assert_eq!(router.health_probe_task_count(), 1);
        let abort_handle = router
            .health_probe_tasks
            .lock()
            .values()
            .next()
            .expect("probe task should exist")
            .abort_handle();

        drop(router);
        tokio::task::yield_now().await;
        assert!(abort_handle.is_finished());
    }

    #[tokio::test]
    async fn global_switch_disables_probe_tasks() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("disabled probe listener should bind");
        let address = listener
            .local_addr()
            .expect("disabled probe address should exist");
        let provider = test_provider(Some(format!("http://{address}/v1"))).await;
        let config = RouterConfig {
            enable_pre_call_checks: false,
            ..RouterConfig::default()
        };
        let router = Router::new(config);
        router.add_deployment(test_deployment("one", provider, test_policy(None)).await);

        assert_eq!(router.start_configured_health_checks().unwrap(), 0);
        assert_eq!(router.health_probe_task_count(), 0);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "disabled health checks must not create background upstream traffic"
        );
    }

    #[test]
    fn programmatic_deployment_has_no_probe_policy() {
        assert!(DeploymentConfig::default().health_check_policy.is_none());
        assert_eq!(
            DeploymentState::default().health_status(),
            HealthStatus::Healthy
        );
    }
}
