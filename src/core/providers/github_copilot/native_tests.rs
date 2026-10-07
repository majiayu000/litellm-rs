use super::*;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, web};
use serde_json::{Value, json};
use std::sync::Mutex;

type Seen = Arc<Mutex<Vec<(String, Value, String, String, String)>>>;

#[derive(Clone)]
struct Upstream {
    seen: Seen,
    endpoints: Value,
    status: u16,
}

async fn handle(req: HttpRequest, bytes: web::Bytes, state: web::Data<Upstream>) -> HttpResponse {
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let header = |name| {
        req.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string()
    };
    state.seen.lock().unwrap().push((
        req.path().into(),
        body.clone(),
        header("authorization"),
        header("x-initiator"),
        header("copilot-vision-request"),
    ));
    if req.path() == "/models" {
        if state.status != 200 {
            return HttpResponse::build(
                actix_web::http::StatusCode::from_u16(state.status).unwrap(),
            )
            .insert_header(("retry-after", "7"))
            .json(json!({"error":{"message":"unavailable"}}));
        }
        return HttpResponse::Ok()
            .json(json!({"data":[{"id":"account-model", "supported_endpoints":state.endpoints}]}));
    }
    if let Some(status) = body.get("test_status").and_then(Value::as_u64) {
        return HttpResponse::build(actix_web::http::StatusCode::from_u16(status as u16).unwrap())
            .insert_header(("retry-after", "7"))
            .json(json!({"error":{"message":"native failure"}}));
    }
    if body.get("stream") != Some(&Value::Bool(true)) {
        return HttpResponse::Ok().json(json!({"id":"copilot-native", "output":[], "usage":{"input_tokens":3,"output_tokens":2},"extension":true}));
    }
    HttpResponse::Ok().insert_header(("content-type", "text/event-stream"))
        .body("event: response.completed\ndata: {\"type\":\"response.completed\",\"opaque\":true}\n\n")
}

async fn fixture(
    endpoints: Value,
    status: u16,
) -> (
    GitHubCopilotProvider,
    Upstream,
    actix_web::dev::ServerHandle,
) {
    let state = Upstream {
        seen: Arc::default(),
        endpoints,
        status,
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let data = state.clone();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
            .default_service(web::to(handle))
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    tokio::spawn(server);
    let mut provider = GitHubCopilotProvider::new(GitHubCopilotConfig {
        api_base: Some(format!("http://{address}")),
        ..Default::default()
    })
    .await
    .unwrap();
    provider.native_endpoint_access = crate::core::net::ProviderEndpointAccess::PrivateNetwork;
    *provider.cached_api_key.write().await = Some("fake-local-copilot-token".into());
    (provider, state, handle)
}

#[tokio::test]
async fn native_responses_use_account_endpoint_evidence_and_preserve_body_headers_sse() {
    let (provider, state, server) = fixture(json!(["/responses"]), 200).await;
    let body = json!({"model":"account-model","stream":true,"input":[{"role":"user","content":[{"type":"input_image","image_url":"data:image/png;base64,AA=="}]},{"type":"function_call_output","call_id":"call_1","output":"opaque"}],"tools":[{"type":"function","name":"lookup","parameters":{}}],"reasoning":{"effort":"low"},"future_field":true});
    let response = provider
        .native_response(body.clone(), &mut false)
        .await
        .unwrap();
    assert!(response.text().await.unwrap().contains("\"opaque\":true"));
    let seen = state.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].0, "/models");
    assert_eq!(seen[1].0, "/responses");
    assert_eq!(seen[1].1, body);
    assert_eq!(seen[1].2, "Bearer fake-local-copilot-token");
    assert_eq!(seen[1].3, "agent");
    assert_eq!(seen[1].4, "true");
    server.stop(false).await;
}

#[tokio::test]
async fn response_only_models_never_reach_chat_completions() {
    let (provider, state, server) = fixture(json!(["/responses"]), 200).await;
    let request = ChatRequest::new("account-model");
    assert!(
        provider
            .chat_completion(request.clone(), RequestContext::new())
            .await
            .is_err()
    );
    assert!(
        provider
            .chat_completion_stream(request, RequestContext::new())
            .await
            .is_err()
    );
    assert!(
        state
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|entry| entry.0 == "/models")
    );
    server.stop(false).await;
}

#[tokio::test]
async fn missing_endpoint_evidence_and_unknown_models_fail_before_inference() {
    for endpoints in [Value::Null, json!([]), json!(["/chat/completions"])] {
        let (provider, state, server) = fixture(endpoints, 200).await;
        assert!(matches!(
            provider
                .native_response(json!({"model":"account-model","input":"Hi"}), &mut false)
                .await,
            Err(ProviderError::NotSupported { .. })
        ));
        assert!(matches!(
            provider
                .native_response(json!({"model":"gpt-made-up","input":"Hi"}), &mut false)
                .await,
            Err(ProviderError::ModelNotFound { .. })
        ));
        assert!(
            state
                .seen
                .lock()
                .unwrap()
                .iter()
                .all(|entry| entry.0 == "/models")
        );
        server.stop(false).await;
    }
}

