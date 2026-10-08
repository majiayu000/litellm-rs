//! Real downstream disconnects preserve the exact streaming attempt's admission.
use crate::config::Config;
use crate::config::models::provider::ProviderConfig;
use crate::core::models::openai::ChatCompletionRequest;
use crate::core::net::ProviderEndpointAccess;
use crate::core::router::deployment::{Deployment, current_timestamp};
use crate::core::router::{RouterConfig, UnifiedRouter};
use crate::core::types::model::ProviderCapability;
use crate::storage::redis::RedisPool;
use actix_web::{App, HttpResponse, HttpServer, web};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::Relaxed},
};
use std::time::Duration;

const MODEL: &str = "gpt-4o-mini";
const OUTPUT: &str = "stream-usage-marker";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ending {
    BeforeOutput,
    Disconnect,
    Complete,
    DelayedComplete,
    TerminalUsage,
    UsageSequence,
    UsageOnlySequence,
    ProviderError,
}

#[derive(Clone)]
struct Upstream {
    calls: Arc<AtomicUsize>,
    entered: Arc<tokio::sync::Notify>,
    error_ready: Arc<tokio::sync::Notify>,
    ending: Ending,
    usage: Option<Value>,
}

async fn upstream(state: web::Data<Upstream>, body: web::Json<Value>) -> HttpResponse {
    assert_eq!(body["model"], MODEL);
    assert_eq!(body["messages"][0]["content"], "Hello");
    assert_eq!(body["stream"], true);
    let first = state.calls.fetch_add(1, Relaxed) == 0;
    let ending = if first {
        state.ending
    } else {
        Ending::Complete
    };
    let mut chunk = json!({"id":"usage-fixture","object":"chat.completion.chunk","created":1,"model":MODEL,
        "choices":[{"index":0,"delta":{"role":"assistant","content":OUTPUT},"finish_reason":null}]});
    if matches!(ending, Ending::UsageSequence | Ending::UsageOnlySequence) {
        chunk["usage"] = json!({"prompt_tokens":3,"completion_tokens":0,"total_tokens":3});
        if ending == Ending::UsageOnlySequence {
            chunk["choices"] = json!([]);
        }
    } else if ending != Ending::TerminalUsage
        && let Some(usage) = &state.usage
    {
        chunk["usage"] = usage.clone();
    }
    let packet = Bytes::from(format!("data: {chunk}\n\n"));
    let mut terminal_chunk = json!({"id":"usage-fixture","object":"chat.completion.chunk",
        "created":1,"model":MODEL,"choices":[]});
    if let Some(usage) = &state.usage {
        terminal_chunk["usage"] = usage.clone();
    }
    let terminal_usage = Bytes::from(format!("data: {terminal_chunk}\n\n"));
    let stream = futures::stream::unfold(
        (0_u8, packet, state.error_ready.clone(), terminal_usage),
        move |(phase, packet, error_ready, terminal_usage)| async move {
            let bytes = match (ending, phase) {
            (Ending::BeforeOutput, _) => {
                if phase > 0 { tokio::time::sleep(Duration::from_millis(50)).await; }
                Bytes::from_static(b": keepalive\n\n")
            }
            (Ending::DelayedComplete, 0) => {
                tokio::time::sleep(Duration::from_millis(1500)).await;
                packet.clone()
            }
            (_, 0) => packet.clone(),
            (Ending::Disconnect, _) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Bytes::from_static(b": keepalive\n\n")
            }
            // A valid JSON frame without choices triggers the real upstream
            // protocol-error path after already accepted candidate output.
            (Ending::ProviderError, 1) => {
                error_ready.notified().await;
                Bytes::from_static(b"data: {\"id\":\"malformed-stream\"}\n\n")
            },
            (Ending::UsageSequence | Ending::UsageOnlySequence, 1) => terminal_usage.clone(),
            (Ending::Complete | Ending::DelayedComplete | Ending::TerminalUsage, 1) | (Ending::UsageSequence | Ending::UsageOnlySequence, 2) => Bytes::from_static(b"data: {\"id\":\"usage-fixture\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n"),
            (Ending::TerminalUsage, 2) => terminal_usage.clone(),
            (Ending::Complete | Ending::DelayedComplete, 2) | (Ending::TerminalUsage | Ending::UsageSequence | Ending::UsageOnlySequence, 3) => Bytes::from_static(b"data: [DONE]\n\n"),
            _ => return None,
        };
            Some((
                Ok::<_, std::io::Error>(bytes),
                (phase.saturating_add(1), packet, error_ready, terminal_usage),
            ))
        },
    );
    state.entered.notify_one();
    HttpResponse::Ok()
        .insert_header(("content-type", "text/event-stream"))
        .streaming(stream)
}

