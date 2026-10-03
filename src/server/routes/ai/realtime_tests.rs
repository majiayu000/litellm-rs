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
    assert!((rates.bound() - 0.40192).abs() < 1e-12);
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
        assert!(prepare_event(&mut event, &rates, false).is_err());
    }
    let mut event = json!({"type":"response.create","response":{"tools":[{"type":"function","name":"weather"}]}});
    assert!(prepare_event(&mut event, &rates, false).unwrap());
    assert_eq!(event["response"]["max_output_tokens"], rates.max_output);
    assert!(prepare_event(&mut event, &rates, true).is_err());
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
                            if value["response"]["metadata"]["hold"] == true {
                                if session.text(json!({"type":"response.created","response":{"id":"response-1"}}).to_string()).await.is_err() { break; }
                                continue;
                            }
                            for event in [
                                json!({"type":"response.created","response":{"id":"response-1"}}),
                                json!({"type":"response.output_audio.delta","delta":"aGVsbG8="}),
                                json!({"type":"response.function_call_arguments.done","arguments":"{}","call_id":"call-1"}),
                                json!({"type":"response.done","response":{"id":"response-1","status":"completed","usage":usage()}}),
                            ] {
                                if session.text(event.to_string()).await.is_err() {
                                    break;
                                }
                            }
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
    let calls: Calls = Arc::default();
    let copy = calls.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(copy.clone()))
            .route("/v1/realtime", web::get().to(upstream))
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
    provider.settings.insert("model_identity_mappings".into(), json!({"gpt-realtime-mini":{"capability_catalog_model":"gpt-realtime-mini","pricing_model":"gpt-realtime-mini"}}));
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
    let (_, url, key, _, handles) = fixture().await;
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
    let bound = rates().bound();
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
            if (stored.usage_stats.total_cost - rates().bound()).abs() < 1e-9
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
    for handle in handles {
        handle.stop(false).await;
    }
}
