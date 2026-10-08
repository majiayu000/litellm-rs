//! The real Gemini HTTP route must retain observations despite its numeric zero return.
use crate::config::Config;
use crate::config::models::provider::ProviderConfig;
use crate::core::net::ProviderEndpointAccess;
use crate::core::router::deployment::current_timestamp;
use crate::core::router::{RouterConfig, UnifiedRouter};
use crate::core::types::model::ProviderCapability;
use crate::storage::redis::RedisPool;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::Relaxed},
};
use std::time::Duration;

#[derive(Clone)]
struct Upstream {
    calls: Arc<AtomicUsize>,
    response: Value,
}

async fn respond(
    state: web::Data<Upstream>,
    req: HttpRequest,
    body: web::Json<Value>,
) -> HttpResponse {
    assert!(
        req.path()
            .ends_with("/models/gemini-2.5-flash:generateContent")
    );
    assert_eq!(body["contents"][0]["parts"][0]["text"], "Hello");
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 64);
    state.calls.fetch_add(1, Relaxed);
    HttpResponse::Ok().json(&state.response)
}

#[tokio::test]
async fn gemini_unary_http_preserves_observed_usage_over_numeric_zero() {
    // OpenAI-compatible Gemini transport is shipped with the gateway. Also
    // exercise the native Gemini factory when that provider is compiled in.
    let selectors = [
        "openai_compatible",
        #[cfg(feature = "providers-extended")]
        "gemini",
    ];
    for selector in selectors {
        for shared in [false, true] {
            let redis_url = if shared {
                match std::env::var("REDIS_URL") {
                    Ok(url) => Some(url),
                    Err(_) if std::env::var_os("CI").is_some() => {
                        panic!("CI requires actual REDIS_URL")
                    }
                    Err(_) => {
                        eprintln!("Skipping shared Gemini usage cases: REDIS_URL unset");
                        continue;
                    }
                }
            } else {
                None
            };
            for tokens in [None, Some(0), Some(15)] {
                let body = json!({
                    "contents": [{"role":"user","parts":[{"text":"Hello"}]}],
                    "generationConfig": {"maxOutputTokens":64}
                });
                let estimate =
                    super::super::gemini_estimated_admission_tokens(&body, "gemini-2.5-flash")
                        .unwrap();
                assert!(estimate > 15);
                // E + 15 permits two known 15-token responses, then blocks a
                // third. Unknown usage retains E and blocks the second call.
                let limit = u32::try_from(estimate + 15).unwrap();
                let mut output = json!({"candidates":[{"content":{"role":"model","parts":[{"text":"result"}]},"finishReason":"STOP"}]});
                if let Some(tokens) = tokens {
                    output["usageMetadata"] = json!({
                        "promptTokenCount": tokens,
                        "candidatesTokenCount": 0,
                        "totalTokenCount": tokens
                    });
                }
                // Shared strict parsing intentionally treats an all-zero
                // metadata object as unknown; this test preserves that contract.
                let observed = tokens.filter(|tokens| *tokens > 0);
                let calls = Arc::new(AtomicUsize::new(0));
                let upstream = Upstream {
                    calls: calls.clone(),
                    response: output.clone(),
                };
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let address = listener.local_addr().unwrap();
                let server = HttpServer::new(move || {
                    App::new()
                        .app_data(web::Data::new(upstream.clone()))
                        .default_service(web::post().to(respond))
                })
                .workers(1)
                .listen(listener)
                .unwrap()
                .run();
                let handle = server.handle();
                let task = tokio::spawn(server);
                let mut config = Config::default();
                config.gateway.auth.enable_jwt = false;
                config.gateway.auth.enable_api_key = false;
                config.gateway.auth.allow_anonymous = true;
                config.gateway.storage.database.enabled = false;
                config.gateway.storage.redis.enabled = redis_url.is_some();
                config.gateway.storage.redis.allow_degraded = false;
                if let Some(url) = &redis_url {
                    config.gateway.storage.redis.url = url.clone();
                }
                config.gateway.router.load_balancer.health_check_enabled = false;
                config.gateway.providers = vec![ProviderConfig {
                    name: format!("gemini-usage-{}", uuid::Uuid::new_v4()),
                    provider_type: selector.into(),
                    api_key: "test-gemini-usage-key-12345678901234567890".into(),
                    base_url: Some(format!("http://{address}")),
                    endpoint_access: ProviderEndpointAccess::PrivateNetwork,
                    models: vec!["gemini-2.5-flash".into()],
                    settings: std::collections::HashMap::from([(
                        "provider_name".into(),
                        json!("gemini"),
                    )]),
                    rpm: 100,
                    tpm: limit,
                    max_concurrent_requests: 100,
                    ..Default::default()
                }];
                let state = crate::server::HttpServer::new(&config)
                    .await
                    .unwrap()
                    .state()
                    .clone();
                // Preserve the real provider factory and shared backends, but
                // make admission denial return before the minute can roll over.
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
                let ids = router.get_deployments_for_model("gemini-2.5-flash");
                assert_eq!(ids.len(), 1);
                let deployment = router.get_deployment(&ids[0]).unwrap();
                assert!(deployment.provider.supports_capability_for_model(
                    "gemini-2.5-flash",
                    &ProviderCapability::GeminiGenerateContent,
                ));
                let mut connection = match &redis_url {
                    Some(url) => Some(
                        redis::Client::open(url.as_str())
                            .unwrap()
                            .get_multiplexed_async_connection()
                            .await
                            .unwrap(),
                    ),
                    None => None,
                };
                wait_for_window(connection.as_mut()).await;
                deployment.state.reset_minute();
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(state))
                        .configure(crate::server::routes::ai::configure_routes),
                )
                .await;
                let successful = if observed.is_some() { 2 } else { 1 };
                for completed in 1..=successful {
                    let response = test::call_service(
                        &app,
                        test::TestRequest::post()
                            .uri("/v1beta/models/gemini-2.5-flash:generateContent")
                            .set_json(&body)
                            .to_request(),
                    )
                    .await;
                    assert_eq!(
                        response.status(),
                        StatusCode::OK,
                        "{selector}/{tokens:?}/{shared}"
                    );
                    let actual: Value = test::read_body_json(response).await;
                    assert_eq!(actual, output);
                    assert_eq!(calls.load(Relaxed), completed as usize);
                    assert_eq!(deployment.state.rpm_current.load(Relaxed), completed);
                    assert_eq!(
                        deployment.state.tpm_current.load(Relaxed),
                        observed.unwrap_or(0) * completed
                    );
                    assert_eq!(deployment.state.success_requests.load(Relaxed), completed);
                    assert_eq!(deployment.state.total_requests.load(Relaxed), completed);
                    assert_eq!(deployment.state.fail_requests.load(Relaxed), 0);
                    assert_eq!(deployment.state.active_requests.load(Relaxed), 0);
                    if let Some(connection) = &mut connection {
                        assert_shared_usage(
                            connection,
                            &deployment.shared_state_id(),
                            completed,
                            observed.map_or(estimate, |tokens| tokens * completed),
                        )
                        .await;
                    }
                }
                let rejected = tokio::time::timeout(
                    Duration::from_secs(5),
                    test::call_service(
                        &app,
                        test::TestRequest::post()
                            .uri("/v1beta/models/gemini-2.5-flash:generateContent")
                            .set_json(&body)
                            .to_request(),
                    ),
                )
                .await
                .expect("quota denial must return without retrying into another minute");
                assert!(
                    !rejected.status().is_success(),
                    "TPM must reject before dispatch"
                );
                assert_eq!(calls.load(Relaxed), successful as usize);
                assert_eq!(deployment.state.rpm_current.load(Relaxed), successful);
                assert_eq!(
                    deployment.state.tpm_current.load(Relaxed),
                    observed.unwrap_or(0) * successful
                );
                assert_eq!(deployment.state.total_requests.load(Relaxed), successful);
                assert_eq!(deployment.state.fail_requests.load(Relaxed), 0);
                assert_eq!(deployment.state.active_requests.load(Relaxed), 0);
                if let Some(connection) = &mut connection {
                    let id = deployment.shared_state_id();
                    assert_shared_usage(
                        connection,
                        &id,
                        successful,
                        observed.map_or(estimate, |tokens| tokens * successful),
                    )
                    .await;
                    let _: i64 = redis::cmd("DEL")
                        .arg(&[RedisPool::admission_key(&id), RedisPool::circuit_key(&id)])
                        .query_async(connection)
                        .await
                        .unwrap();
                }
                handle.stop(false).await;
                task.await.unwrap().unwrap();
            }
        }
    }
}

async fn assert_shared_usage(
    connection: &mut redis::aio::MultiplexedConnection,
    id: &str,
    requests: u64,
    tokens: u64,
) {
    let key = RedisPool::admission_key(id);
    let counts: (u64, u64, u64) = redis::cmd("HMGET")
        .arg(&key)
        .arg(&["p", "r", "t"])
        .query_async(&mut *connection)
        .await
        .unwrap();
    assert_eq!(counts, (0, requests, tokens));
    let fields: Vec<String> = redis::cmd("HKEYS")
        .arg(&key)
        .query_async(connection)
        .await
        .unwrap();
    assert!(!fields.iter().any(|field| field.starts_with("l:")));
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
