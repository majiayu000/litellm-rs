//! Exercise completion through the public SDK stream, including its outer RAII owner.
use futures::{Stream, StreamExt, future::BoxFuture, stream::BoxStream};
use litellm_rs::core::completion::{CompletionOptions, DefaultRouter, Router as CompletionRouter};
use litellm_rs::sdk::types::{ChatOptions, SdkChatRequest};
use litellm_rs::{
    core::{
        providers::{ExternalProvider, Provider, ProviderError},
        router::{Deployment, RouterConfig, RuntimeBinding, UnifiedRouter},
        types::{
            chat::ChatRequest,
            context::RequestContext,
            model::{ModelInfo, ProviderCapability},
            responses::{ChatChunk, ChatResponse},
        },
    },
    sdk::{
        LLMClient,
        errors::SDKError,
        types::{Content, Message, Role},
    },
};
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
};

#[derive(Debug)]
struct CompletionProvider {
    models: Vec<ModelInfo>,
    events: Mutex<Option<Vec<Result<ChatChunk, ProviderError>>>>,
    dropped: Arc<AtomicBool>,
}
struct TrackedStream {
    events: VecDeque<Result<ChatChunk, ProviderError>>,
    dropped: Arc<AtomicBool>,
}
impl Stream for TrackedStream {
    type Item = Result<ChatChunk, ProviderError>;
    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.events.pop_front())
    }
}
impl Drop for TrackedStream {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Relaxed);
    }
}
impl ExternalProvider for CompletionProvider {
    fn name(&self) -> &str {
        "sdk-completion-test"
    }
    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[ProviderCapability::ChatCompletionStream]
    }
    fn models(&self) -> &[ModelInfo] {
        &self.models
    }
    fn chat_completion(
        &self,
        _: ChatRequest,
        _: RequestContext,
    ) -> BoxFuture<'_, Result<ChatResponse, ProviderError>> {
        Box::pin(async {
            Err(ProviderError::invalid_request(
                "sdk-completion-test",
                "stream only",
            ))
        })
    }
    fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _: RequestContext,
    ) -> BoxFuture<'_, Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError>>
    {
        Box::pin(async move {
            assert_eq!(request.model, "wire-model");
            assert_eq!(request.stream_options.unwrap().include_usage, Some(true));
            Ok(Box::pin(TrackedStream {
                events: self.events.lock().unwrap().take().unwrap().into(),
                dropped: self.dropped.clone(),
            }) as BoxStream<'static, _>)
        })
    }
}
fn usage_chunk(created: i64) -> ChatChunk {
    serde_json::from_value(serde_json::json!({
        "id":"completion", "object":"chat.completion.chunk", "created":created,
        "model":"wire-model", "choices":[],
        "usage":{"prompt_tokens":4,"completion_tokens":8,"total_tokens":12}
    }))
    .unwrap()
}
fn fixture(
    events: Vec<Result<ChatChunk, ProviderError>>,
) -> (LLMClient, Arc<UnifiedRouter>, Arc<AtomicBool>) {
    let (client, router, provider) = fixture_with_config(events, RouterConfig::default());
    (client, router, provider.dropped.clone())
}
fn fixture_with_config(
    events: Vec<Result<ChatChunk, ProviderError>>,
    config: RouterConfig,
) -> (LLMClient, Arc<UnifiedRouter>, Arc<CompletionProvider>) {
    let dropped = Arc::new(AtomicBool::new(false));
    let provider = Arc::new(CompletionProvider {
        models: vec![ModelInfo {
            id: "wire-model".into(),
            capabilities: vec![ProviderCapability::ChatCompletionStream],
            ..Default::default()
        }],
        events: Mutex::new(Some(events)),
        dropped: dropped.clone(),
    });
    let router = Arc::new(UnifiedRouter::new(config));
    router.add_deployment(Deployment::new(
        "completion".into(),
        Provider::External(provider.clone()),
        "wire-model".into(),
        "public-model".into(),
    ));
    let client =
        LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "public-model").unwrap();
    (client, router, provider)
}
fn request() -> Vec<Message> {
    vec![Message {
        role: Role::User,
        content: Some(Content::Text("hello".into())),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    }]
}

