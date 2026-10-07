//! Exercise the public SDK facade as a downstream crate with isolated runtimes.
use futures::future::BoxFuture;
use litellm_rs::{
    core::{
        providers::{ExternalProvider, Provider, ProviderError},
        router::{Deployment, RuntimeBinding, UnifiedRouter},
        types::{
            chat::ChatRequest,
            context::RequestContext,
            embedding::EmbeddingRequest,
            model::{ModelInfo, ProviderCapability},
            responses::{ChatResponse, EmbeddingResponse},
        },
    },
    sdk::{LLMClient, errors::SDKError},
};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

type Calls = Arc<Mutex<Vec<EmbeddingRequest>>>;

#[derive(Debug)]
struct EmbeddingProvider {
    models: Vec<ModelInfo>,
    calls: Calls,
    marker: f32,
    fail: bool,
    usage: Option<u32>,
    gate: Option<Arc<tokio::sync::Semaphore>>,
    dispatches: Arc<AtomicUsize>,
}

impl EmbeddingProvider {
    async fn begin_request(&self) {
        self.dispatches.fetch_add(1, Ordering::Relaxed);
        if let Some(gate) = &self.gate {
            gate.acquire().await.unwrap().forget();
        }
    }
}

impl ExternalProvider for EmbeddingProvider {
    fn name(&self) -> &str {
        "sdk-embedding-test"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::Embeddings,
            ProviderCapability::ChatCompletion,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<ChatResponse, ProviderError>> {
        Box::pin(async move {
            self.begin_request().await;
            Ok(serde_json::from_value(json!({
                "id":"unary-test", "object":"chat.completion", "created":1,
                "model":request.model,
                "choices":[{"index":0,"message":{"role":"assistant","content":"ready"},
                    "finish_reason":"stop"}],
                "usage":self.usage.map(|tokens| json!({
                    "prompt_tokens":tokens,"completion_tokens":0,"total_tokens":tokens
                }))
            }))
            .unwrap())
        })
    }

    fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> BoxFuture<'_, Result<EmbeddingResponse, ProviderError>> {
        Box::pin(async move {
            assert!(!context.request_id.is_empty());
            let count = request.input.to_vec().len();
            let model = request.model.clone();
            self.calls.lock().unwrap().push(request);
            self.begin_request().await;
            if self.fail {
                return Err(ProviderError::api_error(
                    "sdk-embedding-test",
                    403,
                    "embedding access denied",
                ));
            }
            // Upstreams may return entries out of order; preserve input order
            // in the SDK batch result by respecting each entry's index.
            let data: Vec<_> = (0..count)
                .rev()
                .map(|index| {
                    json!({"object":"embedding", "index":index,
                        "embedding":[self.marker, index as f32]})
                })
                .collect();
            Ok(serde_json::from_value(json!({
                "object":"list", "model":model, "data":data,
                "usage":self.usage.map(|tokens| json!({
                    "prompt_tokens":tokens,"completion_tokens":0,"total_tokens":tokens
                }))
            }))
            .unwrap())
        })
    }
}

fn client(marker: f32, fail: bool) -> (LLMClient, Calls, Arc<UnifiedRouter>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let provider = Provider::External(Arc::new(EmbeddingProvider {
        models: vec![ModelInfo {
            id: "vendor/embed-v1".into(),
            capabilities: vec![ProviderCapability::Embeddings],
            ..Default::default()
        }],
        calls: Arc::clone(&calls),
        marker,
        fail,
        usage: Some(3),
        gate: None,
        dispatches: Arc::new(AtomicUsize::new(0)),
    }));
    let router = Arc::new(UnifiedRouter::default());
    router.add_deployment(Deployment::new(
        "embeddings".into(),
        provider,
        "vendor/embed-v1".into(),
        "my-embeddings".into(),
    ));
    router
        .add_model_alias("openai/text-embedding-3-small", "my-embeddings")
        .unwrap();
    let client =
        LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "my-embeddings").unwrap();
    (client, calls, router)
}

