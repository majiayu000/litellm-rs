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
    empty_responses: AtomicUsize,
    response_indices: Mutex<Option<Vec<usize>>>,
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
            let count = if self
                .empty_responses
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                0
            } else {
                count
            };
            // Upstreams may return entries out of order; preserve input order
            // in the SDK batch result by respecting each entry's index.
            let indices = self
                .response_indices
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| (0..count).rev().collect());
            let data: Vec<_> = indices
                .into_iter()
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
        empty_responses: AtomicUsize::new(0),
        response_indices: Mutex::new(None),
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

async fn request_embedding_vectors(
    client: &LLMClient,
    texts: &[String],
    batch: bool,
) -> Result<Vec<Vec<f32>>, SDKError> {
    if batch {
        client.batch_embedding(texts, None).await
    } else {
        client
            .embedding(&texts[0], None)
            .await
            .map(|value| vec![value])
    }
}

#[tokio::test]
async fn sdk_empty_embedding_data_retries_before_recording_success() {
    use litellm_rs::core::router::{
        DeploymentConfig, FallbackConfig, RetrySchedule, RouterConfig, UnifiedRoutingStrategy,
    };

    for batch in [false, true] {
        for fallback in [false, true] {
            for malformed_usage in [None, Some(0), Some(3)] {
                let make_provider = |marker, empty_responses, usage| {
                    Arc::new(EmbeddingProvider {
                        models: vec![ModelInfo {
                            id: "vendor/embed-v1".into(),
                            capabilities: vec![ProviderCapability::Embeddings],
                            ..Default::default()
                        }],
                        calls: Arc::new(Mutex::new(Vec::new())),
                        marker,
                        fail: false,
                        empty_responses: AtomicUsize::new(empty_responses),
                        response_indices: Mutex::new(None),
                        usage,
                        gate: None,
                        dispatches: Arc::new(AtomicUsize::new(0)),
                    })
                };
                let malformed = make_provider(1.0, 1, malformed_usage);
                let healthy = make_provider(2.0, 0, Some(3));
                let router = Arc::new(UnifiedRouter::new(RouterConfig {
                    routing_strategy: UnifiedRoutingStrategy::PriorityBased,
                    num_retries: u32::from(!fallback),
                    max_fallbacks: u32::from(fallback),
                    ..Default::default()
                }));
                for (id, provider, model, priority) in [
                    ("malformed", malformed.clone(), "primary", 0),
                    (
                        "healthy",
                        healthy.clone(),
                        if fallback { "backup" } else { "primary" },
                        1,
                    ),
                ] {
                    router.add_deployment(
                        Deployment::new(
                            id.into(),
                            Provider::External(provider),
                            "vendor/embed-v1".into(),
                            model.into(),
                        )
                        .with_config(DeploymentConfig {
                            priority,
                            max_parallel_requests: Some(1),
                            rpm_limit: Some(2),
                            tpm_limit: Some(if batch { 64 } else { 32 }),
                            retry_schedule: Some(RetrySchedule {
                                base_delay_ms: 0,
                                max_delay_ms: 0,
                                backoff_multiplier: 1.0,
                                jitter_ratio: 0.0,
                            }),
                            ..Default::default()
                        }),
                    );
                }
                if fallback {
                    router.set_fallback_config(
                        FallbackConfig::new().add_general("primary", vec!["backup".into()]),
                    );
                }
                let client =
                    LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "primary")
                        .unwrap();
                let texts = vec!["a".repeat(80); if batch { 2 } else { 1 }];
                let expected = |marker: f32| {
                    (0..texts.len())
                        .map(|index| vec![marker, index as f32])
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    request_embedding_vectors(&client, &texts, batch)
                        .await
                        .unwrap(),
                    expected(2.0),
                    "batch={batch}, fallback={fallback}, malformed_usage={malformed_usage:?}"
                );
                assert_eq!(malformed.dispatches.load(Ordering::Relaxed), 1);
                assert_eq!(healthy.dispatches.load(Ordering::Relaxed), 1);
                let bad = router.get_deployment("malformed").unwrap();
                let good = router.get_deployment("healthy").unwrap();
                for (deployment, successes, failures, tokens) in [
                    (&bad, 0, 1, u64::from(malformed_usage.unwrap_or(0))),
                    (&good, 1, 0, 3),
                ] {
                    assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
                    assert_eq!(
                        deployment.state.success_requests.load(Ordering::Relaxed),
                        successes
                    );
                    assert_eq!(
                        deployment.state.fail_requests.load(Ordering::Relaxed),
                        failures
                    );
                    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
                    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), tokens);
                    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                }

                // Each estimate exceeds half the TPM limit. Unknown completed
                // usage must retain that quota, so the next call uses the healthy
                // candidate. Known usage (including zero) replaces the estimate
                // and allows the formerly malformed candidate to serve again.
                assert_eq!(
                    request_embedding_vectors(&client, &texts, batch)
                        .await
                        .unwrap(),
                    expected(if malformed_usage.is_some() { 1.0 } else { 2.0 })
                );
                let known = usize::from(malformed_usage.is_some());
                assert_eq!(malformed.dispatches.load(Ordering::Relaxed), 1 + known);
                assert_eq!(healthy.dispatches.load(Ordering::Relaxed), 2 - known);
                assert_eq!(
                    bad.state.total_requests.load(Ordering::Relaxed),
                    (1 + known) as u64
                );
                assert_eq!(
                    bad.state.success_requests.load(Ordering::Relaxed),
                    known as u64
                );
                assert_eq!(bad.state.fail_requests.load(Ordering::Relaxed), 1);
                assert_eq!(
                    bad.state.rpm_current.load(Ordering::Relaxed),
                    (1 + known) as u64
                );
                assert_eq!(
                    bad.state.tpm_current.load(Ordering::Relaxed),
                    2 * u64::from(malformed_usage.unwrap_or(0))
                );
                assert_eq!(bad.state.active_requests.load(Ordering::Relaxed), 0);
                for request in malformed.calls.lock().unwrap().iter() {
                    assert_eq!(request.input.to_vec(), texts);
                }
                assert_eq!(healthy.calls.lock().unwrap()[0].input.to_vec(), texts);
            }
        }
    }
}

