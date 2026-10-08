//! Native Gemini streams retain unknown admission and settle observed usage exactly once.
use crate::config::Config;
use crate::config::models::provider::ProviderConfig;
use crate::core::net::ProviderEndpointAccess;
use crate::core::router::deployment::{Deployment, current_timestamp};
use crate::core::router::{RouterConfig, UnifiedRouter};
use crate::core::types::model::ProviderCapability;
use crate::storage::redis::RedisPool;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, web};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::Relaxed},
};
use std::time::Duration;

const MODEL: &str = "gemini-2.5-flash";
const ROUTE: &str = "/v1beta/models/gemini-2.5-flash:streamGenerateContent";
const OUTPUT: &str = "gemini-stream-usage-marker";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ending {
    BeforeOutput,
    Disconnect,
    Complete,
    ProviderError,
    IdleBeforeOutput,
    IdleDisconnect,
    DelayedComplete,
}

#[derive(Clone)]
struct Upstream {
    calls: Arc<AtomicUsize>,
    error_ready: Arc<tokio::sync::Notify>,
    ending: Ending,
    usage: Option<u64>,
}

async fn upstream(
    state: web::Data<Upstream>,
    req: HttpRequest,
    body: web::Json<Value>,
) -> HttpResponse {
    assert_eq!(req.path(), ROUTE);
    assert_eq!(body["contents"][0]["parts"][0]["text"], "Hello");
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 64);
    let first = state.calls.fetch_add(1, Relaxed) == 0;
    let ending = if first {
        state.ending
    } else {
        Ending::Complete
    };
    let mut chunk = json!({"candidates":[{"content":{"role":"model","parts":[{"text":OUTPUT}]}}]});
    if let Some(tokens) = state.usage {
        chunk["usageMetadata"] = json!({
            "promptTokenCount":tokens,"candidatesTokenCount":0,"totalTokenCount":tokens
        });
    }
    let packet = Bytes::from(format!("data: {chunk}\n\n"));
    let stream = futures::stream::unfold(
        (0_u8, packet, state.error_ready.clone()),
        move |(phase, packet, error_ready)| async move {
            let bytes = match (ending, phase) {
                (Ending::IdleBeforeOutput | Ending::DelayedComplete, 0) => {
                    Ok(Bytes::from_static(b": idle-upstream\n\n"))
                }
                (Ending::IdleBeforeOutput | Ending::IdleDisconnect, 1..) => {
                    std::future::pending::<()>().await;
                    unreachable!("idle upstream emits no further bytes");
                }
                (Ending::DelayedComplete, 1) => {
                    tokio::time::sleep(Duration::from_millis(2200)).await;
                    Ok(packet.clone())
                }
                (Ending::BeforeOutput, _) | (Ending::Disconnect, 1..) => {
                    if phase > 0 {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Ok(Bytes::from_static(b": keepalive\n\n"))
                }
                (_, 0) => Ok(packet.clone()),
                (Ending::ProviderError, 1) => {
                    // Fail the actual HTTP chunked body only after the client
                    // received candidate output. This cannot become a fused
                    // first read that loses the earlier usage observation.
                    error_ready.notified().await;
                    Err(std::io::Error::other(
                        "intentional Gemini upstream body failure",
                    ))
                }
                (Ending::Complete, 1) | (Ending::DelayedComplete, 2) => Ok(Bytes::from_static(
                    b"data: {\"candidates\":[{\"finishReason\":\"STOP\"}]}\n\n",
                )),
                _ => return None,
            };
            Some((bytes, (phase.saturating_add(1), packet, error_ready)))
        },
    );
    HttpResponse::Ok()
        .insert_header(("content-type", "text/event-stream"))
        .streaming(stream)
}