#[tokio::test]
async fn sdk_embeddings_use_the_bound_runtime_for_default_and_explicit_models() {
    let (first, first_calls, router) = client(1.0, false);
    let (second, second_calls, _) = client(2.0, false);
    // Neither SDK has a legacy provider config or a global runtime installed.
    assert!(first.config().providers.is_empty());
    assert_eq!(first.embedding("one", None).await.unwrap(), [1.0, 0.0]);
    assert_eq!(
        second
            .embedding("two", Some("openai/text-embedding-3-small"))
            .await
            .unwrap(),
        [2.0, 0.0]
    );
    let texts = vec!["first".to_string(), "second".to_string()];
    assert_eq!(
        first
            .batch_embedding(&texts, Some("my-embeddings"))
            .await
            .unwrap(),
        vec![vec![1.0, 0.0], vec![1.0, 1.0]]
    );
    let calls = first_calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|request| request.model == "vendor/embed-v1")
    );
    assert_eq!(calls[0].input.to_vec(), ["one"]);
    assert_eq!(calls[1].input.to_vec(), texts);
    assert_eq!(second_calls.lock().unwrap().len(), 1);
    let deployment = router.get_deployment("embeddings").unwrap();
    assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 2);
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn sdk_embedding_unknown_models_do_not_fall_back_to_the_default() {
    let (client, calls, _) = client(1.0, false);
    assert!(matches!(
        client.embedding("one", Some("unknown/model")).await,
        Err(SDKError::ModelNotFound(_))
    ));
    assert!(calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn sdk_embedding_preserves_provider_errors_without_retrying_authorization_failures() {
    let (client, calls, _) = client(1.0, true);
    assert!(matches!(
        client.embedding("one", None).await,
        Err(SDKError::AuthError(_))
    ));
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn sdk_embedding_requires_embedding_capability_before_dispatch() {
    let (client, calls, router) = client(1.0, false);
    let provider = router
        .get_deployment("embeddings")
        .unwrap()
        .provider
        .clone();
    router.add_deployment(Deployment::new(
        "unsupported".into(),
        provider,
        "unadvertised-model".into(),
        "unsupported".into(),
    ));
    let error = client
        .embedding("one", Some("unsupported"))
        .await
        .expect_err("a model without the embedding capability must be rejected");
    // The shared router classifies a known model lacking a requested
    // capability as InvalidRequest, consistently with runtime-backed chat.
    assert!(
        matches!(&error, SDKError::InvalidRequest(message) if message.contains("Embeddings")),
        "unexpected capability error: {error:?}"
    );
    assert!(calls.lock().unwrap().is_empty());
}

#[cfg(feature = "gateway")]
async fn unary_admission_request(
    client: &LLMClient,
    router: &Arc<UnifiedRouter>,
    facade: &str,
    text: &str,
) -> Result<Option<u32>, String> {
    use litellm_rs::core::completion::{CompletionOptions, DefaultRouter, Router};
    use litellm_rs::sdk::types::{ChatOptions, Content, Message, Role, SdkChatRequest};
    match facade {
        "embedding" => {
            client
                .batch_embedding(&[text.to_string(), text.to_string()], None)
                .await
                .map_err(|error| error.to_string())?;
            Ok(None) // The embedding facade exposes vectors, not a usage DTO.
        }
        "sdk-chat" => {
            let response = client
                .chat_with_options(SdkChatRequest {
                    model: "public-model".into(),
                    messages: vec![Message {
                        role: Role::User,
                        content: Some(Content::Text(text.into())),
                        name: None,
                        tool_calls: None,
                        tool_call_id: None,
                    }],
                    options: ChatOptions {
                        max_tokens: Some(20),
                        ..Default::default()
                    },
                })
                .await
                .map_err(|error| error.to_string())?;
            Ok(Some(response.usage.total_tokens))
        }
        "default-router" => {
            let response = DefaultRouter::from_runtime(RuntimeBinding::new(router.clone()))
                .complete(
                    "public-model",
                    ChatRequest::new("public-model")
                        .add_user_message(text)
                        .messages,
                    CompletionOptions {
                        max_tokens: Some(20),
                        ..Default::default()
                    },
                )
                .await
                .map_err(|error| error.to_string())?;
            Ok(response.usage.map(|usage| usage.total_tokens))
        }
        _ => unreachable!("only the three runtime unary facades are exercised"),
    }
}

#[cfg(feature = "gateway")]
#[tokio::test]
async fn runtime_unary_facades_reserve_estimates_and_preserve_unknown_usage() {
    use litellm_rs::config::models::storage::RedisConfig;
    use litellm_rs::storage::redis::RedisPool;
    use litellm_rs::core::router::{DeploymentConfig, RouterConfig};
    use litellm_rs::utils::ai::counter::token_counter::{TokenCounter, TokenizerIdentity};

    let Ok(url) = std::env::var("REDIS_URL") else {
        assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
        return;
    };
    let pool = Arc::new(
        RedisPool::new(&RedisConfig {
            url: url.clone(),
            enabled: true,
            allow_degraded: false,
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let text = "a".repeat(80);
    for facade in ["embedding", "sdk-chat", "default-router"] {
        for usage in [None, Some(0), Some(3)] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let dispatches = Arc::new(AtomicUsize::new(0));
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let provider = Provider::External(Arc::new(EmbeddingProvider {
                models: vec![ModelInfo {
                    id: "vendor/unary".into(),
                    capabilities: vec![
                        ProviderCapability::Embeddings,
                        ProviderCapability::ChatCompletion,
                    ],
                    ..Default::default()
                }],
                calls,
                marker: 1.0,
                fail: false,
                usage,
                gate: Some(gate.clone()),
                dispatches: dispatches.clone(),
            }));
            let router = Arc::new(
                UnifiedRouter::new(RouterConfig {
                    num_retries: 0,
                    max_fallbacks: 0,
                    ..Default::default()
                })
                .with_admission_redis(pool.clone()),
            );
            let id = uuid::Uuid::new_v4().to_string();
            router.add_deployment(
                Deployment::new(
                    id.clone(),
                    provider,
                    "vendor/unary".into(),
                    "public-model".into(),
                )
                .with_config(DeploymentConfig {
                    max_parallel_requests: Some(2),
                    rpm_limit: Some(100),
                    tpm_limit: Some(64),
                    ..Default::default()
                }),
            );
            let client = Arc::new(
                LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "public-model")
                    .unwrap(),
            );
            let worker_client = client.clone();
            let worker_router = router.clone();
            let worker_text = text.clone();
            let task = tokio::spawn(async move {
                unary_admission_request(&worker_client, &worker_router, facade, &worker_text).await
            });
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while dispatches.load(Ordering::Relaxed) != 1 {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("first provider request must have reserved admission");
            let key = format!("litellm-rs:admission:v1:{id}");
            let redis_client = redis::Client::open(url.as_str()).unwrap();
            let mut conn = redis_client
                .get_multiplexed_async_connection()
                .await
                .unwrap();
            let expected = if facade == "embedding" {
                u64::from(
                    TokenCounter::new()
                        .count_embedding_tokens(
                            &TokenizerIdentity::approximate("runtime", "public-model"),
                            &[text.clone(), text.clone()],
                        )
                        .unwrap()
                        .input_tokens,
                )
            } else {
                u64::from(
                    ChatRequest::new("public-model")
                        .add_user_message(&text)
                        .estimate_input_tokens(),
                ) + 20
            };
            let reserved: (i64, i64, i64) = redis::cmd("HMGET")
                .arg(&key)
                .arg(&["p", "r", "t"])
                .query_async(&mut conn)
                .await
                .unwrap();
            assert_eq!(reserved, (1, 1, expected as i64), "{facade}");
            assert!(expected > 32 && expected <= 64);
            assert!(
                unary_admission_request(&client, &router, facade, &text)
                    .await
                    .is_err(),
                "a second request must fail the TPM check before provider dispatch"
            );
            assert_eq!(dispatches.load(Ordering::Relaxed), 1);
            gate.add_permits(1);
            let displayed_usage = task.await.unwrap().unwrap();
            if facade == "sdk-chat" {
                // Its existing required usage DTO displays unknown as zero;
                // accounting must retain the original core Option independently.
                assert_eq!(displayed_usage, Some(usage.unwrap_or(0)));
            } else if facade == "default-router" {
                assert_eq!(displayed_usage, usage);
            }
            let final_state: (i64, i64, i64) = redis::cmd("HMGET")
                .arg(&key)
                .arg(&["p", "r", "t"])
                .query_async(&mut conn)
                .await
                .unwrap();
            assert_eq!(
                final_state,
                (0, 1, usage.map(i64::from).unwrap_or(expected as i64)),
                "{facade}, usage={usage:?}"
            );
            let fields: Vec<String> = redis::cmd("HKEYS")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .unwrap();
            assert!(!fields.iter().any(|field| field.starts_with("l:")));
            let deployment = router.get_deployment(&id).unwrap();
            assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
            assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
            assert_eq!(
                deployment.state.tpm_current.load(Ordering::Relaxed),
                u64::from(usage.unwrap_or(0))
            );
            if usage.is_none() {
                assert!(
                    unary_admission_request(&client, &router, facade, &text)
                        .await
                        .is_err(),
                    "unknown usage must keep the estimate for subsequent requests"
                );
                assert_eq!(dispatches.load(Ordering::Relaxed), 1);
            }
            redis::cmd("DEL")
                .arg(&key)
                .query_async::<i64>(&mut conn)
                .await
                .unwrap();
        }
    }
}
