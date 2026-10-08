//! Raw finite usage must not release a deployment's full admission estimate.
use crate::config::Config;
use crate::config::models::provider::ProviderConfig;
use crate::core::models::openai::ChatCompletionRequest;
use crate::core::net::ProviderEndpointAccess;
use crate::core::router::deployment::{Deployment, current_timestamp};
use crate::core::router::{RouterConfig, UnifiedRouter};
use crate::core::types::embedding::EmbeddingInput;
use crate::core::types::model::ProviderCapability;
use crate::storage::redis::RedisPool;
use crate::utils::ai::counter::token_counter::TokenizerIdentity;
use actix_web::{App, HttpResponse, HttpServer, web};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::Relaxed},
};
use std::time::Duration;

const CHAT_MODEL: &str = "gpt-4o-mini";
const EMBEDDING_MODEL: &str = "text-embedding-3-small";
const EMBEDDING_INPUT: &str =
    "This fixture contains enough embedding text to reserve more than three input tokens.";
const OUTPUT: &str = "unary-usage-marker";

#[derive(Clone, Copy, Debug)]
enum Report {
    Zero,
    Inconsistent,
    TooLarge,
    MalformedDetails,
    WrongSideDetails,
    CachedOverPrompt,
    ValidCached,
    ReadOverPrompt,
    CreationOverPrompt,
    CacheSumOverPrompt,
    ValidCacheBreakdown,
    Valid,
}

impl Report {
    fn known(self) -> Option<u64> {
        match self {
            Self::Valid | Self::ValidCached | Self::ValidCacheBreakdown => Some(3),
            _ => None,
        }
    }

    fn wire(self, embedding: bool) -> Value {
        let extended = match self {
            Self::TooLarge => {
                let tokens = u64::from(u32::MAX) + 1;
                Some(json!({
                    "prompt_tokens":tokens,"completion_tokens":0,"total_tokens":tokens
                }))
            }
            Self::MalformedDetails => Some(json!({
                "prompt_tokens":3,"completion_tokens":0,"total_tokens":3,
                "prompt_tokens_details":{"cached_tokens":"3"}
            })),
            Self::CachedOverPrompt | Self::ValidCached => Some(json!({
                "prompt_tokens":3,"completion_tokens":0,"total_tokens":3,
                "prompt_tokens_details":{"cached_tokens":if matches!(self, Self::ValidCached) {3} else {100}}
            })),
            Self::ReadOverPrompt
            | Self::CreationOverPrompt
            | Self::CacheSumOverPrompt
            | Self::ValidCacheBreakdown => Some(json!({
                "prompt_tokens":3,"completion_tokens":0,"total_tokens":3,
                "prompt_tokens_details":match self {
                    Self::ReadOverPrompt => json!({"cached_tokens":1,"cache_read_tokens":100}),
                    Self::CreationOverPrompt => json!({"cache_creation_tokens":100}),
                    Self::CacheSumOverPrompt => json!({"cached_tokens":2,"cache_creation_tokens":2}),
                    Self::ValidCacheBreakdown => json!({"cached_tokens":3,"cache_read_tokens":1,"cache_creation_tokens":2,"audio_tokens":1}),
                    _ => unreachable!(),
                }
            })),
            Self::WrongSideDetails => Some(json!({
                "prompt_tokens":3,"completion_tokens":0,"total_tokens":3,
                "completion_tokens_details":{"cached_tokens":"3"}
            })),
            _ => None,
        };
        if let Some(mut usage) = extended {
            if embedding {
                usage.as_object_mut().unwrap().remove("completion_tokens");
            }
            return usage;
        }
        if embedding {
            match self {
                Self::Zero => json!({"prompt_tokens":0,"total_tokens":0}),
                Self::Inconsistent => json!({"prompt_tokens":100,"total_tokens":1}),
                Self::Valid => json!({"prompt_tokens":3,"total_tokens":3}),
                Self::TooLarge
                | Self::MalformedDetails
                | Self::WrongSideDetails
                | Self::CachedOverPrompt
                | Self::ValidCached
                | Self::ReadOverPrompt
                | Self::CreationOverPrompt
                | Self::CacheSumOverPrompt
                | Self::ValidCacheBreakdown => unreachable!(),
            }
        } else {
            match self {
                Self::Zero => {
                    json!({"prompt_tokens":0,"completion_tokens":0,"total_tokens":0})
                }
                Self::Inconsistent => {
                    json!({"prompt_tokens":100,"completion_tokens":50,"total_tokens":1})
                }
                Self::Valid => {
                    json!({"prompt_tokens":3,"completion_tokens":0,"total_tokens":3})
                }
                Self::TooLarge
                | Self::MalformedDetails
                | Self::WrongSideDetails
                | Self::CachedOverPrompt
                | Self::ValidCached
                | Self::ReadOverPrompt
                | Self::CreationOverPrompt
                | Self::CacheSumOverPrompt
                | Self::ValidCacheBreakdown => unreachable!(),
            }
        }
    }
}