#[tokio::test]
async fn gemini_native_http_stream_usage_survives_disconnect_and_terminal_settlement() {
    for shared in [false, true] {
        let redis_url = if shared {
            match std::env::var("REDIS_URL") {
                Ok(url) => Some(url),
                Err(_) if std::env::var_os("CI").is_some() => {
                    panic!("CI requires actual REDIS_URL")
                }
                Err(_) => {
                    eprintln!("Skipping shared Gemini HTTP stream cases: REDIS_URL unset");
                    continue;
                }
            }
        } else {
            None
        };
        // Ten cases per backend. All-zero Gemini metadata is intentionally
        // unknown; native Responses and trusted internal usage have separate
        // explicit-zero contracts.
        let cases = [
            (Ending::BeforeOutput, Some(3)),
            (Ending::Disconnect, None),
            (Ending::Disconnect, Some(0)),
            (Ending::Disconnect, Some(3)),
            (Ending::Complete, None),
            (Ending::Complete, Some(0)),
            (Ending::Complete, Some(3)),
            (Ending::ProviderError, None),
            (Ending::ProviderError, Some(0)),
            (Ending::ProviderError, Some(3)),
        ];
        for (ending, usage) in cases {
            let mut fixture = Fixture::new(redis_url.as_deref(), ending, usage).await;
            let body = request_body();
            let endpoint = format!("{}{ROUTE}", fixture.base_url);
            let response = fixture
                .client
                .post(&endpoint)
                .json(&body)
                .send()
                .await
                .unwrap();
            assert!(
                response.status().is_success(),
                "{ending:?}/{usage:?}/{shared}"
            );
            if ending == Ending::Complete {
                assert!(response.text().await.unwrap().contains(OUTPUT));
            } else {
                let mut stream = response.bytes_stream();
                let mut observed = Vec::new();
                let marker = if ending == Ending::BeforeOutput {
                    ": keepalive"
                } else {
                    OUTPUT
                };
                tokio::time::timeout(Duration::from_secs(5), async {
                    while !String::from_utf8_lossy(&observed).contains(marker) {
                        observed.extend_from_slice(
                            &stream.next().await.expect("body must remain open").unwrap(),
                        );
                    }
                })
                .await
                .expect("client must receive actual forwarded upstream bytes");
                assert_eq!(
                    fixture.deployment.state.active_requests.load(Relaxed),
                    1,
                    "forwarded bytes must belong to the still-held attempt"
                );
                assert_eq!(fixture.calls.load(Relaxed), 1);
                if ending == Ending::BeforeOutput {
                    assert!(!String::from_utf8_lossy(&observed).contains(OUTPUT));
                }
                if ending == Ending::ProviderError {
                    fixture.error_ready.notify_one();
                    tokio::time::timeout(Duration::from_secs(5), async {
                        while let Some(bytes) = stream.next().await {
                            observed.extend_from_slice(&bytes.unwrap());
                        }
                    })
                    .await
                    .expect("real upstream read error must terminate the gateway stream");
                    assert!(
                        String::from_utf8_lossy(&observed).contains("Gemini upstream stream error")
                    );
                }
                drop(stream);
            }
            let consumed = u64::from(ending != Ending::BeforeOutput);
            let successes = u64::from(ending == Ending::Complete);
            let failures = u64::from(ending == Ending::ProviderError);
            let known = usage.filter(|tokens| *tokens > 0);
            let tokens = known.unwrap_or(0) * consumed;
            let charged = if consumed == 0 {
                0
            } else {
                known.unwrap_or(fixture.estimate)
            };
            fixture
                .assert_accounting(consumed, tokens, successes, failures, charged)
                .await;
            assert_eq!(fixture.calls.load(Relaxed), 1);
            if matches!(ending, Ending::BeforeOutput | Ending::Disconnect) {
                assert_eq!(
                    fixture.deployment.state.health.load(Relaxed),
                    fixture.initial_health,
                    "client cancellation cannot change upstream health"
                );
            }

            let second = fixture
                .client
                .post(&endpoint)
                .json(&body)
                .send()
                .await
                .unwrap();
            if let Some(known) = known {
                assert!(
                    second.status().is_success(),
                    "known usage or pre-output cancellation must release excess reservation"
                );
                assert!(second.text().await.unwrap().contains(OUTPUT));
                assert_eq!(fixture.calls.load(Relaxed), 2);
                let total_tokens = tokens + known;
                fixture
                    .assert_accounting(
                        consumed + 1,
                        total_tokens,
                        successes + 1,
                        failures,
                        total_tokens,
                    )
                    .await;
                if consumed == 1 {
                    let third = fixture
                        .client
                        .post(&endpoint)
                        .json(&body)
                        .send()
                        .await
                        .unwrap();
                    assert!(
                        !third.status().is_success(),
                        "two known charges must contribute to TPM"
                    );
                    assert_eq!(fixture.calls.load(Relaxed), 2);
                    fixture
                        .assert_accounting(2, 6, successes + 1, failures, 6)
                        .await;
                }
            } else {
                assert!(
                    !second.status().is_success(),
                    "missing and all-zero usage retain the whole estimate"
                );
                assert_eq!(fixture.calls.load(Relaxed), 1);
                fixture
                    .assert_accounting(consumed, tokens, successes, failures, fixture.estimate)
                    .await;
            }
            fixture.shutdown().await;
        }
    }
}