fn bounded_request() -> SdkChatRequest {
    SdkChatRequest {
        model: "public-model".into(),
        messages: request(),
        options: ChatOptions {
            max_tokens: Some(20),
            ..Default::default()
        },
    }
}

#[tokio::test(flavor = "current_thread")]
async fn local_stream_admission_retains_unknown_output_without_inventing_actual_usage() {
    async fn start(
        client: &LLMClient,
        router: &Arc<UnifiedRouter>,
        default_facade: bool,
    ) -> Result<BoxStream<'static, Result<(), ()>>, String> {
        if default_facade {
            DefaultRouter::from_runtime(RuntimeBinding::new(router.clone()))
                .complete_stream(
                    "public-model",
                    ChatRequest::new("public-model")
                        .add_user_message("hello")
                        .messages,
                    CompletionOptions {
                        max_tokens: Some(20),
                        ..Default::default()
                    },
                )
                .await
                .map(|stream| {
                    stream
                        .map(|chunk| chunk.map(|_| ()).map_err(|_| ()))
                        .boxed()
                })
                .map_err(|error| error.to_string())
        } else {
            client
                .chat_stream_with_options(bounded_request())
                .await
                .map(|stream| {
                    stream
                        .map(|chunk| chunk.map(|_| ()).map_err(|_| ()))
                        .boxed()
                })
                .map_err(|error| error.to_string())
        }
    }

    for default_facade in [false, true] {
        let cases = [
            "unpolled",
            "heartbeat",
            "heartbeat-error",
            "drop",
            "eof",
            "error",
            "known",
            "known-error",
        ]
        .into_iter()
        .map(|case| (case, 0_u32))
        .chain(
            [
                "snapshot-drop",
                "snapshot-eof",
                "snapshot-error",
                "snapshot-final",
                "snapshot-zero",
            ]
            .into_iter()
            .flat_map(|case| [4, 40].map(|tokens| (case, tokens))),
        );
        for (case, snapshot) in cases {
            let mut events = vec![Ok(serde_json::from_value(serde_json::json!({
                "id":"local", "object":"chat.completion.chunk", "created":1,
                "model":"wire-model", "choices":[{"index":0,"delta":{
                    "content":if case.starts_with("heartbeat") { "" } else { "partial" }
                }}]
            }))
            .unwrap())];
            if matches!(case, "known" | "known-error") {
                events = vec![Ok(usage_chunk(1))];
            }
            if case.starts_with("snapshot-") {
                let mut chunk = usage_chunk(1);
                let usage = chunk.usage.as_mut().unwrap();
                usage.prompt_tokens = snapshot;
                usage.completion_tokens = 0;
                usage.total_tokens = snapshot;
                events.insert(0, Ok(chunk));
                if matches!(case, "snapshot-final" | "snapshot-zero") {
                    let mut chunk = usage_chunk(1);
                    if case == "snapshot-zero" {
                        let usage = chunk.usage.as_mut().unwrap();
                        usage.prompt_tokens = 0;
                        usage.completion_tokens = 0;
                        usage.total_tokens = 0;
                        chunk.choices = serde_json::from_value(serde_json::json!([
                            {"index":0,"delta":{"content":"final"}}
                        ]))
                        .unwrap();
                    }
                    events.push(Ok(chunk));
                }
            }
            let completed = matches!(
                case,
                "eof" | "known" | "snapshot-eof" | "snapshot-final" | "snapshot-zero"
            );
            let failed = matches!(
                case,
                "error" | "heartbeat-error" | "known-error" | "snapshot-error"
            );
            if failed {
                events.push(Err(ProviderError::Other {
                    provider: "sdk-completion-test",
                    message: "stream failed".into(),
                }));
            }
            let dropped = Arc::new(AtomicBool::new(false));
            let provider = Arc::new(CompletionProvider {
                models: vec![ModelInfo {
                    id: "wire-model".into(),
                    capabilities: vec![ProviderCapability::ChatCompletionStream],
                    ..Default::default()
                }],
                events: Mutex::new(Some(events)),
                dropped: dropped.clone(),
            });
            let router = Arc::new(UnifiedRouter::new(RouterConfig {
                num_retries: 0,
                max_fallbacks: 0,
                allowed_fails: 100,
                min_requests: 100,
                ..Default::default()
            }));
            let mut deployment = Deployment::new(
                "local-stream".into(),
                Provider::External(provider.clone()),
                "wire-model".into(),
                "public-model".into(),
            );
            // TPM alone must enforce both in-flight and retained unknown work.
            deployment.config.tpm_limit = Some(40);
            router.add_deployment(deployment);
            let client =
                LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "public-model")
                    .unwrap();
            let mut stream = start(&client, &router, default_facade).await.unwrap();
            *provider.events.lock().unwrap() = Some(Vec::new());
            assert!(
                start(&client, &router, default_facade).await.is_err(),
                "in-flight estimate must block a competing call"
            );
            let deployment = router.get_deployment("local-stream").unwrap();
            assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
            assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
            if case != "unpolled" {
                assert!(stream.next().await.unwrap().is_ok());
                if case.starts_with("snapshot-") {
                    assert!(stream.next().await.unwrap().is_ok());
                    if matches!(case, "snapshot-final" | "snapshot-zero") {
                        assert!(stream.next().await.unwrap().is_ok());
                    }
                }
                if completed {
                    assert!(stream.next().await.is_none());
                } else if failed {
                    assert!(stream.next().await.unwrap().is_err());
                }
            }
            drop(stream);
            let observed = if matches!(case, "known" | "known-error" | "snapshot-final") {
                12
            } else if case == "snapshot-zero" {
                0
            } else {
                u64::from(snapshot)
            };
            assert_counts(
                &deployment,
                u64::from(completed),
                u64::from(failed),
                observed,
            );
            let retained = matches!(case, "drop" | "eof" | "error")
                || matches!(case, "snapshot-drop" | "snapshot-eof" | "snapshot-error");
            let billable = retained
                || matches!(
                    case,
                    "known" | "known-error" | "snapshot-final" | "snapshot-zero"
                );
            assert_eq!(
                deployment.state.rpm_current.load(Ordering::Relaxed),
                u64::from(billable)
            );
            assert!(dropped.load(Ordering::Relaxed));
            assert!(
                !deployment.is_in_cooldown(),
                "the TPM assertion must not be explained by provider health"
            );
            *provider.events.lock().unwrap() = Some(Vec::new());
            let next = start(&client, &router, default_facade).await;
            assert_eq!(
                next.is_err(),
                retained,
                "facade={default_facade}, case={case}, snapshot={snapshot}"
            );
            drop(next);
            assert_counts(
                &deployment,
                u64::from(completed),
                u64::from(failed),
                observed,
            );
        }
    }
}