#[derive(Clone)]
struct Upstream {
    calls: Arc<AtomicUsize>,
    report: Report,
    embedding: bool,
}

async fn upstream(state: web::Data<Upstream>, body: web::Json<Value>) -> HttpResponse {
    state.calls.fetch_add(1, Relaxed);
    let model = if state.embedding {
        EMBEDDING_MODEL
    } else {
        CHAT_MODEL
    };
    assert_eq!(body["model"], model);
    let usage = state.report.wire(state.embedding);
    if state.embedding {
        assert_eq!(body["input"], EMBEDDING_INPUT);
        HttpResponse::Ok().json(json!({
            "object":"list","model":model,
            "data":[{"object":"embedding","index":0,"embedding":[0.25,0.5]}],
            "usage":usage
        }))
    } else {
        assert_eq!(body["messages"][0]["content"], "Hello");
        assert_ne!(body["stream"], true);
        HttpResponse::Ok().json(json!({
            "id":"unary-usage-fixture","object":"chat.completion","created":1,
            "model":model,
            "choices":[{"index":0,"message":{"role":"assistant","content":OUTPUT},
                "finish_reason":"stop","logprobs":null}],
            "usage":usage
        }))
    }
}

fn request_body(route: &str) -> Value {
    match route {
        "/v1/embeddings" => {
            json!({"model":EMBEDDING_MODEL,"input":EMBEDDING_INPUT})
        }
        "/v1/completions" => {
            json!({"model":CHAT_MODEL,"prompt":"Hello","max_tokens":64,"stream":false})
        }
        "/v1/responses" => {
            json!({"model":CHAT_MODEL,"input":"Hello","max_output_tokens":64,"stream":false,"store":false})
        }
        _ => {
            json!({"model":CHAT_MODEL,"messages":[{"role":"user","content":"Hello"}],"max_tokens":64,"stream":false})
        }
    }
}

fn assert_output(body: &Value, embedding: bool) {
    if embedding {
        assert_eq!(body["data"][0]["embedding"], json!([0.25, 0.5]));
    } else {
        assert!(body.to_string().contains(OUTPUT), "{body}");
    }
}

#[tokio::test]
async fn raw_unary_usage_keeps_admission_until_a_trusted_count_is_available() {
    for provider_type in ["openai", "openai_compatible"] {
        for route in [
            "/v1/chat/completions",
            "/v1/completions",
            "/v1/embeddings",
            "/v1/responses",
        ] {
            // Direct OpenAI Responses uses its native validated-zero contract.
            // Exercise the chat conversion only when the provider selects it.
            if provider_type == "openai" && route == "/v1/responses" {
                continue;
            }
            for shared in [false, true] {
                let redis_url = if shared {
                    match std::env::var("REDIS_URL") {
                        Ok(url) => Some(url),
                        Err(_) if std::env::var_os("CI").is_some() => {
                            panic!("CI requires actual REDIS_URL")
                        }
                        Err(_) => {
                            eprintln!("Skipping shared unary HTTP cases: REDIS_URL unset");
                            continue;
                        }
                    }
                } else {
                    None
                };
                for report in [
                    Report::Zero,
                    Report::Inconsistent,
                    Report::TooLarge,
                    Report::MalformedDetails,
                    Report::WrongSideDetails,
                    Report::Valid,
                    Report::CachedOverPrompt,
                    Report::ValidCached,
                    Report::ReadOverPrompt,
                    Report::CreationOverPrompt,
                    Report::CacheSumOverPrompt,
                    Report::ValidCacheBreakdown,
                ] {
                    if route == "/v1/embeddings" && matches!(report, Report::WrongSideDetails) {
                        continue;
                    }
                    let mut fixture =
                        Fixture::new(provider_type, route, redis_url.as_deref(), report).await;
                    let endpoint = format!("{}{route}", fixture.base_url);
                    let body = request_body(route);
                    let response = fixture
                        .client
                        .post(&endpoint)
                        .json(&body)
                        .send()
                        .await
                        .unwrap();
                    let status = response.status();
                    let output: Value = response.json().await.unwrap();
                    assert!(status.is_success(), "{}: {output}", fixture.case);
                    assert_output(&output, route == "/v1/embeddings");
                    let known = report.known();
                    fixture
                        .assert_accounting(1, known.unwrap_or(0), known.unwrap_or(fixture.estimate))
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
                    if let Some(tokens) = known {
                        assert!(second.status().is_success(), "{}", fixture.case);
                        let output: Value = second.json().await.unwrap();
                        assert_output(&output, route == "/v1/embeddings");
                        fixture.assert_accounting(2, tokens * 2, tokens * 2).await;
                        assert_eq!(fixture.calls.load(Relaxed), 2);
                    } else {
                        assert!(
                            !second.status().is_success(),
                            "{}: invalid raw usage must retain the full estimate",
                            fixture.case
                        );
                        fixture.assert_accounting(1, 0, fixture.estimate).await;
                        assert_eq!(
                            fixture.calls.load(Relaxed),
                            1,
                            "{}: TPM rejection must happen before upstream dispatch",
                            fixture.case
                        );
                    }
                    fixture.shutdown().await;
                }
            }
        }
    }
}