#[tokio::test]
async fn gemini_idle_upstream_releases_disconnected_readers_and_keeps_half_closed_writers() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for shared in [false, true] {
        let redis_url = if shared {
            match std::env::var("REDIS_URL") {
                Ok(url) => Some(url),
                Err(_) if std::env::var_os("CI").is_some() => {
                    panic!("CI requires actual REDIS_URL")
                }
                Err(_) => {
                    eprintln!("Skipping shared Gemini idle stream cases: REDIS_URL unset");
                    continue;
                }
            }
        } else {
            None
        };
        for ending in [
            Ending::IdleBeforeOutput,
            Ending::IdleDisconnect,
            Ending::DelayedComplete,
        ] {
            let usage = (ending != Ending::IdleDisconnect).then_some(3);
            let mut fixture = Fixture::new(redis_url.as_deref(), ending, usage).await;
            let body = request_body().to_string();
            let mut socket =
                tokio::net::TcpStream::connect(fixture.base_url.trim_start_matches("http://"))
                    .await
                    .unwrap();
            let request = format!(
                "POST {ROUTE} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len(),
            );
            socket.write_all(request.as_bytes()).await.unwrap();
            let mut response = Vec::new();
            if ending == Ending::DelayedComplete {
                // A FIN on the request writer does not close its response reader.
                socket.shutdown().await.unwrap();
                tokio::time::timeout(Duration::from_secs(5), socket.read_to_end(&mut response))
                    .await
                    .expect("half-closed writer must receive the delayed Gemini completion")
                    .unwrap();
                let wire = String::from_utf8(response).unwrap();
                assert!(wire.starts_with("HTTP/1.1 200"), "{wire}");
                assert!(wire.contains(": keep-alive"), "{wire}");
                assert!(wire.contains(OUTPUT), "{wire}");
                assert!(wire.contains("STOP"), "{wire}");
                fixture.assert_accounting(1, 3, 1, 0, 3).await;
                assert_eq!(fixture.calls.load(Relaxed), 1);
            } else {
                let marker = if ending == Ending::IdleBeforeOutput {
                    ": idle-upstream"
                } else {
                    OUTPUT
                };
                tokio::time::timeout(Duration::from_secs(5), async {
                    let mut buffer = [0_u8; 4096];
                    while !String::from_utf8_lossy(&response).contains(marker) {
                        let read = socket.read(&mut buffer).await.unwrap();
                        assert!(
                            read > 0,
                            "idle stream must remain open until client disconnect"
                        );
                        response.extend_from_slice(&buffer[..read]);
                    }
                })
                .await
                .expect("read actual upstream bytes before dropping the response reader");
                assert!(String::from_utf8_lossy(&response).starts_with("HTTP/1.1 200"));
                assert_eq!(fixture.deployment.state.active_requests.load(Relaxed), 1);
                assert_eq!(fixture.calls.load(Relaxed), 1);
                // The upstream is now pending forever with no keepalive or timeout.
                // Downstream comments must expose this fully closed socket.
                drop(socket);
                let consumed = u64::from(ending == Ending::IdleDisconnect);
                let retained = if consumed == 0 { 0 } else { fixture.estimate };
                fixture.assert_accounting(consumed, 0, 0, 0, retained).await;
                assert_eq!(
                    fixture.deployment.state.health.load(Relaxed),
                    fixture.initial_health
                );
                assert!(!fixture.deployment.is_in_cooldown());
                let next = fixture
                    .client
                    .post(format!("{}{ROUTE}", fixture.base_url))
                    .json(&request_body())
                    .send()
                    .await
                    .unwrap();
                if consumed == 0 {
                    assert!(next.status().is_success());
                    assert!(next.text().await.unwrap().contains(OUTPUT));
                    fixture.assert_accounting(1, 3, 1, 0, 3).await;
                    assert_eq!(fixture.calls.load(Relaxed), 2);
                } else {
                    assert!(!next.status().is_success());
                    fixture
                        .assert_accounting(1, 0, 0, 0, fixture.estimate)
                        .await;
                    assert_eq!(fixture.calls.load(Relaxed), 1);
                }
            }
            fixture.shutdown().await;
        }
    }
}