fn assert_counts(deployment: &Deployment, success: u64, failures: u64, tokens: u64) {
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    assert_eq!(
        deployment.state.success_requests.load(Ordering::Relaxed),
        success
    );
    assert_eq!(
        deployment.state.fail_requests.load(Ordering::Relaxed),
        failures
    );
    assert_eq!(
        deployment.state.total_requests.load(Ordering::Relaxed),
        success + failures
    );
    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), tokens);
}
#[tokio::test]
async fn exhausted_sdk_stream_releases_and_records_success_before_drop() {
    let (client, router, dropped) = fixture(vec![Ok(usage_chunk(1))]);
    let mut output = client.chat_stream(request()).await.unwrap();
    assert!(output.next().await.unwrap().is_ok());
    assert!(output.next().await.is_none());
    let deployment = router.get_deployment("completion").unwrap();
    assert_counts(&deployment, 1, 0, 12);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
    assert!(dropped.load(Ordering::Relaxed));
    assert!(output.next().await.is_none());
    drop(output);
    assert_counts(&deployment, 1, 0, 12);
}
#[tokio::test]
async fn partial_stream_failures_count_once_toward_local_breaker_min_requests() {
    for known_usage in [false, true] {
        let prefix = if known_usage {
            usage_chunk(1)
        } else {
            serde_json::from_value(serde_json::json!({
                "id":"partial", "object":"chat.completion.chunk", "created":1,
                "model":"wire-model", "choices":[{"index":0,"delta":{"content":"partial"}}]
            }))
            .unwrap()
        };
        let (client, router, provider) = fixture_with_config(
            vec![],
            RouterConfig {
                allowed_fails: 1,
                min_requests: 2,
                cooldown_time_secs: 60,
                ..Default::default()
            },
        );
        for attempt in 1..=2 {
            *provider.events.lock().unwrap() = Some(vec![
                Ok(prefix.clone()),
                Err(ProviderError::network(
                    "sdk-completion-test",
                    "stream failed",
                )),
            ]);
            let mut output = client.chat_stream(request()).await.unwrap();
            assert!(output.next().await.unwrap().is_ok());
            assert!(output.next().await.unwrap().is_err());
            let deployment = router.get_deployment("completion").unwrap();
            assert_counts(
                &deployment,
                0,
                attempt,
                if known_usage { 12 * attempt } else { 0 },
            );
            assert_eq!(
                deployment.state.rpm_current.load(Ordering::Relaxed),
                attempt
            );
            assert_eq!(
                deployment.is_in_cooldown(),
                attempt == 2,
                "known_usage={known_usage}: one failed request must count once toward min_requests"
            );
            assert!(output.next().await.is_none());
        }
    }
}