#[tokio::test]
async fn discovery_errors_preserve_classification_and_never_fall_back() {
    for status in [401, 429, 503] {
        let (provider, state, server) = fixture(json!(["/responses"]), status).await;
        let error = provider
            .native_response(json!({"model":"account-model","input":"Hi"}), &mut false)
            .await
            .unwrap_err();
        match status {
            401 => assert!(matches!(error, ProviderError::Authentication { .. })),
            429 => assert!(matches!(error, ProviderError::RateLimit { .. })),
            _ => assert!(matches!(error, ProviderError::ProviderUnavailable { .. })),
        }
        assert_eq!(state.seen.lock().unwrap().len(), 1);
        server.stop(false).await;
    }
}

#[tokio::test]
async fn native_json_and_upstream_errors_are_preserved() {
    let (provider, state, server) = fixture(json!(["/responses"]), 200).await;
    let response = provider
        .native_response(json!({"model":"account-model","input":"Hi"}), &mut false)
        .await
        .unwrap();
    let value: Value = response.json().await.unwrap();
    assert_eq!(value["usage"]["input_tokens"], 3);
    assert_eq!(value["extension"], true);
    for status in [429, 503, 401] {
        let error = provider
            .native_response(
                json!({"model":"account-model","input":"Hi","test_status":status}),
                &mut false,
            )
            .await
            .unwrap_err();
        match status {
            401 => assert!(matches!(error, ProviderError::Authentication { .. })),
            429 => assert!(matches!(error, ProviderError::RateLimit { .. })),
            _ => assert!(matches!(error, ProviderError::ProviderUnavailable { .. })),
        }
    }
    assert_eq!(state.seen.lock().unwrap().len(), 8);
    server.stop(false).await;
}