fn request_body() -> Value {
    json!({"contents":[{"role":"user","parts":[{"text":"Hello"}]}],"generationConfig":{"maxOutputTokens":64}})
}

struct Fixture {
    base_url: String,
    client: reqwest::Client,
    calls: Arc<AtomicUsize>,
    error_ready: Arc<tokio::sync::Notify>,
    deployment: Arc<Deployment>,
    estimate: u64,
    initial_health: u8,
    connection: Option<redis::aio::MultiplexedConnection>,
    handles: Vec<actix_web::dev::ServerHandle>,
    tasks: Vec<tokio::task::JoinHandle<std::io::Result<()>>>,
}

impl Fixture {
    async fn new(redis_url: Option<&str>, ending: Ending, usage: Option<u64>) -> Self {
        let estimate = super::gemini_estimated_admission_tokens(&request_body(), MODEL).unwrap();
        // E + 3 permits a second known charge, then rejects a third. Unknown
        // usage consumes E and therefore rejects the second dispatch.
        assert!(estimate > 3);
        let calls = Arc::new(AtomicUsize::new(0));
        let error_ready = Arc::new(tokio::sync::Notify::new());
        let upstream_state = Upstream {
            calls: calls.clone(),
            error_ready: error_ready.clone(),
            ending,
            usage,
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream_address = listener.local_addr().unwrap();
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(upstream_state.clone()))
                .default_service(web::post().to(upstream))
        })
        .workers(1)
        .listen(listener)
        .unwrap()
        .run();
        let upstream_handle = server.handle();
        let upstream_task = tokio::spawn(server);
        let mut config = Config::default();
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = redis_url.is_some();
        config.gateway.storage.redis.allow_degraded = false;
        if let Some(url) = redis_url {
            config.gateway.storage.redis.url = url.into();
        }
        config.gateway.router.load_balancer.health_check_enabled = false;
        config.gateway.server.stream_idle_timeout = 0;
        config.gateway.guardrails.enabled = true;
        config.gateway.providers = vec![ProviderConfig {
            name: format!("gemini-stream-usage-{}", uuid::Uuid::new_v4()),
            provider_type: "openai_compatible".into(),
            api_key: "test-gemini-stream-usage-fixture-key".into(),
            base_url: Some(format!("http://{upstream_address}")),
            endpoint_access: ProviderEndpointAccess::PrivateNetwork,
            models: vec![MODEL.into()],
            settings: std::collections::HashMap::from([("provider_name".into(), json!("gemini"))]),
            rpm: 100,
            tpm: u32::try_from(estimate + 3).unwrap(),
            max_concurrent_requests: 100,
            max_retries: 0,
            ..Default::default()
        }];
        let state = crate::server::HttpServer::new(&config)
            .await
            .unwrap()
            .state()
            .clone();
        // Keep the real factory and AppState services, but disable request
        // retries so an admission rejection cannot obscure dispatch counts.
        let router = UnifiedRouter::from_gateway_config(
            &config.gateway.providers,
            Some(RouterConfig {
                num_retries: 0,
                max_fallbacks: 0,
                enable_pre_call_checks: false,
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .with_admission_redis(state.storage.redis.clone())
        .with_circuit_redis(state.storage.redis.clone());
        let mut revision = state.pin_runtime().as_ref().clone();
        revision.unified_router = Arc::new(router);
        state.runtime.store(revision);
        let router = state.unified_router();
        let ids = router.get_deployments_for_model(MODEL);
        assert_eq!(ids.len(), 1);
        let deployment = router.get_deployment(&ids[0]).unwrap();
        assert!(
            deployment
                .provider
                .supports_capability_for_model(MODEL, &ProviderCapability::GeminiGenerateContent)
        );
        let initial_health = deployment.state.health.load(Relaxed);
        let mut connection = match redis_url {
            Some(url) => Some(
                redis::Client::open(url)
                    .unwrap()
                    .get_multiplexed_async_connection()
                    .await
                    .unwrap(),
            ),
            None => None,
        };
        wait_for_window(connection.as_mut()).await;
        deployment.state.reset_minute();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(crate::server::routes::ai::configure_routes)
        })
        .workers(1)
        .listen(listener)
        .unwrap()
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        let client = reqwest::Client::builder()
            .no_proxy()
            .http1_only()
            .pool_max_idle_per_host(0)
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();
        Self {
            base_url: format!("http://{address}"),
            client,
            calls,
            error_ready,
            deployment,
            estimate,
            initial_health,
            connection,
            handles: vec![handle, upstream_handle],
            tasks: vec![task, upstream_task],
        }
    }

    async fn assert_accounting(
        &mut self,
        requests: u64,
        tokens: u64,
        successes: u64,
        failures: u64,
        admission_tokens: u64,
    ) {
        let state = &self.deployment.state;
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if (state.active_requests.load(Relaxed), state.rpm_current.load(Relaxed), state.tpm_current.load(Relaxed), state.success_requests.load(Relaxed), state.fail_requests.load(Relaxed), state.total_requests.load(Relaxed)) == (0, requests, tokens, successes, failures, successes + failures) { break; }
                tokio::task::yield_now().await;
            }
        }).await.unwrap_or_else(|_| panic!("local stream accounting did not finish: active={} rpm={} tpm={} success={} fail={} total={}", state.active_requests.load(Relaxed), state.rpm_current.load(Relaxed), state.tpm_current.load(Relaxed), state.success_requests.load(Relaxed), state.fail_requests.load(Relaxed), state.total_requests.load(Relaxed)));
        if let Some(connection) = &mut self.connection {
            let key = RedisPool::admission_key(&self.deployment.shared_state_id());
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let counts: (u64, u64, u64) = redis::cmd("HMGET")
                        .arg(&key)
                        .arg(&["p", "r", "t"])
                        .query_async(&mut *connection)
                        .await
                        .unwrap();
                    if counts == (0, requests, admission_tokens) {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("shared admission must finish once");
            let fields: Vec<String> = redis::cmd("HKEYS")
                .arg(&key)
                .query_async(&mut *connection)
                .await
                .unwrap();
            assert!(
                !fields
                    .iter()
                    .any(|field| field.starts_with("l:") || field.starts_with("p:"))
            );
            // A client cancellation is neutral on the shared circuit too;
            // accepted read errors and EOF each contribute exactly one outcome.
            let circuit_key = RedisPool::circuit_key(&self.deployment.shared_state_id());
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let counts: (Option<u64>, Option<u64>) = redis::cmd("HMGET")
                        .arg(&circuit_key)
                        .arg(&["tot", "fail"])
                        .query_async(&mut *connection)
                        .await
                        .unwrap();
                    if (counts.0.unwrap_or(0), counts.1.unwrap_or(0))
                        == (successes + failures, failures)
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("shared circuit outcomes must agree with the original stream owner");
        }
    }

    async fn shutdown(mut self) {
        if let Some(connection) = &mut self.connection {
            let id = self.deployment.shared_state_id();
            let _: i64 = redis::cmd("DEL")
                .arg(&[RedisPool::admission_key(&id), RedisPool::circuit_key(&id)])
                .query_async(connection)
                .await
                .unwrap();
        }
        for handle in self.handles {
            handle.stop(false).await;
        }
        for task in self.tasks {
            task.await.unwrap().unwrap();
        }
    }
}

async fn wait_for_window(mut connection: Option<&mut redis::aio::MultiplexedConnection>) {
    loop {
        let local = current_timestamp() % 60;
        let shared = if let Some(connection) = connection.as_deref_mut() {
            let (seconds, _): (u64, u64) =
                redis::cmd("TIME").query_async(connection).await.unwrap();
            seconds % 60
        } else {
            local
        };
        let delay = [local, shared]
            .into_iter()
            .filter(|second| *second >= 40)
            .map(|second| 61 - second)
            .max();
        match delay {
            Some(seconds) => tokio::time::sleep(Duration::from_secs(seconds)).await,
            None => return,
        }
    }
}