#[tokio::test]
async fn local_breaker_success_window_resets_with_billing_counters() {
    for reset in ["none", "explicit", "lazy"] {
        let (client, router, provider) = fixture_with_config(
            vec![Ok(usage_chunk(1))],
            RouterConfig {
                allowed_fails: 1,
                min_requests: 2,
                cooldown_time_secs: 60,
                ..Default::default()
            },
        );
        let mut successful = client.chat_stream(request()).await.unwrap();
        assert!(successful.next().await.unwrap().is_ok());
        assert!(successful.next().await.is_none());
        let deployment = router.get_deployment("completion").unwrap();
        assert_counts(&deployment, 1, 0, 12);
        // Rollover through a cloned handle must reset the shared window.
        let state = deployment.state.clone();
        match reset {
            "explicit" => state.reset_minute(),
            "lazy" => state.minute_reset_at.store(0, Ordering::Release),
            _ => {}
        }
        *provider.events.lock().unwrap() = Some(vec![
            Ok(usage_chunk(2)),
            Err(ProviderError::network(
                "sdk-completion-test",
                "stream failed",
            )),
        ]);
        let mut failed = client.chat_stream(request()).await.unwrap();
        assert!(failed.next().await.unwrap().is_ok());
        assert!(failed.next().await.unwrap().is_err());
        assert_counts(&deployment, 1, 1, if reset == "none" { 24 } else { 12 });
        assert_eq!(
            state.rpm_current.load(Ordering::Relaxed),
            if reset == "none" { 2 } else { 1 }
        );
        assert_eq!(
            deployment.is_in_cooldown(),
            reset == "none",
            "reset={reset}: only a success in the current window counts toward min_requests"
        );
    }
}

