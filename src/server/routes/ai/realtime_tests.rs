use super::*;
use crate::core::{
    budget::{ProviderLimitConfig, ResetPeriod},
    net::ProviderEndpointAccess,
};
use actix_web::{App, HttpServer, test};
use std::sync::{Arc, Mutex};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

type Calls = Arc<Mutex<Vec<Value>>>;
fn usage() -> Value {
    json!({"total_tokens":60,"input_tokens":40,"output_tokens":20,"input_token_details":{"text_tokens":20,"audio_tokens":20,"cached_tokens":15,"cached_tokens_details":{"text_tokens":10,"audio_tokens":5},"image_tokens":0},"output_token_details":{"text_tokens":5,"audio_tokens":15}})
}
fn rates() -> Rates {
    let pricing = crate::core::pricing_service::PricingService::with_embedded_default().unwrap();
    let (_, info) = pricing
        .snapshot()
        .get_model_info_for_provider("openai", "gpt-realtime-mini")
        .unwrap();
    Rates::load(&info, None).unwrap()
}
#[actix_web::test]
async fn modality_cache_cost_and_conservative_bound() {
    let rates = rates();
    let (cost, tokens) = rates.cost(&usage()).unwrap();
    let expected = 10.0 * 0.0000006
        + 15.0 * 0.00001
        + 10.0 * 0.00000006
        + 5.0 * 0.0000003
        + 5.0 * 0.0000024
        + 15.0 * 0.00002;
    assert!((cost - expected).abs() < 1e-12);
    assert_eq!(tokens, 60);
    assert!((rates.bound(rates.max_output) - 0.40192).abs() < 1e-12);
    let mut invalid = usage();
    invalid["input_token_details"]["cached_tokens_details"]["audio_tokens"] = json!(500);
    assert!(rates.cost(&invalid).is_err());
    assert!(rates.cost(&json!({})).is_err());
}
#[actix_web::test]
async fn explicit_manual_scope_and_output_policy() {
    let rates = rates();
    for mut event in [
        json!({"type":"session.update","session":{"audio":{"input":{"turn_detection":{"type":"server_vad"}}}}}),
        json!({"type":"session.update","session":{"audio":{"input":{"transcription":{"model":"whisper-1"}}}}}),
        json!({"type":"response.create","response":{"tools":[{"type":"mcp"}]}}),
        json!({"type":"session.update","session":{"model":"another"}}),
        json!({"type":"response.create","response":{"max_output_tokens":99999}}),
        json!({"type":"conversation.item.create","item":{"content":[{"type":"input_image"}]}}),
    ] {
        assert!(prepare_event(&mut event, &rates, false, "pinned", "wire").is_err());
    }
    let mut event = json!({"type":"response.create","response":{"tools":[{"type":"function","name":"weather"}]}});
    assert!(prepare_event(&mut event, &rates, false, "pinned", "wire").unwrap());
    assert!(event["response"].get("max_output_tokens").is_none());
    assert!(prepare_event(&mut event, &rates, true, "pinned", "wire").is_err());
}
async fn upstream(
    req: HttpRequest,
    payload: web::Payload,
    calls: web::Data<Calls>,
) -> actix_web::Result<HttpResponse> {
    assert_eq!(
        req.headers().get("authorization").unwrap(),
        "Bearer sk-upstream-realtime-test"
    );
    assert!(!req.headers().contains_key("x-api-key"));
    assert_eq!(req.query_string(), "model=gpt-realtime-mini-mapped");
    let (response, mut session, mut stream) = actix_ws::handle(&req, payload)?;
    let calls = calls.get_ref().clone();
    actix_web::rt::spawn(async move {
        session
            .text(json!({"type":"session.created","session":{"id":"session-1"}}).to_string())
            .await
            .unwrap();
        while let Some(Ok(event)) = stream.next().await {
            match event {
                actix_ws::Message::Text(text) => {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    calls.lock().unwrap().push(value.clone());
                    match value["type"].as_str().unwrap() {
                        "session.update" => {
                            if value["session"]["audio"]["output"]["voice"] == "reject-voice" {
                                let _ = session.text(json!({"type":"error","error":{"type":"invalid_request_error","event_id":value["event_id"],"message":"mock invalid voice"}}).to_string()).await;
                                continue;
                            }
                            if session
                                .text(
                                    json!({"type":"session.updated","session":value["session"]})
                                        .to_string(),
                                )
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        "response.create" => {
                            if value["response"]["metadata"]["reject"] == true {
                                let _ = session.text(json!({"type":"error","error":{"type":value["response"]["metadata"].get("reject_type").and_then(Value::as_str).unwrap_or("invalid_request_error"),"event_id":value["event_id"],"message":"mock rejection"}}).to_string()).await;
                                continue;
                            }
                            if value["response"]["metadata"]["unrelated_error"] == true {
                                let _ = session.text(json!({"type":"error","error":{"type":"invalid_request_error","event_id":"unrelated-item","message":"mock unrelated event"}}).to_string()).await;
                            }
                            if value["response"]["metadata"]["normal_close"] == true {
                                let _ = session.text(json!({"type":"response.created","response":{"id":"response-1"}}).to_string()).await;
                                let _ = session
                                    .close(Some(actix_ws::CloseCode::Normal.into()))
                                    .await;
                                return;
                            }
                            if value["response"]["metadata"]["hold"] == true {
                                if session.text(json!({"type":"response.created","response":{"id":"response-1"}}).to_string()).await.is_err() { break; }
                                continue;
                            }
                            for event in [
                                json!({"type":"response.created","response":{"id":"response-1"}}),
                                json!({"type":"response.output_audio.delta","delta":"aGVsbG8="}),
                                json!({"type":"response.function_call_arguments.done","arguments":"{}","call_id":"call-1"}),
                                json!({"type":"response.done","response":{"id":"response-1","status":value["response"]["metadata"]["status"].as_str().unwrap_or("completed"),"status_details":{"error":{"type":"server_error","message":"mock failure"}},"usage":usage()}}),
                            ] {
                                if session.text(event.to_string()).await.is_err() {
                                    break;
                                }
                            }
                        }
                        "input_audio_buffer.clear" => {
                            let _ = session.text(json!({"type":"response.done","response":{"id":"response-1","status":"completed","usage":usage()}}).to_string()).await;
                        }
                        "conversation.item.delete" => {
                            let _ = session
                                .close(Some(actix_ws::CloseReason {
                                    code: actix_ws::CloseCode::Policy,
                                    description: Some("mock-policy".into()),
                                }))
                                .await;
                            return;
                        }
                        _ => {
                            if session.text(text).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                actix_ws::Message::Close(reason) => {
                    let _ = session.close(reason).await;
                    break;
                }
                actix_ws::Message::Ping(bytes) if session.pong(&bytes).await.is_err() => {
                    break;
                }
                _ => {}
            }
        }
        calls.lock().unwrap().push(json!({"closed":true}));
    });
    Ok(response)
}
async fn fixture() -> (
    AppState,
    String,
    String,
    Calls,
    Vec<actix_web::dev::ServerHandle>,
) {
    fixture_with_config(|_| {}).await
}
async fn fixture_with_config(
    configure: impl FnOnce(&mut crate::Config),
) -> (
    AppState,
    String,
    String,
    Calls,
    Vec<actix_web::dev::ServerHandle>,
) {
    let calls: Calls = Arc::default();
    let copy = calls.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(copy.clone()))
            .route("/v1/realtime", web::get().to(upstream))
            .route(
                "/broken/v1/realtime",
                web::get().to(rejected_initialization),
            )
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let upstream_handle = server.handle();
    actix_web::rt::spawn(server);
    let mut config = crate::server::valid_test_config();
    config.gateway.auth.enable_jwt = false;
    config.gateway.guardrails.enabled = false;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = Some("config/model_prices_extended.json".into());
    let provider = &mut config.gateway.providers[0];
    provider.name = "openai".into();
    provider.api_key = "sk-upstream-realtime-test".into();
    provider.base_url = Some(base);
    provider.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    provider.models = vec!["gpt-realtime-mini".into()];
    provider.settings.insert(
        "model_mappings".into(),
        json!({"gpt-realtime-mini":"gpt-realtime-mini-mapped"}),
    );
    provider.settings.insert("model_identity_mappings".into(), json!({"gpt-realtime-mini":{"capability_catalog_model":"gpt-realtime-mini","pricing_model":"gpt-realtime-mini"}}));
    configure(&mut config);
    let gateway = crate::server::http::HttpServer::new(&config).await.unwrap();
    let state = gateway.state().clone();
    let mut owner = crate::core::models::user::types::User::new(
        "realtime-test".into(),
        "realtime@example.test".into(),
        "unused".into(),
    );
    owner.status = crate::core::models::user::types::UserStatus::Active;
    let owner = state.storage.db().create_user(&owner).await.unwrap();
    let (_, raw) = state
        .auth
        .create_api_key(
            owner.id(),
            "realtime-test".into(),
            vec!["api.realtime".into()],
        )
        .await
        .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "ws://{}/v1/realtime?model=gpt-realtime-mini",
        listener.local_addr().unwrap()
    );
    let server_state = state.clone();
    let server = HttpServer::new(move || {
        crate::server::http::HttpServer::create_app(web::Data::new(server_state.clone()))
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    actix_web::rt::spawn(server);
    (state, url, raw, calls, vec![handle, upstream_handle])
}
async fn client(url: &str, key: &str) -> WebSocketStream<tokio::net::TcpStream> {
    let mut request = url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("x-api-key", key.parse().unwrap());
    let url = url::Url::parse(url).unwrap();
    let tcp = tokio::net::TcpStream::connect((url.host_str().unwrap(), url.port().unwrap()))
        .await
        .unwrap();
    tokio_tungstenite::client_async(request, tcp)
        .await
        .unwrap()
        .0
}
async fn next_json(client: &mut WebSocketStream<tokio::net::TcpStream>) -> Value {
    let event = tokio::time::timeout(Duration::from_secs(3), client.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    serde_json::from_str(event.to_text().unwrap()).unwrap()
}
#[actix_web::test]
async fn gateway_websocket_preserves_events_and_records_modality_cost() {
    let (state, url, key, calls, handles) = fixture().await;
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    let mut client = client(&url, &key).await;
    assert_eq!(next_json(&mut client).await["type"], "session.created");
    assert_eq!(next_json(&mut client).await["type"], "session.updated");
    let audio = json!({"type":"input_audio_buffer.append","audio":"aGVsbG8="});
    client
        .send(Message::Text(audio.to_string().into()))
        .await
        .unwrap();
    assert_eq!(next_json(&mut client).await, audio);
    client
        .send(Message::Text(
            json!({"type":"response.create"}).to_string().into(),
        ))
        .await
        .unwrap();
    for event in [
        "response.created",
        "response.output_audio.delta",
        "response.function_call_arguments.done",
        "response.done",
    ] {
        assert_eq!(next_json(&mut client).await["type"], event);
    }
    let cost = rates().cost(&usage()).unwrap().0;
    assert!(
        (state
            .budget_limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend
            - cost)
            .abs()
            < 1e-10
    );
    let (stored, _) = state
        .auth
        .api_key()
        .verify_key(&key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.usage_stats.total_tokens, 60);
    assert!((stored.usage_stats.total_cost - cost).abs() < 1e-9);
    assert_eq!(
        calls.lock().unwrap()[0]["session"]["audio"]["input"]["turn_detection"],
        Value::Null
    );
    let close = tokio_tungstenite::tungstenite::protocol::CloseFrame {
        code: 1000.into(),
        reason: "client-end".into(),
    };
    client.close(Some(close.clone())).await.unwrap();
    assert_eq!(
        client.next().await.unwrap().unwrap(),
        Message::Close(Some(close))
    );
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}
#[actix_web::test]
async fn budget_and_scope_rejections_never_reach_upstream() {
    let (state, url, key, calls, handles) = fixture().await;
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
    );
    let mut client = client(&url, &key).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    client
        .send(Message::Text(
            json!({"type":"response.create"}).to_string().into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        next_json(&mut client).await["error"]["type"],
        "insufficient_quota"
    );
    client.send(Message::Text(json!({"type":"session.update","session":{"audio":{"input":{"turn_detection":{"type":"server_vad"}}}}}).to_string().into())).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["error"]["type"],
        "invalid_request_error"
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    client.close(None).await.unwrap();
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}
#[actix_web::test]
async fn upstream_close_code_and_reason_are_preserved() {
    let (state, url, key, _, handles) = fixture().await;
    let mut client = client(&url, &key).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    client
        .send(Message::Text(
            json!({"type":"conversation.item.delete","item_id":"close"})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let event = client.next().await.unwrap().unwrap();
    let Message::Close(Some(close)) = event else {
        panic!("expected close")
    };
    assert_eq!(u16::from(close.code), 1008);
    assert_eq!(close.reason, "mock-policy");
    let router = state.pin_runtime().unified_router.clone();
    let id = &router.get_deployments_for_model("gpt-realtime-mini")[0];
    let deployment = router.get_deployment(id).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while deployment
            .state
            .fail_requests
            .load(std::sync::atomic::Ordering::Relaxed)
            == 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        deployment
            .state
            .success_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}
#[actix_web::test]
async fn authentication_and_model_policy_reject_before_upgrade() {
    let (state, _, _, calls, handles) = fixture().await;
    let app = test::init_service(crate::server::http::HttpServer::create_app(web::Data::new(
        state,
    )))
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/realtime?model=gpt-realtime-mini")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    assert!(calls.lock().unwrap().is_empty());
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn key_model_and_endpoint_permissions_are_enforced() {
    let (state, _, raw, calls, handles) = fixture().await;
    let (mut key, _) = state
        .auth
        .api_key()
        .verify_key(&raw)
        .await
        .unwrap()
        .unwrap();
    let app = test::init_service(crate::server::http::HttpServer::create_app(web::Data::new(
        state.clone(),
    )))
    .await;
    for policy in [
        json!({"allowed_models":["gpt-4o"]}),
        json!({"allowed_endpoints":["/v1/chat/completions"]}),
    ] {
        key.metadata
            .set_extra("__core_keys", json!({"permissions":policy}));
        state.storage.db().update_api_key(&key).await.unwrap();
        let req = test::TestRequest::get()
            .uri("/v1/realtime?model=gpt-realtime-mini")
            .insert_header(("x-api-key", raw.clone()))
            .to_request();
        assert_eq!(
            test::call_service(&app, req).await.status(),
            actix_web::http::StatusCode::FORBIDDEN
        );
    }
    assert!(calls.lock().unwrap().is_empty());
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn key_budget_and_interruption_fallback_use_existing_reservations() {
    use crate::core::budget::{BudgetConfig, BudgetScope};
    let (state, _, _, _, handles) = fixture().await;
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    let scope = BudgetScope::ApiKey("realtime-budget-test".into());
    let budget = state
        .budget_manager
        .create_budget(scope.clone(), BudgetConfig::new("realtime", 0.5))
        .await
        .unwrap();
    let id = Some(budget.id.parse().unwrap());
    let bound = rates().bound(rates().max_output);
    let pending = Pending::reserve(&state, "openai", "gpt-realtime-mini", id, bound).unwrap();
    assert!(Pending::reserve(&state, "openai", "gpt-realtime-mini", id, bound).is_err());
    pending
        .settle(&state, None, Some(rates().cost(&usage()).unwrap()))
        .await
        .unwrap();
    let expected = rates().cost(&usage()).unwrap().0;
    assert!((state.budget_manager.get_current_spend(&scope) - expected).abs() < 1e-9);
    let pending = Pending::reserve(&state, "openai", "gpt-realtime-mini", id, bound).unwrap();
    drop(pending);
    assert!((state.budget_manager.get_current_spend(&scope) - expected - bound).abs() < 1e-9);
    assert!(
        (state
            .budget_limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend
            - expected
            - bound)
            .abs()
            < 1e-9
    );
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn upstream_handshake_status_and_error_detail_are_preserved() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = HttpServer::new(|| App::new().default_service(web::to(|| async {
        HttpResponse::Unauthorized().json(json!({"error":{"type":"authentication_error","code":"invalid_api_key","message":"mock upstream denied"}}))
    }))).workers(1).listen(listener).unwrap().run();
    let handle = server.handle();
    actix_web::rt::spawn(server);
    let mut config = crate::core::providers::openai::OpenAIConfig::default();
    config.base.api_key = Some("sk-test".into());
    config.base.api_base = Some(base);
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = crate::core::providers::openai::OpenAIProvider::new(config)
        .await
        .unwrap();
    let error = open_upstream(&provider, "gpt-realtime-mini", 4096)
        .await
        .unwrap_err();
    let response =
        super::super::openai_errors::gateway_error_response(&GatewayError::Provider(error));
    assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    let body = actix_web::body::to_bytes(response.into_body())
        .await
        .unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["error"]["code"], "invalid_api_key");
    assert_eq!(value["error"]["message"], "mock upstream denied");
    handle.stop(false).await;
}

#[actix_web::test]
async fn client_disconnect_closes_upstream_and_records_reserved_fallback() {
    let (state, url, key, calls, handles) =
        fixture_with_config(|config| config.gateway.providers[0].rpm = 1).await;
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    let mut client = client(&url, &key).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    client
        .send(Message::Text(
            json!({"type":"response.create","response":{"metadata":{"hold":true}}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut client).await["type"], "response.created");
    drop(client);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let (stored, _) = state
                .auth
                .api_key()
                .verify_key(&key)
                .await
                .unwrap()
                .unwrap();
            if (stored.usage_stats.total_cost - rates().bound(rates().max_output)).abs() < 1e-9
                && calls
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|event| event["closed"] == true)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("disconnect must settle and close the upstream");
    let router = state.pin_runtime().unified_router.clone();
    let deployment = router
        .get_deployment(&router.get_deployments_for_model("gpt-realtime-mini")[0])
        .unwrap();
    use std::sync::atomic::Ordering;
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
    assert_eq!(
        deployment.state.tpm_current.load(Ordering::Relaxed),
        rates().max_input as u64 + rates().max_output as u64
    );
    assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    let mut request = url.clone().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("x-api-key", key.parse().unwrap());
    let parsed = url::Url::parse(&url).unwrap();
    let tcp = tokio::net::TcpStream::connect((parsed.host_str().unwrap(), parsed.port().unwrap()))
        .await
        .unwrap();
    let error = tokio_tungstenite::client_async(request, tcp)
        .await
        .unwrap_err();
    let tokio_tungstenite::tungstenite::Error::Http(response) = error else {
        panic!("expected RPM rejection")
    };
    // Exhausted routing availability retains the gateway handshake error contract.
    assert_eq!(response.status().as_u16(), 503);
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn realtime_2_has_complete_authoritative_audio_cache_rates() {
    let pricing = crate::core::pricing_service::PricingService::with_embedded_default().unwrap();
    let (_, info) = pricing
        .snapshot()
        .get_model_info_for_provider("openai", "gpt-realtime-2")
        .unwrap();
    let rates = Rates::load(&info, None).unwrap();
    assert_eq!(
        info.extra["cache_read_input_audio_token_cost"].as_f64(),
        Some(0.0000004)
    );
    assert!(rates.cost(&usage()).unwrap().0 < rates.bound(rates.max_output));
}

#[actix_web::test]
async fn completed_tokens_survive_a_later_protocol_failure() {
    let (state, url, key, _, handles) = fixture().await;
    let mut client = client(&url, &key).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    client
        .send(Message::Text(
            json!({"type":"response.create"}).to_string().into(),
        ))
        .await
        .unwrap();
    for _ in 0..4 {
        next_json(&mut client).await;
    }
    client.send(Message::Text("not-json".into())).await.unwrap();
    assert_eq!(next_json(&mut client).await["type"], "error");
    let router = state.pin_runtime().unified_router.clone();
    let id = &router.get_deployments_for_model("gpt-realtime-mini")[0];
    let deployment = router.get_deployment(id).unwrap();
    assert_eq!(
        deployment
            .state
            .fail_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        deployment
            .state
            .tpm_current
            .load(std::sync::atomic::Ordering::Relaxed),
        60
    );
    assert_eq!(
        deployment
            .state
            .success_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn response_creation_consumes_the_existing_key_rpm_allowance() {
    let (state, url, raw, calls, handles) = fixture().await;
    let (mut key, _) = state
        .auth
        .api_key()
        .verify_key(&raw)
        .await
        .unwrap()
        .unwrap();
    key.rate_limits = Some(crate::core::models::RateLimits {
        rpm: Some(2),
        tpm: None,
        rpd: None,
        tpd: None,
        concurrent: None,
    });
    state.storage.db().update_api_key(&key).await.unwrap();
    let mut client = client(&url, &raw).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    client
        .send(Message::Text(
            json!({"type":"response.create"}).to_string().into(),
        ))
        .await
        .unwrap();
    for _ in 0..4 {
        next_json(&mut client).await;
    }
    client
        .send(Message::Text(
            json!({"type":"response.create"}).to_string().into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        next_json(&mut client).await["error"]["type"],
        "rate_limit_error"
    );
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v["type"] == "response.create")
            .count(),
        1
    );
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn only_matching_precreation_errors_consume_the_reservation() {
    let (state, url, key, calls, handles) = fixture().await;
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    let mut client = client(&url, &key).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    client
        .send(Message::Text(
            json!({"type":"response.create","response":{"metadata":{"unrelated_error":true}}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        next_json(&mut client).await["error"]["event_id"],
        "unrelated-item"
    );
    for kind in [
        "response.created",
        "response.output_audio.delta",
        "response.function_call_arguments.done",
        "response.done",
    ] {
        assert_eq!(next_json(&mut client).await["type"], kind);
    }
    assert!(
        (state
            .budget_limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend
            - rates().cost(&usage()).unwrap().0)
            .abs()
            < 1e-10
    );
    client.send(Message::Text(json!({"type":"response.create","event_id":"rejected-response","response":{"metadata":{"reject":true}}}).to_string().into())).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["error"]["event_id"],
        "rejected-response"
    );
    client
        .send(Message::Text(
            json!({"type":"response.create"}).to_string().into(),
        ))
        .await
        .unwrap();
    for _ in 0..4 {
        next_json(&mut client).await;
    }
    {
        let events = calls.lock().unwrap();
        for event in events.iter().filter(|v| v["type"] == "response.create") {
            assert!(event["event_id"].as_str().is_some());
        }
    }
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn usage_persistence_failure_does_not_hide_completed_response() {
    let (state, url, raw, _, handles) = fixture().await;
    let (key, _) = state
        .auth
        .api_key()
        .verify_key(&raw)
        .await
        .unwrap()
        .unwrap();
    let mut client = client(&url, &raw).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    client
        .send(Message::Text(
            json!({"type":"response.create","response":{"metadata":{"hold":true}}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut client).await["type"], "response.created");
    state
        .storage
        .db()
        .delete_api_key(key.metadata.id)
        .await
        .unwrap();
    assert!(
        state
            .budgeted
            .key_manager()
            .record_usage(key.metadata.id, 1, 0.001)
            .await
            .is_err()
    );
    client
        .send(Message::Text(
            json!({"type":"input_audio_buffer.clear"})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut client).await["type"], "response.done");
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}

async fn rejected_initialization(
    req: HttpRequest,
    payload: web::Payload,
    calls: web::Data<Calls>,
) -> actix_web::Result<HttpResponse> {
    let (response, mut session, mut input) = actix_ws::handle(&req, payload)?;
    actix_web::rt::spawn(async move {
        if let Some(Ok(actix_ws::Message::Text(_))) = input.next().await {
            calls
                .lock()
                .unwrap()
                .push(json!({"initialization_rejected":true}));
            let _ = session
                .text(
                    json!({"type":"error","error":{"type":"server_error","message":"bad backend"}})
                        .to_string(),
                )
                .await;
        }
    });
    Ok(response)
}

#[actix_web::test]
async fn session_limits_survive_omitted_fields_and_restricted_key_updates() {
    let (state, url, raw, calls, handles) = fixture().await;
    let (mut key, _) = state
        .auth
        .api_key()
        .verify_key(&raw)
        .await
        .unwrap()
        .unwrap();
    let mut client = client(&url, &raw).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    for event in [
        json!({"type":"session.update","session":{"max_output_tokens":128}}),
        json!({"type":"session.update","session":{"instructions":"brief"}}),
    ] {
        client
            .send(Message::Text(event.to_string().into()))
            .await
            .unwrap();
        assert_eq!(next_json(&mut client).await["type"], "session.updated");
    }
    for cap in [128, 64] {
        if cap == 64 {
            key.metadata.extra.insert("__core_keys".into(), json!({"permissions":{"allowed_models":[],"allowed_endpoints":[],"max_tokens_per_request":64,"is_admin":false,"custom_permissions":["api.realtime"]}}));
            state.storage.db().update_api_key(&key).await.unwrap();
        }
        client
            .send(Message::Text(
                json!({"type":"response.create"}).to_string().into(),
            ))
            .await
            .unwrap();
        for _ in 0..4 {
            next_json(&mut client).await;
        }
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|v| v["type"] == "response.create")
                .unwrap()["response"]["max_output_tokens"],
            cap
        );
    }
    assert!(
        calls
            .lock()
            .unwrap()
            .iter()
            .find(|v| v["session"]["instructions"] == "brief")
            .unwrap()["session"]
            .get("max_output_tokens")
            .is_none()
    );
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn active_normal_close_and_failed_done_affect_provider_health() {
    for metadata in [
        json!({"normal_close":true}),
        json!({"status":"failed"}),
        json!({"status":"cancelled"}),
        json!({"status":"incomplete"}),
    ] {
        let should_fail = metadata["normal_close"] == true || metadata["status"] == "failed";
        let (state, url, key, _, handles) = fixture().await;
        let mut client = client(&url, &key).await;
        next_json(&mut client).await;
        next_json(&mut client).await;
        client
            .send(Message::Text(
                json!({"type":"response.create","response":{"metadata":metadata}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        if metadata["normal_close"] == true {
            assert_eq!(next_json(&mut client).await["type"], "response.created");
            assert!(matches!(
                client.next().await.unwrap().unwrap(),
                Message::Close(_)
            ));
        } else {
            for _ in 0..4 {
                next_json(&mut client).await;
            }
        }
        let router = state.pin_runtime().unified_router.clone();
        let deployment = router
            .get_deployment(&router.get_deployments_for_model("gpt-realtime-mini")[0])
            .unwrap();
        if should_fail {
            tokio::time::timeout(Duration::from_secs(3), async {
                while deployment
                    .state
                    .fail_requests
                    .load(std::sync::atomic::Ordering::Relaxed)
                    == 0
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        } else {
            assert_eq!(
                deployment
                    .state
                    .fail_requests
                    .load(std::sync::atomic::Ordering::Relaxed),
                0
            );
            assert_eq!(
                deployment
                    .state
                    .success_requests
                    .load(std::sync::atomic::Ordering::Relaxed),
                1
            );
        }
        drop(client);
        for handle in handles {
            handle.stop(false).await;
        }
    }
}

#[actix_web::test]
async fn initialization_failure_retries_before_the_client_upgrade() {
    let (_, url, key, calls, handles) = fixture_with_config(|config| {
        config.gateway.router.strategy =
            crate::core::router::config::RoutingStrategy::PriorityBased;
        let mut broken = config.gateway.providers[0].clone();
        broken.name = "broken-openai".into();
        broken.base_url = broken.base_url.map(|url| url.replace("/v1", "/broken/v1"));
        broken.priority = 0;
        config.gateway.providers[0].priority = 1;
        config.gateway.providers.insert(0, broken);
    })
    .await;
    let mut client = client(&url, &key).await;
    assert_eq!(next_json(&mut client).await["type"], "session.created");
    assert_eq!(next_json(&mut client).await["type"], "session.updated");
    assert!(
        calls
            .lock()
            .unwrap()
            .iter()
            .any(|v| v["initialization_rejected"] == true)
    );
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn response_boundaries_recheck_deployment_rpm_tpm_and_parallel_admission() {
    for rpm in [true, false] {
        let (state, url, key, calls, handles) = fixture_with_config(|config| {
            config.gateway.providers[0].max_concurrent_requests = 1;
            if rpm {
                config.gateway.providers[0].rpm = 1;
            } else {
                config.gateway.providers[0].tpm = 60;
            }
        })
        .await;
        let mut client = client(&url, &key).await;
        next_json(&mut client).await;
        next_json(&mut client).await;
        client
            .send(Message::Text(
                json!({"type":"response.create"}).to_string().into(),
            ))
            .await
            .unwrap();
        for _ in 0..4 {
            next_json(&mut client).await;
        }
        let router = state.pin_runtime().unified_router.clone();
        let deployment = router
            .get_deployment(&router.get_deployments_for_model("gpt-realtime-mini")[0])
            .unwrap();
        assert_eq!(
            deployment
                .state
                .tpm_current
                .load(std::sync::atomic::Ordering::Relaxed),
            60
        );
        assert_eq!(
            deployment
                .state
                .rpm_current
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        assert_eq!(
            deployment
                .state
                .active_requests
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        client
            .send(Message::Text(
                json!({"type":"response.create"}).to_string().into(),
            ))
            .await
            .unwrap();
        assert_eq!(
            next_json(&mut client).await["error"]["type"],
            "rate_limit_error"
        );
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|v| v["type"] == "response.create")
                .count(),
            1
        );
        drop(client);
        for handle in handles {
            handle.stop(false).await;
        }
    }
}

#[actix_web::test]
async fn response_boundaries_reauthorize_current_keys() {
    for change in [
        "deleted",
        "revoked",
        "expired",
        "permission",
        "model",
        "endpoint",
        "rpm",
    ] {
        let (state, url, raw, calls, handles) = fixture().await;
        let (mut key, _) = state
            .auth
            .api_key()
            .verify_key(&raw)
            .await
            .unwrap()
            .unwrap();
        let mut client = client(&url, &raw).await;
        next_json(&mut client).await;
        next_json(&mut client).await;
        match change {
            "revoked" => key.is_active = false,
            "expired" => key.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(1)),
            "permission" => key.permissions = vec!["api.chat".into()],
            "model" | "endpoint" => {
                key.metadata.extra.insert("__core_keys".into(), json!({"permissions":{"allowed_models":if change=="model" {vec!["other"]} else {vec![]},"allowed_endpoints":if change=="endpoint" {vec!["/v1/chat/completions"]} else {vec![]},"is_admin":false,"custom_permissions":["api.realtime"]}}));
            }
            "rpm" => {
                key.rate_limits = Some(crate::core::models::RateLimits {
                    rpm: Some(1),
                    tpm: None,
                    rpd: None,
                    tpd: None,
                    concurrent: None,
                })
            }
            _ => {}
        }
        if change == "deleted" {
            state
                .storage
                .db()
                .delete_api_key(key.metadata.id)
                .await
                .unwrap();
        } else {
            state.storage.db().update_api_key(&key).await.unwrap();
        }
        client
            .send(Message::Text(
                json!({"type":"response.create"}).to_string().into(),
            ))
            .await
            .unwrap();
        if change == "rpm" {
            for _ in 0..4 {
                next_json(&mut client).await;
            }
            client
                .send(Message::Text(
                    json!({"type":"response.create"}).to_string().into(),
                ))
                .await
                .unwrap();
        }
        assert_eq!(next_json(&mut client).await["type"], "error", "{change}");
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|v| v["type"] == "response.create")
                .count(),
            usize::from(change == "rpm"),
            "{change}"
        );
        drop(client);
        for handle in handles {
            handle.stop(false).await;
        }
    }
}

#[actix_web::test]
async fn realtime_auth_middleware_returns_openai_error_envelopes() {
    assert_eq!(
        super::super::operation_for_path("/v1/realtime"),
        Some("realtime")
    );
    let (_, url, _, _, handles) = fixture().await;
    for key in [None, Some("invalid-local-key")] {
        let mut request = reqwest::Client::new().get(url.replace("ws://", "http://"));
        if let Some(key) = key {
            request = request.header("x-api-key", key);
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
        let error: Value = response.json().await.unwrap();
        assert!(error["error"]["message"].is_string());
    }
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn malformed_realtime_queries_use_openai_errors() {
    let (_, url, key, _, handles) = fixture().await;
    let base = url.split('?').next().unwrap().replace("ws://", "http://");
    for query in ["", "?extra=1", "?model=gpt-realtime-mini&extra=1"] {
        let response = reqwest::Client::new()
            .get(format!("{base}{query}"))
            .header("x-api-key", &key)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert!(body["error"]["message"].is_string());
    }
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn lowered_output_policy_reserves_only_the_effective_cap() {
    let (state, url, raw, calls, handles) = fixture().await;
    let (mut key, _) = state
        .auth
        .api_key()
        .verify_key(&raw)
        .await
        .unwrap()
        .unwrap();
    let mut client = client(&url, &raw).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    key.metadata.extra.insert("__core_keys".into(), json!({"permissions":{"allowed_models":[],"allowed_endpoints":[],"max_tokens_per_request":64,"is_admin":false,"custom_permissions":["api.realtime"]}}));
    state.storage.db().update_api_key(&key).await.unwrap();
    let bound = rates().bound(64);
    assert!(bound < rates().bound(rates().max_output));
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(bound + 0.000001, ResetPeriod::Monthly),
    );
    client
        .send(Message::Text(
            json!({"type":"response.create"}).to_string().into(),
        ))
        .await
        .unwrap();
    for kind in [
        "response.created",
        "response.output_audio.delta",
        "response.function_call_arguments.done",
        "response.done",
    ] {
        assert_eq!(next_json(&mut client).await["type"], kind);
    }
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .find(|v| v["type"] == "response.create")
            .unwrap()["response"]["max_output_tokens"],
        64
    );
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn precreation_provider_errors_affect_deployment_health() {
    for kind in [
        "invalid_request_error",
        "server_error",
        "rate_limit_error",
        "authentication_error",
    ] {
        let (state, url, raw, _, handles) = fixture().await;
        let mut client = client(&url, &raw).await;
        next_json(&mut client).await;
        next_json(&mut client).await;
        client.send(Message::Text(json!({"type":"response.create","response":{"metadata":{"reject":true,"reject_type":kind}}}).to_string().into())).await.unwrap();
        assert_eq!(next_json(&mut client).await["error"]["type"], kind);
        let router = state.pin_runtime().unified_router.clone();
        let deployment = router
            .get_deployment(&router.get_deployments_for_model("gpt-realtime-mini")[0])
            .unwrap();
        assert_eq!(
            deployment
                .state
                .fail_requests
                .load(std::sync::atomic::Ordering::Relaxed),
            u64::from(kind != "invalid_request_error"),
            "{kind}"
        );
        if matches!(kind, "rate_limit_error" | "authentication_error") {
            assert!(deployment.is_in_cooldown(), "{kind}");
        }
        drop(client);
        for handle in handles {
            handle.stop(false).await;
        }
    }
}

#[actix_web::test]
async fn response_boundaries_reauthorize_jwt_users_and_teams() {
    use crate::core::models::team::{Team, TeamMember, TeamRole, TeamStatus};
    use crate::core::teams::TeamRepository;
    use crate::storage::database::{SeaOrmTeamRepository, entities::user};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};

    for change in [
        "active",
        "deleted",
        "deactivated",
        "expired",
        "team",
        "membership",
    ] {
        let (state, url, raw, calls, handles) = fixture_with_config(|config| {
            config.gateway.auth.enable_jwt = true;
            config.gateway.auth.jwt_secret = "Realtime-JWT-fixture-only-123456789!".into();
        })
        .await;
        let (key, _) = state
            .auth
            .api_key()
            .verify_key(&raw)
            .await
            .unwrap()
            .unwrap();
        let owner = state
            .storage
            .db()
            .find_user_by_id(key.user_id.unwrap())
            .await
            .unwrap()
            .unwrap();
        let repository = SeaOrmTeamRepository::new(state.storage.database.clone());
        let mut team = repository
            .create(Team::new("realtime-jwt-team".into(), None))
            .await
            .unwrap();
        let mut token = if matches!(change, "team" | "membership") {
            repository
                .add_member(TeamMember::new(
                    team.id(),
                    owner.id(),
                    TeamRole::Member,
                    None,
                ))
                .await
                .unwrap();
            let proof = state
                .auth
                .validate_active_team(owner.id(), team.id())
                .await
                .unwrap()
                .unwrap();
            state
                .auth
                .jwt()
                .create_access_token_for_verified_team(
                    owner.id(),
                    "user".into(),
                    vec!["use:api".into()],
                    &proof,
                    None,
                )
                .await
                .unwrap()
        } else {
            state
                .auth
                .jwt()
                .create_access_token(
                    owner.id(),
                    "user".into(),
                    vec!["use:api".into()],
                    None,
                    None,
                )
                .await
                .unwrap()
        };
        if change == "expired" {
            let mut claims = state.auth.jwt().verify_access_token(&token).await.unwrap();
            // Cross the existing JWT verifier's 60-second clock tolerance without
            // adding a minute-long sleep to the WebSocket regression.
            claims.exp = chrono::Utc::now().timestamp() as u64 - 50;
            token = jsonwebtoken::encode(
                &jsonwebtoken::Header::default(),
                &claims,
                &jsonwebtoken::EncodingKey::from_secret(
                    state
                        .pin_runtime()
                        .config
                        .gateway
                        .auth
                        .jwt_secret
                        .as_bytes(),
                ),
            )
            .unwrap();
        }
        let mut request = url.as_str().into_client_request().unwrap();
        request
            .headers_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        let parsed = url::Url::parse(&url).unwrap();
        let tcp =
            tokio::net::TcpStream::connect((parsed.host_str().unwrap(), parsed.port().unwrap()))
                .await
                .unwrap();
        let mut client = tokio_tungstenite::client_async(request, tcp)
            .await
            .unwrap()
            .0;
        next_json(&mut client).await;
        next_json(&mut client).await;
        match change {
            "deleted" => {
                state
                    .storage
                    .db()
                    .delete_api_key(key.metadata.id)
                    .await
                    .unwrap();
                state
                    .storage
                    .db()
                    .delete_user(&owner.id().to_string())
                    .await
                    .unwrap();
                user::Entity::delete_by_id(owner.id())
                    .exec(state.storage.db().connection())
                    .await
                    .unwrap();
            }
            "deactivated" => {
                user::ActiveModel {
                    id: Set(owner.id()),
                    status: Set("suspended".into()),
                    ..Default::default()
                }
                .update(state.storage.db().connection())
                .await
                .unwrap();
            }
            "expired" => tokio::time::sleep(Duration::from_secs(12)).await,
            "team" => {
                team.status = TeamStatus::Inactive;
                repository.update(team).await.unwrap();
            }
            "membership" => repository
                .remove_member(team.id(), owner.id())
                .await
                .unwrap(),
            _ => {}
        }
        client
            .send(Message::Text(
                json!({"type":"response.create"}).to_string().into(),
            ))
            .await
            .unwrap();
        let event = next_json(&mut client).await;
        if change == "active" {
            assert_eq!(event["type"], "response.created");
            for _ in 0..3 {
                next_json(&mut client).await;
            }
        } else {
            assert_eq!(event["type"], "error", "{change}");
            assert_eq!(event["error"]["type"], "authentication_error", "{change}");
        }
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|v| v["type"] == "response.create")
                .count(),
            usize::from(change == "active"),
            "{change}"
        );
        drop(client);
        for handle in handles {
            handle.stop(false).await;
        }
    }
}

#[actix_web::test]
async fn rejected_session_updates_do_not_change_the_response_cap() {
    let (_, url, raw, calls, handles) = fixture().await;
    let mut client = client(&url, &raw).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    client
        .send(Message::Text(
            json!({"type":"session.update","session":{"max_output_tokens":128}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut client).await["type"], "session.updated");
    client.send(Message::Text(json!({"type":"session.update","session":{"max_output_tokens":1024,"audio":{"output":{"voice":"reject-voice"}}}}).to_string().into())).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["error"]["type"],
        "invalid_request_error"
    );
    client
        .send(Message::Text(
            json!({"type":"response.create"}).to_string().into(),
        ))
        .await
        .unwrap();
    for _ in 0..4 {
        next_json(&mut client).await;
    }
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .find(|value| value["type"] == "response.create")
            .unwrap()["response"]["max_output_tokens"],
        128
    );
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn completed_responses_survive_failed_budget_settlement() {
    let (state, url, raw, _, handles) = fixture().await;
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    let (key, _) = state
        .auth
        .api_key()
        .verify_key(&raw)
        .await
        .unwrap()
        .unwrap();
    let mut client = client(&url, &raw).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    client
        .send(Message::Text(
            json!({"type":"response.create","response":{"metadata":{"hold":true}}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut client).await["type"], "response.created");
    // Exercise a real settlement error after admission without an external Redis
    // dependency. This does not claim a Redis disconnect/reconciliation test.
    state
        .budget_limits
        .providers
        .budgets
        .get_mut("openai")
        .unwrap()
        .current_spend = f64::NAN;
    client
        .send(Message::Text(
            json!({"type":"input_audio_buffer.clear"})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut client).await["type"], "response.done");
    let stored = state
        .storage
        .db()
        .find_api_key_by_id(key.metadata.id)
        .await
        .unwrap()
        .unwrap();
    let (cost, tokens) = rates().cost(&usage()).unwrap();
    assert_eq!(stored.usage_stats.total_tokens, tokens);
    assert!((stored.usage_stats.total_cost - cost).abs() < 1e-12);
    let router = state.pin_runtime().unified_router.clone();
    let deployment = router
        .get_deployment(&router.get_deployments_for_model("gpt-realtime-mini")[0])
        .unwrap();
    assert_eq!(
        deployment
            .state
            .fail_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn infinite_output_and_pinned_session_models_reach_upstream() {
    let (state, url, raw, calls, handles) = fixture().await;
    let (mut key, _) = state
        .auth
        .api_key()
        .verify_key(&raw)
        .await
        .unwrap()
        .unwrap();
    let mut client = client(&url, &raw).await;
    next_json(&mut client).await;
    next_json(&mut client).await;
    for model in ["gpt-realtime-mini", "gpt-realtime-mini-mapped"] {
        client.send(Message::Text(json!({"type":"session.update","session":{"model":model,"max_output_tokens":"inf"}}).to_string().into())).await.unwrap();
        assert_eq!(next_json(&mut client).await["type"], "session.updated");
    }
    key.metadata.extra.insert("__core_keys".into(), json!({"permissions":{"allowed_models":[],"allowed_endpoints":[],"max_tokens_per_request":64,"is_admin":false,"custom_permissions":["api.realtime"]}}));
    state.storage.db().update_api_key(&key).await.unwrap();
    client
        .send(Message::Text(
            json!({"type":"response.create","response":{"max_output_tokens":"inf"}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    for kind in [
        "response.created",
        "response.output_audio.delta",
        "response.function_call_arguments.done",
        "response.done",
    ] {
        assert_eq!(next_json(&mut client).await["type"], kind);
    }
    let events = calls.lock().unwrap().clone();
    for event in events
        .iter()
        .filter(|event| event["type"] == "session.update")
        .skip(1)
    {
        assert!(event["session"].get("model").is_none());
        assert_eq!(event["session"]["max_output_tokens"], rates().max_output);
    }
    assert_eq!(
        events
            .iter()
            .find(|event| event["type"] == "response.create")
            .unwrap()["response"]["max_output_tokens"],
        64
    );
    client
        .send(Message::Text(
            json!({"type":"session.update","session":{"model":"other-model"}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        next_json(&mut client).await["error"]["type"],
        "invalid_request_error"
    );
    drop(client);
    for handle in handles {
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn zero_output_keys_do_not_acquire_or_penalize_deployments() {
    let (state, _, raw, calls, handles) = fixture().await;
    let (mut key, _) = state
        .auth
        .api_key()
        .verify_key(&raw)
        .await
        .unwrap()
        .unwrap();
    key.metadata.extra.insert("__core_keys".into(), json!({"permissions":{"allowed_models":[],"allowed_endpoints":[],"max_tokens_per_request":0,"is_admin":false,"custom_permissions":["api.realtime"]}}));
    state.storage.db().update_api_key(&key).await.unwrap();
    let app = test::init_service(crate::server::http::HttpServer::create_app(web::Data::new(
        state.clone(),
    )))
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/realtime?model=gpt-realtime-mini")
            .insert_header(("x-api-key", raw))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), actix_web::http::StatusCode::FORBIDDEN);
    assert!(calls.lock().unwrap().is_empty());
    let router = state.pin_runtime().unified_router.clone();
    let deployment = router
        .get_deployment(&router.get_deployments_for_model("gpt-realtime-mini")[0])
        .unwrap();
    use std::sync::atomic::Ordering;
    assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    for handle in handles {
        handle.stop(false).await;
    }
}
