//! Exercise completion through the public SDK stream, including its outer RAII owner.
use futures::{Stream, StreamExt, future::BoxFuture, stream::BoxStream};
use litellm_rs::{
    core::{
        providers::{ExternalProvider, Provider, ProviderError},
        router::{Deployment, RuntimeBinding, UnifiedRouter},
        types::{
            chat::ChatRequest,
            context::RequestContext,
            model::{ModelInfo, ProviderCapability},
            responses::{ChatChunk, ChatResponse},
        },
    },
    sdk::{LLMClient, errors::SDKError, types::Message},
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
    let router = Arc::new(UnifiedRouter::default());
    router.add_deployment(Deployment::new(
        "completion".into(),
        Provider::External(provider),
        "wire-model".into(),
        "public-model".into(),
    ));
    let client =
        LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "public-model").unwrap();
    (client, router, dropped)
}
fn request() -> Vec<Message> {
    vec![Message::user("hello")]
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
async fn sdk_completion_records_against_the_original_deployment_snapshot() {
    let (client, router, _) = fixture(vec![Ok(usage_chunk(1))]);
    let mut output = client.chat_stream(request()).await.unwrap();
    let original = router.get_deployment("completion").unwrap();
    let (_, replacement_router, _) = fixture(vec![]);
    router.add_deployment(
        replacement_router
            .get_deployment("completion")
            .unwrap()
            .as_ref()
            .clone(),
    );
    while let Some(chunk) = output.next().await {
        chunk.unwrap();
    }
    assert_counts(&original, 1, 0, 12);
    let replacement = router.get_deployment("completion").unwrap();
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
    let mut output = client.chat_stream(request()).await.unwrap();
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
    let mut output = client.chat_stream(request()).await.unwrap();
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