#[tokio::test]
async fn provider_error_retains_observed_usage_and_releases_before_error_is_returned() {
    let (client, router, dropped) = fixture(vec![
        Ok(usage_chunk(1)),
        Err(ProviderError::authentication(
            "sdk-completion-test",
            "denied",
        )),
    ]);
    let mut output = client.chat_stream(request()).await.unwrap();
    assert!(output.next().await.unwrap().is_ok());
    assert!(matches!(
        output.next().await.unwrap(),
        Err(SDKError::AuthError(_))
    ));
    assert_counts(&router.get_deployment("completion").unwrap(), 0, 1, 12);
    assert!(dropped.load(Ordering::Relaxed));
    assert!(output.next().await.is_none());
}
#[tokio::test]
async fn dropping_sdk_stream_before_first_poll_releases_without_outcome() {
    let (client, router, dropped) = fixture(vec![Ok(usage_chunk(1))]);
    let output = client.chat_stream(request()).await.unwrap();
    assert_eq!(
        router
            .get_deployment("completion")
            .unwrap()
            .state
            .active_requests
            .load(Ordering::Relaxed),
        1
    );
    drop(output);
    assert_counts(&router.get_deployment("completion").unwrap(), 0, 0, 0);
    assert!(dropped.load(Ordering::Relaxed));
}
#[tokio::test]
async fn dropping_sdk_stream_after_usage_preserves_usage_without_claiming_success() {
    let (client, router, dropped) = fixture(vec![Ok(usage_chunk(1))]);
    let mut output = client.chat_stream(request()).await.unwrap();
    assert!(output.next().await.unwrap().is_ok());
    drop(output);
    let deployment = router.get_deployment("completion").unwrap();
    assert_counts(&deployment, 0, 0, 12);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
    assert!(dropped.load(Ordering::Relaxed));
}
#[tokio::test]
async fn sdk_conversion_error_is_neutral_for_provider_health_and_retains_usage() {
    let mut invalid = usage_chunk(1);
    invalid.choices = serde_json::from_value(serde_json::json!([{
        "index": 0, "delta": { "thinking": { "content": "cannot be represented by SDK" } }
    }]))
    .unwrap();
    let (client, router, dropped) = fixture(vec![Ok(invalid)]);
    let mut output = client.chat_stream(request()).await.unwrap();
    assert!(matches!(
        output.next().await.unwrap(),
        Err(SDKError::NotSupported(_))
    ));
    assert_counts(&router.get_deployment("completion").unwrap(), 0, 0, 12);
    assert!(dropped.load(Ordering::Relaxed));
    assert!(output.next().await.is_none());
}
#[tokio::test]
async fn sdk_completion_keeps_original_snapshot_after_deployment_id_is_removed_and_reused() {
    let (client, router, _) = fixture(vec![Ok(usage_chunk(1))]);
    let mut output = client.chat_stream(request()).await.unwrap();
    let original = router.get_deployment("completion").unwrap();
    let (_, replacement_router, _) = fixture(vec![]);
    // In-place updates intentionally share the existing runtime counters.
    // Remove/re-add the ID to create a distinct current admission owner.
    assert!(router.remove_deployment("completion").is_some());
    router.add_deployment(
        replacement_router
            .get_deployment("completion")
            .unwrap()
            .as_ref()
            .clone(),
    );
    let replacement = router.get_deployment("completion").unwrap();
    assert_eq!(original.state.active_requests.load(Ordering::Relaxed), 1);
    assert_counts(&replacement, 0, 0, 0);
    while let Some(chunk) = output.next().await {
        chunk.unwrap();
    }
    assert_counts(&original, 1, 0, 12);
    assert!(!Arc::ptr_eq(&original, &replacement));
    assert_counts(&replacement, 0, 0, 0);
}

#[cfg(feature = "gateway")]
#[tokio::test(flavor = "current_thread")]
async fn sdk_outer_drop_settles_observed_usage_in_redis() {
    use litellm_rs::{config::models::storage::RedisConfig, storage::redis::RedisPool};
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
    let (old_client, router, dropped) = fixture(vec![Ok(usage_chunk(1))]);
    drop(old_client);
    let router = Arc::try_unwrap(router)
        .ok()
        .unwrap()
        .with_admission_redis(pool);
    let mut deployment = router
        .get_deployment("completion")
        .unwrap()
        .as_ref()
        .clone();
    deployment.id = uuid::Uuid::new_v4().to_string();
    deployment.config.max_parallel_requests = Some(1);
    deployment.config.tpm_limit = Some(100);
    let _ = router.remove_deployment("completion");
    let id = deployment.id.clone();
    router.add_deployment(deployment);
    let router = Arc::new(router);
    let client =
        LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "public-model").unwrap();
    let mut output = client
        .chat_stream_with_options(bounded_request())
        .await
        .unwrap();
    assert!(output.next().await.unwrap().is_ok());
    drop(output);
    assert_counts(&router.get_deployment(&id).unwrap(), 0, 0, 12);
    assert!(dropped.load(Ordering::Relaxed));
    let mut connection = redis::Client::open(url)
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let key = format!("litellm-rs:admission:v1:{id}");
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let state: (i64, i64) = redis::cmd("HMGET")
                .arg(&key)
                .arg(&["p", "t"])
                .query_async(&mut connection)
                .await
                .unwrap();
            if state == (0, 12) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    redis::cmd("DEL")
        .arg(key)
        .query_async::<i64>(&mut connection)
        .await
        .unwrap();
}