// Exercise the real route, reservation owners and ledger against a local transport
// fault, rather than inferring settlement from a provider error variant alone.
#[cfg(feature = "sqlite")]
#[tokio::test]
async fn discovery_faults_refund_but_generation_faults_settle_unknown_without_replay() {
    use crate::core::budget::{
        BudgetConfig, BudgetScope, ModelLimitConfig, ProviderLimitConfig, ResetPeriod,
    };
    use crate::core::keys::CreateKeyConfig;
    use crate::core::providers::Provider;
    use crate::core::request_ledger::{RequestLedgerFacts, scope_facts, snapshot_facts};
    use crate::core::router::Deployment;
    use actix_web::{HttpMessage, test};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for generation_fault in [false, true] {
        for timeout in [false, true] {
            for streaming in [false, true] {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let base = format!("http://{}", listener.local_addr().unwrap());
                let gets = Arc::new(AtomicUsize::new(0));
                let posts = Arc::new(AtomicUsize::new(0));
                let counts = (gets.clone(), posts.clone());
                let upstream = tokio::spawn(async move {
                    let mut connections = tokio::task::JoinSet::new();
                    loop {
                        tokio::select! {
                            accepted = listener.accept() => {
                                let (mut stream, _) = accepted.unwrap();
                                let (gets, posts) = counts.clone();
                                connections.spawn(async move {
                                    let mut bytes = Vec::new();
                                    let (end, length) = loop {
                                        let mut chunk = [0; 4096];
                                        let n = stream.read(&mut chunk).await.unwrap();
                                        assert_ne!(n, 0);
                                        bytes.extend_from_slice(&chunk[..n]);
                                        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                                            let headers = std::str::from_utf8(&bytes[..end]).unwrap();
                                            let length = headers.lines().find_map(|line| {
                                                let (name, value) = line.split_once(':')?;
                                                name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().unwrap())
                                            }).unwrap_or(0);
                                            break (end + 4, length);
                                        }
                                    };
                                    while bytes.len() < end + length {
                                        let mut chunk = [0; 4096];
                                        let n = stream.read(&mut chunk).await.unwrap();
                                        assert_ne!(n, 0);
                                        bytes.extend_from_slice(&chunk[..n]);
                                    }
                                    if bytes.starts_with(b"GET /models ") {
                                        gets.fetch_add(1, Ordering::SeqCst);
                                        if generation_fault {
                                            let body = r#"{"data":[{"id":"gpt-4o","supported_endpoints":["/responses"]}]}"#;
                                            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                                            stream.write_all(response.as_bytes()).await.unwrap();
                                            return;
                                        }
                                    } else {
                                        assert!(bytes.starts_with(b"POST /responses "));
                                        posts.fetch_add(1, Ordering::SeqCst);
                                    }
                                    if timeout { tokio::time::sleep(Duration::from_secs(60)).await; }
                                    // Drop the connection after consuming the complete request.
                                });
                            }
                            completed = connections.join_next(), if !connections.is_empty() => {
                                completed.unwrap().unwrap();
                            }
                        }
                    }
                });
                let tokens = tempfile::tempdir().unwrap();
                std::fs::write(
                    tokens.path().join("api-key.json"),
                    json!({
                        "token":"fake-local-copilot-token", "expires_at":4102444800_u64
                    })
                    .to_string(),
                )
                .unwrap();
                let mut provider = GitHubCopilotProvider::new(GitHubCopilotConfig {
                    api_base: Some(base),
                    token_dir: Some(tokens.path().display().to_string()),
                    timeout: 1,
                    ..Default::default()
                })
                .await
                .unwrap();
                provider.native_endpoint_access =
                    crate::core::net::ProviderEndpointAccess::PrivateNetwork;
                let mut config = crate::server::valid_test_config();
                config.gateway.storage.database.enabled = false;
                config.gateway.storage.redis.enabled = false;
                config.gateway.auth.enable_api_key = false;
                config.gateway.auth.enable_jwt = false;
                config.gateway.auth.allow_anonymous = true;
                let gateway = crate::server::HttpServer::new(&config).await.unwrap();
                let state = gateway.state();
                state.unified_router().add_deployment(Deployment::new(
                    "copilot-fault".into(),
                    Provider::GitHubCopilot(provider),
                    "gpt-4o".into(),
                    "fault-model".into(),
                ));
                state.pricing.add_custom_model(
                    "gpt-4o".into(),
                    serde_json::from_value(json!({
                        "litellm_provider":"github_copilot", "mode":"chat", "max_output_tokens":1,
                        "input_cost_per_token":0.0, "output_cost_per_token":1.0
                    }))
                    .unwrap(),
                );
                state.budget_limits.providers.set_provider_limit(
                    "github_copilot",
                    ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
                );
                state
                    .budget_limits
                    .models
                    .set_model_limit("gpt-4o", ModelLimitConfig::new(10.0, ResetPeriod::Monthly));
                let scope = BudgetScope::ApiKey("copilot-fault".into());
                let budget = state
                    .budget_manager
                    .create_budget(scope.clone(), BudgetConfig::new("copilot-fault", 10.0))
                    .await
                    .unwrap();
                let (key, _) = state
                    .key_manager
                    .generate_key(CreateKeyConfig {
                        name: "copilot-fault".into(),
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(state.clone()))
                        .configure(crate::server::routes::ai::configure_routes),
                )
                .await;
                let req = test::TestRequest::post().uri("/v1/responses").set_json(json!({
                    "model":"fault-model", "input":"Hi", "store":false, "stream":streaming, "max_output_tokens":1
                })).to_request();
                req.extensions_mut().insert(
                    RequestContext::new()
                        .with_api_key(key)
                        .with_api_key_budget(budget.id.parse().unwrap()),
                );
                let facts = Arc::new(Mutex::new(RequestLedgerFacts::default()));
                let response = scope_facts(facts.clone(), test::call_service(&app, req)).await;
                let status = response.status();
                let error = test::read_body(response).await;
                assert_eq!(
                    status.as_u16(),
                    if timeout { 504 } else { 502 },
                    "{}",
                    String::from_utf8_lossy(&error)
                );
                assert_eq!(gets.load(Ordering::SeqCst), 1, "discovery must not replay");
                assert_eq!(
                    posts.load(Ordering::SeqCst),
                    usize::from(generation_fault),
                    "generation must not replay"
                );
                let expected = if generation_fault { 1.0 } else { 0.0 };
                assert_eq!(
                    state
                        .budget_limits
                        .providers
                        .get_provider_usage("github_copilot")
                        .unwrap()
                        .current_spend,
                    expected
                );
                assert_eq!(
                    state
                        .budget_limits
                        .models
                        .get_model_usage("gpt-4o")
                        .unwrap()
                        .current_spend,
                    expected
                );
                assert_eq!(state.budget_manager.get_current_spend(&scope), expected);
                let usage = state.key_manager.get_usage_stats(key).await.unwrap();
                assert_eq!(usage.total_requests, u64::from(generation_fault));
                assert_eq!(usage.unpriced_requests, u64::from(generation_fault));
                assert_eq!(usage.total_cost, 0.0);
                let ledger = snapshot_facts(&facts);
                // The reserved upper bound is never reported as an actual bill.
                assert_eq!(ledger.cost, None);
                assert_eq!(ledger.total_tokens, None);
                // Reservation identity is known before discovery or generation.
                assert_eq!(ledger.provider.as_deref(), Some("github_copilot"));
                assert_eq!(ledger.model.as_deref(), Some("gpt-4o"));
                upstream.abort();
                let _ = upstream.await;
            }
        }
    }
}