#[tokio::test(start_paused = true)]
async fn sdk_embedding_cardinality_and_indices_retry_without_losing_usage() {
    use litellm_rs::core::router::{
        DeploymentConfig, FallbackConfig, RetrySchedule, RouterConfig, UnifiedRoutingStrategy,
    };

    // A nonempty response can still omit inputs, repeat an index, invent an
    // index, or return surplus vectors. The single-text API requires index 0.
    for (batch, indices) in [
        (true, vec![0]),
        (true, vec![1]),
        (true, vec![0, 0]),
        (true, vec![0, 2]),
        (true, vec![0, u32::MAX as usize]),
        (true, vec![0, 1, 2]),
        (false, vec![1]),
        (false, vec![0, 1]),
    ] {
        for fallback in [false, true] {
            for malformed_usage in [None, Some(0), Some(3)] {
                let make_provider = |marker, response_indices, usage| {
                    Arc::new(EmbeddingProvider {
                        models: vec![ModelInfo {
                            id: "vendor/embed-v1".into(),
                            capabilities: vec![ProviderCapability::Embeddings],
                            ..Default::default()
                        }],
                        calls: Arc::new(Mutex::new(Vec::new())),
                        marker,
                        fail: false,
                        empty_responses: AtomicUsize::new(0),
                        response_indices: Mutex::new(response_indices),
                        usage,
                        gate: None,
                        dispatches: Arc::new(AtomicUsize::new(0)),
                    })
                };
                let malformed = make_provider(1.0, Some(indices.clone()), malformed_usage);
                let healthy = make_provider(2.0, None, Some(3));
                let router = Arc::new(UnifiedRouter::new(RouterConfig {
                    routing_strategy: UnifiedRoutingStrategy::PriorityBased,
                    num_retries: u32::from(!fallback),
                    max_fallbacks: u32::from(fallback),
                    ..Default::default()
                }));
                for (id, provider, model, priority) in [
                    ("malformed", malformed.clone(), "primary", 0),
                    (
                        "healthy",
                        healthy.clone(),
                        if fallback { "backup" } else { "primary" },
                        1,
                    ),
                ] {
                    router.add_deployment(
                        Deployment::new(
                            id.into(),
                            Provider::External(provider),
                            "vendor/embed-v1".into(),
                            model.into(),
                        )
                        .with_config(DeploymentConfig {
                            priority,
                            max_parallel_requests: Some(1),
                            rpm_limit: Some(2),
                            tpm_limit: Some(if batch { 64 } else { 32 }),
                            retry_schedule: Some(RetrySchedule {
                                base_delay_ms: 0,
                                max_delay_ms: 0,
                                backoff_multiplier: 1.0,
                                jitter_ratio: 0.0,
                            }),
                            ..Default::default()
                        }),
                    );
                }
                if fallback {
                    router.set_fallback_config(
                        FallbackConfig::new().add_general("primary", vec!["backup".into()]),
                    );
                }
                let client =
                    LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "primary")
                        .unwrap();
                let texts = vec!["a".repeat(80); if batch { 2 } else { 1 }];
                let expected = |marker: f32| {
                    (0..texts.len())
                        .map(|index| vec![marker, index as f32])
                        .collect::<Vec<_>>()
                };
                // The healthy provider deliberately returns indices in reverse
                // order. A valid permutation must succeed in input order.
                assert_eq!(
                    request_embedding_vectors(&client, &texts, batch)
                        .await
                        .unwrap(),
                    expected(2.0),
                    "batch={batch}, indices={indices:?}, fallback={fallback}, usage={malformed_usage:?}"
                );
                let bad = router.get_deployment("malformed").unwrap();
                let good = router.get_deployment("healthy").unwrap();
                for (deployment, successes, failures, tokens) in [
                    (&bad, 0, 1, u64::from(malformed_usage.unwrap_or(0))),
                    (&good, 1, 0, 3),
                ] {
                    assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
                    assert_eq!(
                        deployment.state.success_requests.load(Ordering::Relaxed),
                        successes
                    );
                    assert_eq!(
                        deployment.state.fail_requests.load(Ordering::Relaxed),
                        failures
                    );
                    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
                    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), tokens);
                    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                }

                // Each input estimate exceeds half the configured TPM limit.
                // A malformed completed response still consumes known usage or
                // conservatively retains unknown usage before retrying.
                let known = usize::from(malformed_usage.is_some());
                assert_eq!(
                    request_embedding_vectors(&client, &texts, batch)
                        .await
                        .unwrap(),
                    expected(if known == 1 { 1.0 } else { 2.0 })
                );
                assert_eq!(malformed.dispatches.load(Ordering::Relaxed), 1 + known);
                assert_eq!(healthy.dispatches.load(Ordering::Relaxed), 2 - known);
                assert_eq!(
                    bad.state.success_requests.load(Ordering::Relaxed),
                    known as u64
                );
                assert_eq!(bad.state.fail_requests.load(Ordering::Relaxed), 1);
                for (deployment, requests, tokens) in [
                    (&bad, 1 + known, 2 * u64::from(malformed_usage.unwrap_or(0))),
                    (&good, 2 - known, 3 * (2 - known) as u64),
                ] {
                    assert_eq!(
                        deployment.state.total_requests.load(Ordering::Relaxed),
                        requests as u64
                    );
                    assert_eq!(
                        deployment.state.rpm_current.load(Ordering::Relaxed),
                        requests as u64
                    );
                    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), tokens);
                    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                }
                for provider in [&malformed, &healthy] {
                    for request in provider.calls.lock().unwrap().iter() {
                        assert_eq!(request.input.to_vec(), texts);
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn sdk_empty_embedding_inputs_preserve_existing_output_contract() {
    let (client, calls, router) = client(1.0, false);
    assert!(client.batch_embedding(&[], None).await.unwrap().is_empty());
    assert_eq!(client.embedding("", None).await.unwrap(), [1.0, 0.0]);
    assert_eq!(calls.lock().unwrap().len(), 2);
    let deployment = router.get_deployment("embeddings").unwrap();
    assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 2);
    assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn sdk_malformed_embedding_terminal_response_preserves_usage() {
    use litellm_rs::core::router::{DeploymentConfig, RouterConfig};
    use litellm_rs::utils::ai::counter::token_counter::{TokenCounter, TokenizerIdentity};

    for shared in [
        false,
        #[cfg(feature = "gateway")]
        true,
    ] {
        #[cfg(feature = "gateway")]
        let pool = if shared {
            use litellm_rs::config::models::storage::RedisConfig;
            use litellm_rs::storage::redis::RedisPool;
            let Ok(url) = std::env::var("REDIS_URL") else {
                assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
                continue;
            };
            Some(Arc::new(
                RedisPool::new(&RedisConfig {
                    url,
                    enabled: true,
                    allow_degraded: false,
                    ..Default::default()
                })
                .await
                .unwrap(),
            ))
        } else {
            None
        };

        for usage in [None, Some(0), Some(3)] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let provider = Arc::new(EmbeddingProvider {
                models: vec![ModelInfo {
                    id: "vendor/embed-v1".into(),
                    capabilities: vec![ProviderCapability::Embeddings],
                    ..Default::default()
                }],
                calls: calls.clone(),
                marker: 1.0,
                fail: false,
                empty_responses: AtomicUsize::new(usize::MAX),
                response_indices: Mutex::new(None),
                usage,
                gate: None,
                dispatches: Arc::new(AtomicUsize::new(0)),
            });
            let router = UnifiedRouter::new(RouterConfig {
                num_retries: 0,
                max_fallbacks: 0,
                ..Default::default()
            });
            #[cfg(feature = "gateway")]
            let router = match &pool {
                Some(pool) => router.with_admission_redis(pool.clone()),
                None => router,
            };
            let router = Arc::new(router);
            let id = uuid::Uuid::new_v4().to_string();
            router.add_deployment(
                Deployment::new(
                    id.clone(),
                    Provider::External(provider.clone()),
                    "vendor/embed-v1".into(),
                    "public-model".into(),
                )
                .with_config(DeploymentConfig {
                    max_parallel_requests: Some(1),
                    rpm_limit: Some(2),
                    tpm_limit: Some(32),
                    ..Default::default()
                }),
            );
            let client =
                LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "public-model")
                    .unwrap();
            let text = "a".repeat(80);
            let estimate = u64::from(
                TokenCounter::new()
                    .count_embedding_tokens(
                        &TokenizerIdentity::approximate("runtime", "public-model"),
                        std::slice::from_ref(&text),
                    )
                    .unwrap()
                    .input_tokens,
            );
            assert!(estimate > 16 && estimate <= 32);
            let error = client.embedding(&text, None).await.unwrap_err();
            assert!(
                matches!(error, SDKError::Unavailable(ref message) if message.contains("No embedding data in response")),
                "shared={shared}, usage={usage:?}, error={error:?}"
            );
            let deployment = router.get_deployment(&id).unwrap();
            assert_eq!(calls.lock().unwrap().len(), 1);
            assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
            assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 0);
            assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
            assert_eq!(
                deployment.state.tpm_current.load(Ordering::Relaxed),
                u64::from(usage.unwrap_or(0))
            );
            #[cfg(feature = "gateway")]
            if pool.is_some() {
                let key = format!("litellm-rs:admission:v1:{id}");
                let redis_client =
                    redis::Client::open(std::env::var("REDIS_URL").unwrap()).unwrap();
                let mut conn = redis_client
                    .get_multiplexed_async_connection()
                    .await
                    .unwrap();
                let counters: (i64, i64, i64) = redis::cmd("HMGET")
                    .arg(&key)
                    .arg(&["p", "r", "t"])
                    .query_async(&mut conn)
                    .await
                    .unwrap();
                assert_eq!(counters, (0, 1, usage.map_or(estimate, u64::from) as i64));
                let fields: Vec<String> = redis::cmd("HKEYS")
                    .arg(&key)
                    .query_async(&mut conn)
                    .await
                    .unwrap();
                assert!(!fields.iter().any(|field| field.starts_with("l:")));
            }

            // Completed unknown usage consumes the original estimate, without
            // presenting it as actual TPM. Once the provider would return data,
            // that retained quota must still deny the same input before dispatch.
            provider.empty_responses.store(0, Ordering::Relaxed);
            let next = client.embedding(&text, None).await;
            match usage {
                None => {
                    assert!(next.is_err(), "unknown usage must retain admission quota");
                    assert_eq!(calls.lock().unwrap().len(), 1);
                    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
                    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
                }
                Some(tokens) => {
                    assert_eq!(next.unwrap(), [1.0, 0.0]);
                    assert_eq!(calls.lock().unwrap().len(), 2);
                    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 2);
                    assert_eq!(
                        deployment.state.tpm_current.load(Ordering::Relaxed),
                        2 * u64::from(tokens)
                    );
                }
            }
            assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
            #[cfg(feature = "gateway")]
            if let Some(pool) = &pool {
                pool.delete(&format!("litellm-rs:admission:v1:{id}"))
                    .await
                    .unwrap();
            }
        }
    }
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