/// Pause only this test's Redis traffic after admission has succeeded. Other
/// tests sharing REDIS_URL continue normally; no global Redis pause is used.
#[cfg(feature = "gateway")]
struct SettlementProxy {
    url: String,
    paused: Arc<AtomicBool>,
    blocked: Arc<tokio::sync::Notify>,
    allow: Arc<tokio::sync::Semaphore>,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}
#[cfg(feature = "gateway")]
impl SettlementProxy {
    async fn new(redis_url: &str) -> Self {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut url = url::Url::parse(redis_url).unwrap();
        let host = url.host_str().unwrap().to_owned();
        let port = url.port().unwrap_or(6379);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        url.set_host(Some("127.0.0.1")).unwrap();
        url.set_port(Some(listener.local_addr().unwrap().port()))
            .unwrap();
        let paused = Arc::new(AtomicBool::new(false));
        let blocked = Arc::new(tokio::sync::Notify::new());
        let allow = Arc::new(tokio::sync::Semaphore::new(0));
        let (paused_task, blocked_task, allow_task) =
            (paused.clone(), blocked.clone(), allow.clone());
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                let (downstream, _) = listener.accept().await?;
                let upstream = tokio::net::TcpStream::connect((host.as_str(), port)).await?;
                let (paused, blocked, allow) = (
                    paused_task.clone(),
                    blocked_task.clone(),
                    allow_task.clone(),
                );
                connections.spawn(async move {
                    let (mut downstream_read, mut downstream_write) = downstream.into_split();
                    let (mut upstream_read, mut upstream_write) = upstream.into_split();
                    let forward = async {
                        let mut buffer = [0_u8; 8192];
                        loop {
                            let count = downstream_read.read(&mut buffer).await?;
                            if count == 0 {
                                return Ok::<(), std::io::Error>(());
                            }
                            if paused.load(Ordering::SeqCst) {
                                blocked.notify_one();
                                allow.acquire().await.unwrap().forget();
                            }
                            upstream_write.write_all(&buffer[..count]).await?;
                        }
                    };
                    tokio::try_join!(
                        forward,
                        tokio::io::copy(&mut upstream_read, &mut downstream_write)
                    )?;
                    Ok::<(), std::io::Error>(())
                });
            }
        });
        Self {
            url: url.to_string(),
            paused,
            blocked,
            allow,
            task,
        }
    }
}
#[cfg(feature = "gateway")]
impl Drop for SettlementProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(feature = "gateway")]
#[tokio::test(flavor = "current_thread")]
async fn cancelling_sdk_eof_during_redis_io_keeps_exactly_one_success_and_actual_usage() {
    use litellm_rs::{config::models::storage::RedisConfig, storage::redis::RedisPool};
    let Ok(url) = std::env::var("REDIS_URL") else {
        assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
        return;
    };
    let proxy = SettlementProxy::new(&url).await;
    let pool = Arc::new(
        RedisPool::new(&RedisConfig {
            url: proxy.url.clone(),
            enabled: true,
            allow_degraded: false,
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let (old_client, router, dropped) = fixture(vec![Ok(usage_chunk(1))]);
    drop(old_client);
    let router = Arc::try_unwrap(router)
        .ok()
        .unwrap()
        .with_admission_redis(pool);
    let mut deployment = router
        .get_deployment("completion")
        .unwrap()
        .as_ref()
        .clone();
    deployment.id = uuid::Uuid::new_v4().to_string();
    deployment.config.max_parallel_requests = Some(1);
    deployment.config.tpm_limit = Some(100);
    let _ = router.remove_deployment("completion");
    let id = deployment.id.clone();
    router.add_deployment(deployment);
    let router = Arc::new(router);
    let client =
        LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "public-model").unwrap();
    let mut output = client
        .chat_stream_with_options(bounded_request())
        .await
        .unwrap();
    assert!(output.next().await.unwrap().is_ok());
    proxy.paused.store(true, Ordering::SeqCst);
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let next = output.next();
        tokio::pin!(next);
        tokio::select! {
            _ = proxy.blocked.notified() => {}
            result = &mut next => panic!("EOF must wait for the paused settlement: {result:?}"),
        }
    })
    .await
    .unwrap();
    drop(output);
    let deployment = router.get_deployment(&id).unwrap();
    assert_counts(&deployment, 1, 0, 12);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
    assert!(dropped.load(Ordering::Relaxed));
    proxy.allow.add_permits(64);
    let mut connection = redis::Client::open(url)
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let key = format!("litellm-rs:admission:v1:{id}");
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let state: (i64, i64) = redis::cmd("HMGET")
                .arg(&key)
                .arg(&["p", "t"])
                .query_async(&mut connection)
                .await
                .unwrap();
            if state == (0, 12) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_counts(&deployment, 1, 0, 12);
    redis::cmd("DEL")
        .arg(key)
        .query_async::<i64>(&mut connection)
        .await
        .unwrap();
}