#[tokio::test]
async fn gateway_http_stream_usage_survives_disconnect_and_terminal_settlement() {
    for route in ["/v1/chat/completions", "/v1/completions", "/v1/responses"] {
        for shared in [false, true] {
            let redis_url = if shared {
                match std::env::var("REDIS_URL") {
                    Ok(url) => Some(url),
                    Err(_) if std::env::var_os("CI").is_some() => {
                        panic!("CI requires actual REDIS_URL")
                    }
                    Err(_) => {
                        eprintln!("Skipping shared HTTP stream cases: REDIS_URL unset");
                        continue;
                    }
                }
            } else {
                None
            };
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
                // Raw all-zero OpenAI chat usage is unknown under this protocol;
                // trusted internal and native Responses zero counts are separate.
                let known_usage = usage.filter(|tokens| *tokens > 0);
                let mut fixture = Fixture::new(route, redis_url.as_deref(), ending, usage).await;
                let body = request_body(route);
                let endpoint = format!("{}{route}", fixture.base_url);
                if ending == Ending::BeforeOutput {
                    let mut request = Box::pin(fixture.client.post(&endpoint).json(&body).send());
                    tokio::time::timeout(Duration::from_secs(5), async {
                        tokio::select! {
                            response = request.as_mut() => {
                                let response = response.unwrap();
                                assert!(response.status().is_success());
                                drop(response);
                            }
                            () = fixture.entered.notified() => {}
                        }
                    })
                    .await
                    .expect("upstream must accept the pre-output request");
                    drop(request);
                } else {
                    let response = fixture
                        .client
                        .post(&endpoint)
                        .json(&body)
                        .send()
                        .await
                        .unwrap();
                    assert!(
                        response.status().is_success(),
                        "{route}/{ending:?}/{usage:?}/{shared}"
                    );
                    if matches!(ending, Ending::Disconnect | Ending::ProviderError) {
                        let mut stream = response.bytes_stream();
                        let mut observed = Vec::new();
                        tokio::time::timeout(Duration::from_secs(5), async {
                            while !String::from_utf8_lossy(&observed).contains(OUTPUT) {
                                observed.extend_from_slice(
                                    &stream.next().await.expect("body must remain open").unwrap(),
                                );
                            }
                        })
                        .await
                        .expect("observe actual forwarded output before disconnecting");
                        if ending == Ending::ProviderError {
                            // Release the malformed frame only after the real
                            // downstream client observed accepted output.
                            fixture.error_ready.notify_one();
                            while let Some(bytes) = stream.next().await {
                                observed.extend_from_slice(&bytes.unwrap());
                            }
                            assert_provider_error(&observed, &fixture.case);
                        }
                        drop(stream);
                    } else {
                        let text = response.text().await.unwrap();
                        assert!(text.contains(OUTPUT));
                    }
                }
                let consumed = u64::from(ending != Ending::BeforeOutput);
                let successes = u64::from(ending == Ending::Complete);
                let failures = u64::from(ending == Ending::ProviderError);
                let tokens = known_usage.unwrap_or(0) * consumed;
                fixture
                    .assert_accounting(
                        consumed,
                        tokens,
                        successes,
                        failures,
                        if consumed == 0 {
                            0
                        } else {
                            known_usage.unwrap_or(fixture.estimate)
                        },
                    )
                    .await;
                assert_eq!(fixture.calls.load(Relaxed), 1);

                let second = fixture
                    .client
                    .post(&endpoint)
                    .json(&body)
                    .send()
                    .await
                    .unwrap();
                if let Some(known) = known_usage {
                    assert!(
                        second.status().is_success(),
                        "known usage or pre-output cancellation must release the excess reservation"
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
                    if known == 3 && consumed == 1 {
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
                        "unknown usage must retain the whole estimate"
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
}

#[tokio::test]
async fn gateway_terminal_usage_rejects_untrusted_counts_and_keeps_valid_counts() {
    for route in ["/v1/chat/completions", "/v1/completions", "/v1/responses"] {
        for shared in [false, true] {
            let redis_url = if shared {
                match std::env::var("REDIS_URL") {
                    Ok(url) => Some(url),
                    Err(_) if std::env::var_os("CI").is_some() => {
                        panic!("CI requires actual REDIS_URL")
                    }
                    Err(_) => {
                        eprintln!("Skipping shared terminal usage cases: REDIS_URL unset");
                        continue;
                    }
                }
            } else {
                None
            };
            for (reported, known) in [
                (
                    json!({"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}),
                    None,
                ),
                (
                    json!({"prompt_tokens":3,"completion_tokens":2,"total_tokens":0}),
                    None,
                ),
                (
                    json!({"prompt_tokens":3,"completion_tokens":2,"total_tokens":3}),
                    None,
                ),
                (
                    json!({"prompt_tokens":3,"completion_tokens":0,"total_tokens":3}),
                    Some(3_u64),
                ),
            ] {
                let mut fixture = Fixture::with_usage(
                    route,
                    redis_url.as_deref(),
                    Ending::TerminalUsage,
                    Some(reported),
                )
                .await;
                let endpoint = format!("{}{route}", fixture.base_url);
                let body = request_body(route);
                let first = fixture
                    .client
                    .post(&endpoint)
                    .json(&body)
                    .send()
                    .await
                    .unwrap();
                assert!(first.status().is_success(), "{}", fixture.case);
                assert!(first.text().await.unwrap().contains(OUTPUT));
                fixture
                    .assert_accounting(
                        1,
                        known.unwrap_or(0),
                        1,
                        0,
                        known.unwrap_or(fixture.estimate),
                    )
                    .await;
                assert_eq!(fixture.calls.load(Relaxed), 1);

                let second = fixture
                    .client
                    .post(&endpoint)
                    .json(&body)
                    .send()
                    .await
                    .unwrap();
                if let Some(tokens) = known {
                    assert!(second.status().is_success(), "{}", fixture.case);
                    assert!(second.text().await.unwrap().contains(OUTPUT));
                    fixture
                        .assert_accounting(2, 2 * tokens, 2, 0, 2 * tokens)
                        .await;
                    assert_eq!(fixture.calls.load(Relaxed), 2);
                    let third = fixture
                        .client
                        .post(&endpoint)
                        .json(&body)
                        .send()
                        .await
                        .unwrap();
                    assert!(
                        !third.status().is_success(),
                        "known usage must contribute to TPM"
                    );
                    fixture.assert_accounting(2, 6, 2, 0, 6).await;
                    assert_eq!(fixture.calls.load(Relaxed), 2);
                } else {
                    assert!(
                        !second.status().is_success(),
                        "{}: untrusted terminal usage must retain the estimate",
                        fixture.case
                    );
                    fixture
                        .assert_accounting(1, 0, 1, 0, fixture.estimate)
                        .await;
                    assert_eq!(fixture.calls.load(Relaxed), 1);
                }
                fixture.shutdown().await;
            }
        }
    }
}

#[tokio::test]
async fn gateway_later_invalid_usage_revokes_earlier_usage_without_refunding_tpm() {
    let invalid_usage = [
        json!({"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}),
        json!({"prompt_tokens":3,"completion_tokens":2,"total_tokens":0}),
        json!({"prompt_tokens":3,"completion_tokens":2,"total_tokens":3}),
        json!({"prompt_tokens":3,"completion_tokens":2,"total_tokens":8}),
        json!({"prompt_tokens":3,"completion_tokens":2}),
        json!({"prompt_tokens":3,"total_tokens":5}),
        json!({"completion_tokens":2,"total_tokens":5}),
        json!({"prompt_tokens":-1,"completion_tokens":2,"total_tokens":1}),
        json!({"prompt_tokens":3.5,"completion_tokens":2,"total_tokens":5.5}),
        json!({"prompt_tokens":"3","completion_tokens":2,"total_tokens":5}),
        json!({"prompt_tokens":u64::MAX,"completion_tokens":1,"total_tokens":0}),
        json!({"prompt_tokens":3,"completion_tokens":2,"total_tokens":5,
            "prompt_tokens_details":{"cached_tokens":"bad"}}),
        json!("not an object"),
        json!([]),
    ];
    for route in ["/v1/chat/completions", "/v1/completions", "/v1/responses"] {
        for shared in [false, true] {
            let redis_url = if shared {
                match std::env::var("REDIS_URL") {
                    Ok(url) => Some(url),
                    Err(_) if std::env::var_os("CI").is_some() => {
                        panic!("CI requires actual REDIS_URL")
                    }
                    Err(_) => {
                        eprintln!("Skipping shared usage invalidation cases: REDIS_URL unset");
                        continue;
                    }
                }
            } else {
                None
            };
            // Exercise every malformed shape after real output, plus the case
            // where only positive usage proved that the provider accepted work.
            let cases = invalid_usage
                .iter()
                .cloned()
                .map(|usage| (Ending::UsageSequence, usage))
                .chain(std::iter::once((
                    Ending::UsageOnlySequence,
                    invalid_usage[0].clone(),
                )));
            for (ending, reported) in cases {
                let mut fixture =
                    Fixture::with_usage(route, redis_url.as_deref(), ending, Some(reported)).await;
                let endpoint = format!("{}{route}", fixture.base_url);
                let body = request_body(route);
                let first = fixture
                    .client
                    .post(&endpoint)
                    .json(&body)
                    .send()
                    .await
                    .unwrap();
                assert!(first.status().is_success(), "{}", fixture.case);
                let wire = first.text().await.unwrap();
                assert_eq!(wire.contains(OUTPUT), ending == Ending::UsageSequence);
                let errors: Vec<Value> = wire
                    .lines()
                    .filter_map(|line| line.strip_prefix("data:"))
                    .filter_map(|data| serde_json::from_str::<Value>(data.trim()).ok())
                    .filter_map(|event| event.get("error").cloned())
                    .collect();
                assert_eq!(
                    errors,
                    vec![json!({
                        "type":"server_error",
                        "code":"internal_error",
                        "message":"Streaming error for openai_like: chat.completion.usage_invalidated at position None"
                    })],
                    "{}: {wire}",
                    fixture.case
                );
                assert_eq!(wire.matches("data: [DONE]").count(), 1);
                fixture
                    .assert_accounting(1, 0, 0, 1, fixture.estimate)
                    .await;
                assert_eq!(fixture.calls.load(Relaxed), 1);
                assert!(fixture.deployment.is_healthy());
                assert!(!fixture.deployment.is_in_cooldown());
                let second = fixture
                    .client
                    .post(&endpoint)
                    .json(&body)
                    .send()
                    .await
                    .unwrap();
                assert!(
                    !second.status().is_success(),
                    "{}: invalidated usage must retain the full estimate",
                    fixture.case
                );
                fixture
                    .assert_accounting(1, 0, 0, 1, fixture.estimate)
                    .await;
                assert_eq!(fixture.calls.load(Relaxed), 1);
                fixture.shutdown().await;
            }

            for reported in [
                None,
                Some(Value::Null),
                Some(json!({"prompt_tokens":3,"completion_tokens":0,"total_tokens":3})),
            ] {
                let second_known = reported.as_ref().is_some_and(|usage| !usage.is_null());
                let mut fixture = Fixture::with_usage(
                    route,
                    redis_url.as_deref(),
                    Ending::UsageSequence,
                    reported,
                )
                .await;
                let endpoint = format!("{}{route}", fixture.base_url);
                let body = request_body(route);
                let first = fixture
                    .client
                    .post(&endpoint)
                    .json(&body)
                    .send()
                    .await
                    .unwrap();
                assert!(first.status().is_success(), "{}", fixture.case);
                let wire = first.text().await.unwrap();
                assert!(wire.contains(OUTPUT));
                assert!(!wire.contains("usage_invalidated"));
                fixture.assert_accounting(1, 3, 1, 0, 3).await;
                let second = fixture
                    .client
                    .post(&endpoint)
                    .json(&body)
                    .send()
                    .await
                    .unwrap();
                assert!(second.status().is_success(), "{}", fixture.case);
                assert!(second.text().await.unwrap().contains(OUTPUT));
                fixture
                    .assert_accounting(
                        2,
                        if second_known { 6 } else { 3 },
                        2,
                        0,
                        if second_known {
                            6
                        } else {
                            3 + fixture.estimate
                        },
                    )
                    .await;
                assert_eq!(fixture.calls.load(Relaxed), 2);
                fixture.shutdown().await;
            }
        }
    }
}

#[tokio::test]
async fn gateway_stream_keepalive_preserves_half_closed_writers() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for route in ["/v1/chat/completions", "/v1/completions", "/v1/responses"] {
        let mut fixture = Fixture::new(route, None, Ending::DelayedComplete, Some(3)).await;
        let body = request_body(route).to_string();
        let mut socket =
            tokio::net::TcpStream::connect(fixture.base_url.trim_start_matches("http://"))
                .await
                .unwrap();
        let request = format!(
            "POST {route} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len(),
        );
        socket.write_all(request.as_bytes()).await.unwrap();
        // Closing only the request writer must not cancel the response reader.
        socket.shutdown().await.unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), socket.read_to_end(&mut response))
            .await
            .expect("the half-closed writer must receive the delayed completion")
            .unwrap();
        let wire = String::from_utf8(response).unwrap();
        assert!(wire.starts_with("HTTP/1.1 200"), "{route}: {wire}");
        assert!(wire.contains(": keep-alive"), "{route}: {wire}");
        assert!(wire.contains(OUTPUT), "{route}: {wire}");
        assert!(wire.contains("[DONE]"), "{route}: {wire}");
        fixture.assert_accounting(1, 3, 1, 0, 3).await;
        assert_eq!(fixture.calls.load(Relaxed), 1);
        fixture.shutdown().await;
    }
}

fn assert_provider_error(observed: &[u8], case: &str) {
    let wire = std::str::from_utf8(observed).unwrap();
    let mut errors = Vec::new();
    let mut done = 0;
    for line in wire.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        if data.trim() == "[DONE]" {
            done += 1;
            continue;
        }
        let event: Value = serde_json::from_str(data.trim()).unwrap();
        if let Some(error) = event.get("error") {
            errors.push(error.clone());
        }
    }
    assert_eq!(errors.len(), 1, "{case}: {wire}");
    assert_eq!(
        errors[0],
        json!({
            "type":"server_error",
            "code":"internal_error",
            "message":"Streaming error for openai_like: chat.completion at position Some(0)"
        }),
        "{case}: {wire}"
    );
    assert_eq!(done, 1, "{case}: {wire}");
}

fn request_body(route: &str) -> Value {
    match route {
        "/v1/completions" => {
            json!({"model":MODEL,"prompt":"Hello","max_tokens":64,"stream":true,"stream_options":{"include_usage":true}})
        }
        "/v1/responses" => {
            json!({"model":MODEL,"input":"Hello","max_output_tokens":64,"stream":true,"store":false})
        }
        _ => {
            json!({"model":MODEL,"messages":[{"role":"user","content":"Hello"}],"max_tokens":64,"stream":true,"stream_options":{"include_usage":true}})
        }
    }
}

struct Fixture {
    case: String,
    base_url: String,
    client: reqwest::Client,
    calls: Arc<AtomicUsize>,
    entered: Arc<tokio::sync::Notify>,
    error_ready: Arc<tokio::sync::Notify>,
    deployment: Arc<Deployment>,
    estimate: u64,
    connection: Option<redis::aio::MultiplexedConnection>,
    handles: Vec<actix_web::dev::ServerHandle>,
    tasks: Vec<tokio::task::JoinHandle<std::io::Result<()>>>,
}

impl Fixture {
    async fn new(route: &str, redis_url: Option<&str>, ending: Ending, usage: Option<u64>) -> Self {
        let reported = usage.map(
            |tokens| json!({"prompt_tokens":tokens,"completion_tokens":0,"total_tokens":tokens}),
        );
        Self::with_usage(route, redis_url, ending, reported).await
    }

    async fn with_usage(
        route: &str,
        redis_url: Option<&str>,
        ending: Ending,
        usage: Option<Value>,
    ) -> Self {
        let normalized: ChatCompletionRequest = if route == "/v1/responses" {
            let response_request = serde_json::from_value(request_body(route)).unwrap();
            crate::server::routes::ai::responses::build_chat_request(&response_request).unwrap()
        } else {
            serde_json::from_value(request_body("/v1/chat/completions")).unwrap()
        };
        let estimate =
            crate::server::routes::ai::execution::estimate::chat(&normalized, 0).unwrap();
        // The legacy completions adapter creates the same single user message
        // and max_tokens=64. Redis's live charge is checked against this E below.
        assert!(estimate > 3);
        let calls = Arc::new(AtomicUsize::new(0));
        let entered = Arc::new(tokio::sync::Notify::new());
        let error_ready = Arc::new(tokio::sync::Notify::new());
        let upstream_state = Upstream {
            calls: calls.clone(),
            entered: entered.clone(),
            error_ready: error_ready.clone(),
            ending,
            usage: usage.clone(),
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
        config.gateway.guardrails.stream_output_check_chars = 1;
        config.gateway.providers = vec![ProviderConfig {
            name: format!("stream-usage-{}", uuid::Uuid::new_v4()),
            provider_type: "openai_compatible".into(),
            api_key: "sk-stream-usage-fixture".into(),
            base_url: Some(format!("http://{upstream_address}/v1")),
            endpoint_access: ProviderEndpointAccess::PrivateNetwork,
            models: vec![MODEL.into()],
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
            !deployment
                .provider
                .capabilities()
                .contains(&ProviderCapability::Responses),
            "The native Responses selector must choose compatibility streaming"
        );
        assert!(
            !deployment
                .provider
                .supports_capability_for_model(MODEL, &ProviderCapability::Responses),
            "Responses must use the compatibility stream route"
        );
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
            case: format!(
                "{route}/{ending:?}/{usage:?}/shared={}",
                redis_url.is_some()
            ),
            base_url: format!("http://{address}"),
            client,
            calls,
            entered,
            error_ready,
            deployment,
            estimate,
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
        let case = &self.case;
        let state = &self.deployment.state;
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if (state.active_requests.load(Relaxed), state.rpm_current.load(Relaxed), state.tpm_current.load(Relaxed), state.success_requests.load(Relaxed), state.fail_requests.load(Relaxed), state.total_requests.load(Relaxed)) == (0, requests, tokens, successes, failures, successes + failures) { break; }
                tokio::task::yield_now().await;
            }
        }).await.unwrap_or_else(|_| panic!("{case}: local stream accounting did not finish: active={} rpm={} tpm={} success={} fail={} total={}", state.active_requests.load(Relaxed), state.rpm_current.load(Relaxed), state.tpm_current.load(Relaxed), state.success_requests.load(Relaxed), state.fail_requests.load(Relaxed), state.total_requests.load(Relaxed)));
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
                .query_async(connection)
                .await
                .unwrap();
            assert!(!fields.iter().any(|field| field.starts_with("l:")));
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