struct Fixture {
    case: String,
    base_url: String,
    client: reqwest::Client,
    calls: Arc<AtomicUsize>,
    deployment: Arc<Deployment>,
    estimate: u64,
    connection: Option<redis::aio::MultiplexedConnection>,
    handles: Vec<actix_web::dev::ServerHandle>,
    tasks: Vec<tokio::task::JoinHandle<std::io::Result<()>>>,
}

impl Fixture {
    async fn new(
        provider_type: &str,
        route: &str,
        redis_url: Option<&str>,
        report: Report,
    ) -> Self {
        let embedding = route == "/v1/embeddings";
        let model = if embedding {
            EMBEDDING_MODEL
        } else {
            CHAT_MODEL
        };
        let estimate = if embedding {
            u64::from(
                super::spend::estimate_embedding_input_tokens(
                    &TokenizerIdentity::approximate("gateway", model),
                    &EmbeddingInput::Text(EMBEDDING_INPUT.into()),
                )
                .unwrap(),
            )
        } else {
            let normalized: ChatCompletionRequest = if route == "/v1/responses" {
                let request = serde_json::from_value(request_body(route)).unwrap();
                super::responses::build_chat_request(&request).unwrap()
            } else {
                serde_json::from_value(request_body("/v1/chat/completions")).unwrap()
            };
            super::execution::estimate::chat(&normalized, 0).unwrap()
        };
        assert!(estimate > 3);
        let calls = Arc::new(AtomicUsize::new(0));
        let upstream_state = Upstream {
            calls: calls.clone(),
            report,
            embedding,
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
        config.gateway.guardrails.enabled = false;
        config.gateway.providers = vec![ProviderConfig {
            name: format!("unary-usage-{}", uuid::Uuid::new_v4()),
            provider_type: provider_type.into(),
            api_key: "sk-unary-usage-fixture".into(),
            base_url: Some(format!("http://{upstream_address}/v1")),
            endpoint_access: ProviderEndpointAccess::PrivateNetwork,
            models: vec![model.into()],
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
        assert!(state.pin_runtime().response_cache.is_none());
        let router = UnifiedRouter::from_gateway_config_with_pricing(
            &config.gateway.providers,
            Some(RouterConfig {
                num_retries: 0,
                max_fallbacks: 0,
                enable_pre_call_checks: false,
                ..Default::default()
            }),
            state.pricing.clone(),
        )
        .await
        .unwrap()
        .with_admission_redis(state.storage.redis.clone())
        .with_circuit_redis(state.storage.redis.clone());
        let mut revision = state.pin_runtime().as_ref().clone();
        revision.unified_router = Arc::new(router);
        state.runtime.store(revision);
        let router = state.unified_router();
        let ids = router.get_deployments_for_model(model);
        assert_eq!(ids.len(), 1);
        let deployment = router.get_deployment(&ids[0]).unwrap();
        if route == "/v1/responses" {
            assert!(
                !deployment
                    .provider
                    .supports_capability_for_model(model, &ProviderCapability::Responses),
                "this case must use compatibility Responses"
            );
        }
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
                .configure(super::configure_routes)
        })
        .workers(1)
        .listen(listener)
        .unwrap()
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        Self {
            case: format!(
                "{provider_type}/{route}/{report:?}/shared={}",
                redis_url.is_some()
            ),
            base_url: format!("http://{address}"),
            client: reqwest::Client::builder()
                .no_proxy()
                .http1_only()
                .pool_max_idle_per_host(0)
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap(),
            calls,
            deployment,
            estimate,
            connection,
            handles: vec![handle, upstream_handle],
            tasks: vec![task, upstream_task],
        }
    }

    async fn assert_accounting(&mut self, requests: u64, tokens: u64, admission_tokens: u64) {
        let state = &self.deployment.state;
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if (
                    state.active_requests.load(Relaxed),
                    state.rpm_current.load(Relaxed),
                    state.tpm_current.load(Relaxed),
                    state.success_requests.load(Relaxed),
                    state.fail_requests.load(Relaxed),
                    state.total_requests.load(Relaxed),
                ) == (0, requests, tokens, requests, 0, requests)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("{}: local unary accounting did not finish", self.case));
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
            .expect("shared unary admission must settle once");
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