#[cfg(feature = "gateway")]
#[tokio::test(flavor = "current_thread")]
async fn sdk_unknown_usage_retains_only_consumed_output_or_completed_responses() {
    use litellm_rs::{config::models::storage::RedisConfig, storage::redis::RedisPool};
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
    let mut connection = redis::Client::open(url)
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    for default_facade in [false, true] {
        let cases = [
            "drop",
            "eof",
            "error",
            "heartbeat",
            "unpolled",
            "heartbeat-error",
            "known",
            "known-error",
        ]
        .into_iter()
        .map(|terminal| (terminal, 0_u64))
        .chain(
            [
                "snapshot-drop",
                "snapshot-eof",
                "snapshot-error",
                "snapshot-final",
                "snapshot-zero",
            ]
            .into_iter()
            .flat_map(|terminal| [4, 40].map(|tokens| (terminal, tokens))),
        );
        for (terminal, snapshot_tokens) in cases {
            let mut events = vec![Ok(serde_json::from_value(serde_json::json!({
            "id":"unknown", "object":"chat.completion.chunk", "created":1,
            "model":"wire-model", "choices":[{"index":0,"delta":{
                "role":"assistant", "content":if terminal.starts_with("heartbeat") { "" } else { "partial" }
            }}]
        }))
        .unwrap())];
            if terminal == "known" || terminal == "known-error" {
                events = vec![Ok(usage_chunk(1))];
            }
            if terminal.starts_with("snapshot-") {
                let mut snapshot = usage_chunk(1);
                snapshot.usage.as_mut().unwrap().total_tokens = snapshot_tokens as u32;
                events.insert(0, Ok(snapshot));
                if terminal == "snapshot-final" || terminal == "snapshot-zero" {
                    let mut final_chunk = usage_chunk(1);
                    if terminal == "snapshot-zero" {
                        final_chunk.usage.as_mut().unwrap().total_tokens = 0;
                        // Same-chunk actual zero covers this newly observed payload.
                        final_chunk.choices = serde_json::from_value(serde_json::json!([
                            {"index":0,"delta":{"content":"final"}}
                        ]))
                        .unwrap();
                    }
                    events.push(Ok(final_chunk));
                }
            }
            let completed = matches!(
                terminal,
                "eof" | "known" | "snapshot-eof" | "snapshot-final" | "snapshot-zero"
            );
            let failed = matches!(
                terminal,
                "error" | "heartbeat-error" | "snapshot-error" | "known-error"
            );
            if failed {
                events.push(Err(ProviderError::authentication(
                    "sdk-completion-test",
                    "denied",
                )));
            }
            let (old_client, router, dropped) = fixture(events);
            drop(old_client);
            let router = Arc::try_unwrap(router)
                .ok()
                .unwrap()
                .with_admission_redis(pool.clone());
            let mut deployment = router
                .get_deployment("completion")
                .unwrap()
                .as_ref()
                .clone();
            deployment.id = uuid::Uuid::new_v4().to_string();
            deployment.config.max_parallel_requests = Some(1);
            deployment.config.rpm_limit = Some(1);
            deployment.config.tpm_limit = Some(100);
            let _ = router.remove_deployment("completion");
            let id = deployment.id.clone();
            router.add_deployment(deployment);
            let router = Arc::new(router);
            let client =
                LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "public-model")
                    .unwrap();
            let mut output: BoxStream<'static, Result<(), ()>> = if default_facade {
                DefaultRouter::from_runtime(RuntimeBinding::new(router.clone()))
                    .complete_stream(
                        "public-model",
                        ChatRequest::new("public-model")
                            .add_user_message("hello")
                            .messages,
                        CompletionOptions {
                            max_tokens: Some(20),
                            ..Default::default()
                        },
                    )
                    .await
                    .unwrap()
                    .map(|result| {
                        result.map(|_| ()).map_err(|error| {
                            assert!(matches!(
                                error,
                                litellm_rs::utils::error::gateway_error::GatewayError::Provider(
                                    ProviderError::Authentication { .. }
                                )
                            ));
                        })
                    })
                    .boxed()
            } else {
                client
                    .chat_stream_with_options(bounded_request())
                    .await
                    .unwrap()
                    .map(|result| {
                        result.map(|_| ()).map_err(|error| {
                            assert!(matches!(error, SDKError::AuthError(_)));
                        })
                    })
                    .boxed()
            };
            let key = format!("litellm-rs:admission:v1:{id}");
            let reserved: (i64, i64, i64) = redis::cmd("HMGET")
                .arg(&key)
                .arg(&["p", "r", "t"])
                .query_async(&mut connection)
                .await
                .unwrap();
            assert_eq!((reserved.0, reserved.1), (1, 1));
            assert!(
                reserved.2 > 1,
                "admission must reserve the request estimate and output bound"
            );
            if terminal.starts_with("snapshot-") {
                assert!(
                    (4..40).contains(&reserved.2),
                    "snapshot matrix must straddle initial estimate: {reserved:?}"
                );
            }
            if terminal != "unpolled" {
                assert!(output.next().await.unwrap().is_ok());
                if terminal.starts_with("snapshot-") {
                    assert!(output.next().await.unwrap().is_ok());
                    if terminal == "snapshot-final" || terminal == "snapshot-zero" {
                        assert!(output.next().await.unwrap().is_ok());
                    }
                }
                if completed {
                    assert!(output.next().await.is_none());
                } else if failed {
                    assert!(output.next().await.unwrap().is_err());
                }
            }
            drop(output);
            let deployment = router.get_deployment(&id).unwrap();
            assert_counts(
                &deployment,
                u64::from(completed),
                u64::from(failed),
                if matches!(terminal, "known" | "known-error" | "snapshot-final") {
                    12
                } else if terminal == "snapshot-zero" {
                    0
                } else if terminal.starts_with("snapshot-") {
                    snapshot_tokens
                } else {
                    0
                },
            );
            let retain = matches!(terminal, "drop" | "eof" | "error" | "known" | "known-error")
                || terminal.starts_with("snapshot-");
            assert_eq!(
                deployment.state.rpm_current.load(Ordering::Relaxed),
                u64::from(retain)
            );
            assert!(dropped.load(Ordering::Relaxed));
            let expected = if matches!(terminal, "known" | "known-error" | "snapshot-final") {
                (0, 1, 12)
            } else if terminal == "snapshot-zero" {
                (0, 1, 0)
            } else if retain {
                (0, 1, reserved.2.max(snapshot_tokens as i64))
            } else {
                (0, 0, 0)
            };
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                loop {
                    let state: (i64, i64, i64) = redis::cmd("HMGET")
                        .arg(&key)
                        .arg(&["p", "r", "t"])
                        .query_async(&mut connection)
                        .await
                        .unwrap();
                    if state == expected {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            let fields: Vec<String> = redis::cmd("HKEYS")
                .arg(&key)
                .query_async(&mut connection)
                .await
                .unwrap();
            assert!(
                !fields.iter().any(|field| field.starts_with("l:")),
                "terminal cleanup must delete the lease"
            );
            if matches!(terminal, "drop" | "eof" | "snapshot-drop" | "snapshot-eof") {
                assert!(
                    matches!(
                        client.chat_stream_with_options(bounded_request()).await,
                        Err(SDKError::Unavailable(_))
                    ),
                    "retained RPM must block another admission"
                );
            }
            redis::cmd("DEL")
                .arg(key)
                .query_async::<i64>(&mut connection)
                .await
                .unwrap();
        }
    }
}