#[tokio::test]
async fn runtime_unary_facades_enforce_local_tpm_without_counting_estimates_as_actual() {
    use litellm_rs::core::router::{DeploymentConfig, RouterConfig};
    use std::time::Duration;

    let text = "a".repeat(80);
    for facade in ["embedding", "sdk-chat", "default-router"] {
        for usage in [None, Some(0), Some(3)] {
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
                calls: Arc::new(Mutex::new(Vec::new())),
                marker: 1.0,
                fail: false,
                empty_responses: AtomicUsize::new(0),
                response_indices: Mutex::new(None),
                usage,
                gate: Some(gate.clone()),
                dispatches: dispatches.clone(),
            }));
            let router = Arc::new(UnifiedRouter::new(RouterConfig {
                num_retries: 0,
                max_fallbacks: 0,
                ..Default::default()
            }));
            let id = uuid::Uuid::new_v4().to_string();
            router.add_deployment(
                Deployment::new(
                    id.clone(),
                    provider,
                    "vendor/unary".into(),
                    "public-model".into(),
                )
                .with_config(DeploymentConfig {
                    max_parallel_requests: None,
                    rpm_limit: None,
                    tpm_limit: Some(64),
                    ..Default::default()
                }),
            );
            let deployment = router.get_deployment(&id).unwrap();
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
            tokio::time::timeout(Duration::from_secs(3), async {
                while dispatches.load(Ordering::Relaxed) != 1 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("first provider call must start after local admission");
            assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
            assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);

            let denied = tokio::time::timeout(
                Duration::from_secs(3),
                unary_admission_request(&client, &router, facade, &text),
            )
            .await
            .expect("TPM rejection must return without waiting at the provider");
            assert!(denied.is_err(), "{facade}, usage={usage:?}");
            assert_eq!(dispatches.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);

            gate.add_permits(1);
            let displayed_usage = tokio::time::timeout(Duration::from_secs(3), task)
                .await
                .expect("the accepted request must finish after the provider is released")
                .unwrap()
                .unwrap();
            if facade == "sdk-chat" {
                assert_eq!(displayed_usage, Some(usage.unwrap_or(0)));
            } else if facade == "default-router" {
                assert_eq!(displayed_usage, usage);
            }
            assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
            assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
            assert_eq!(
                deployment.state.tpm_current.load(Ordering::Relaxed),
                u64::from(usage.unwrap_or(0))
            );

            if usage.is_none() {
                let denied = tokio::time::timeout(
                    Duration::from_secs(3),
                    unary_admission_request(&client, &router, facade, &text),
                )
                .await
                .expect("retained unknown usage must reject before provider dispatch");
                assert!(denied.is_err(), "{facade} must retain unknown local usage");
                assert_eq!(dispatches.load(Ordering::Relaxed), 1);
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
                assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
            } else {
                gate.add_permits(1);
                tokio::time::timeout(
                    Duration::from_secs(3),
                    unary_admission_request(&client, &router, facade, &text),
                )
                .await
                .expect("known usage must release the unused estimate")
                .unwrap();
                assert_eq!(dispatches.load(Ordering::Relaxed), 2);
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 2);
                assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 2);
                assert_eq!(
                    deployment.state.tpm_current.load(Ordering::Relaxed),
                    2 * u64::from(usage.unwrap_or(0))
                );
            }
        }
    }
}

#[cfg(feature = "gateway")]
#[tokio::test]
async fn runtime_unary_facades_reserve_estimates_and_preserve_unknown_usage() {
    use litellm_rs::config::models::storage::RedisConfig;
    use litellm_rs::core::router::{DeploymentConfig, RouterConfig};
    use litellm_rs::storage::redis::RedisPool;
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
                empty_responses: AtomicUsize::new(0),
                response_indices: Mutex::new(None),
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
