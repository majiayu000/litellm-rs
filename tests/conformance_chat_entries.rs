#![cfg(all(feature = "gateway", feature = "storage"))]
//! HTTP / SDK / `completion()` chat execution conformance (issue #1256).
//!
//! One serial harness so process-global `default_runtime()` cannot interleave.

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use futures::StreamExt;
use litellm_rs::config::models::provider::{ProviderConfig, RetryConfig};
use litellm_rs::core::net::ProviderEndpointAccess;
use litellm_rs::core::providers::ProviderError;
use litellm_rs::core::router::{
    Deployment, RuntimeBinding, UnifiedRouter, install_default_runtime, replace_default_runtime,
};
use litellm_rs::core::types::model::ProviderCapability;
use litellm_rs::sdk::LLMClient;
use litellm_rs::sdk::errors::SDKError;
use litellm_rs::sdk::types::{ChatOptions, Content, Message as SdkMessage, Role, SdkChatRequest};
use litellm_rs::server::HttpServer as GatewayHttpServer;
use litellm_rs::server::middleware::AuthMiddleware;
use litellm_rs::server::routes::ai::configure_routes;
use litellm_rs::{
    Config, GatewayError, RoutingStrategy, completion, completion_stream, user_message,
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::AbortHandle;

const GROUP: &str = "conformance-chat";
const REWRITE_UPSTREAM: &str = "gpt-4o-mini";
const NON_STREAM_UPSTREAM: &str = "gpt-5.4-pro";
const STREAM_FALLBACK_UPSTREAM: &str = "gpt-4o";
const MISSING_MODEL: &str = "missing-conformance-model";
const AUDIO_ONLY: &str = "audio-only";
const NO_STREAM_MODEL: &str = "non-stream-only";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ErrorKind {
    Authentication,
    InvalidRequest,
    ModelNotFound,
    Timeout,
    StreamFailed,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Outcome {
    attempts: Vec<(String, String)>,
    result: Result<(String, String), ErrorKind>,
}

#[derive(Clone, Copy)]
enum StreamMode {
    Normal,
    MidFail,
    HangAfterChunk,
}

struct MockCtl {
    attempts: Mutex<Vec<(String, String)>>,
    remaining_retryable: Mutex<HashMap<String, u32>>,
    non_retryable: Mutex<HashSet<String>>,
    stream_mode: Mutex<StreamMode>,
}

impl MockCtl {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            attempts: Mutex::new(Vec::new()),
            remaining_retryable: Mutex::new(HashMap::new()),
            non_retryable: Mutex::new(HashSet::new()),
            stream_mode: Mutex::new(StreamMode::Normal),
        })
    }

    fn reset(&self) {
        self.attempts.lock().unwrap().clear();
        self.remaining_retryable.lock().unwrap().clear();
        self.non_retryable.lock().unwrap().clear();
        *self.stream_mode.lock().unwrap() = StreamMode::Normal;
    }

    fn take_attempts(&self) -> Vec<(String, String)> {
        std::mem::take(&mut *self.attempts.lock().unwrap())
    }

    fn record(&self, deployment_id: String, model: String) {
        self.attempts.lock().unwrap().push((deployment_id, model));
    }

    fn fail_rewrite_retryable(&self) {
        self.remaining_retryable
            .lock()
            .unwrap()
            .insert("rewrite-chat".into(), 1);
    }

    fn fail_rewrite_auth(&self) {
        self.non_retryable
            .lock()
            .unwrap()
            .insert("rewrite-chat".into());
    }
}

fn fast_retry() -> RetryConfig {
    RetryConfig {
        base_delay: 1,
        max_delay: 5,
        backoff_multiplier: 2.0,
        jitter: 0.0,
    }
}

fn openai_cfg(name: &str, base: &str, model: &str, priority: u32) -> ProviderConfig {
    ProviderConfig {
        name: name.to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-conformance".to_string(),
        base_url: Some(base.to_string()),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        models: vec![model.to_string()],
        priority,
        retry: fast_retry(),
        ..Default::default()
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn read_http_request(socket: &mut TcpStream) -> io::Result<(String, Value)> {
    let mut buf = Vec::new();
    let mut tmp = [0_u8; 2048];
    loop {
        let n = socket.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_header_end(&buf) {
            let headers = String::from_utf8_lossy(&buf[..pos]);
            let path = headers
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("")
                .to_string();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            let mut body = buf[pos + 4..].to_vec();
            while body.len() < content_length {
                let n = socket.read(&mut tmp).await?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&tmp[..n]);
            }
            body.truncate(content_length);
            let json = serde_json::from_slice(&body).unwrap_or_else(|_| json!({}));
            return Ok((path, json));
        }
        if buf.len() > 64 * 1024 {
            break;
        }
    }
    Err(io::Error::other("incomplete mock request"))
}

fn unary_ok(model: &str) -> String {
    format!(
        r#"{{"id":"chatcmpl-conf","object":"chat.completion","created":1,"model":"{model}","choices":[{{"index":0,"message":{{"role":"assistant","content":"ok"}},"finish_reason":"stop"}}],"usage":{{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}}}"#
    )
}

fn sse_chunk(model: &str) -> String {
    format!(
        "data: {{\"id\":\"chatcmpl-conf\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"{model}\",\"choices\":[{{\"index\":0,\"delta\":{{\"role\":\"assistant\",\"content\":\"hi\"}},\"finish_reason\":null}}]}}\n\n"
    )
}

async fn write_http(socket: &mut TcpStream, status: u16, content_type: &str, body: &str) {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        408 => "Request Timeout",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
}

async fn handle_conn(mut socket: TcpStream, ctl: Arc<MockCtl>) {
    let Ok((path, body)) = read_http_request(&mut socket).await else {
        return;
    };
    let deployment_id = path
        .split('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    ctl.record(deployment_id.clone(), model.clone());

    if ctl.non_retryable.lock().unwrap().contains(&deployment_id) {
        write_http(&mut socket, 401, "text/plain", "unauthorized").await;
        return;
    }
    let retryable = {
        let mut remaining = ctl.remaining_retryable.lock().unwrap();
        if let Some(left) = remaining.get_mut(&deployment_id)
            && *left > 0
        {
            *left -= 1;
            true
        } else {
            false
        }
    };
    if retryable {
        write_http(&mut socket, 408, "text/plain", "timeout").await;
        return;
    }
    if !streaming {
        write_http(&mut socket, 200, "application/json", &unary_ok(&model)).await;
        return;
    }

    let mode = *ctl.stream_mode.lock().unwrap();
    let chunk = sse_chunk(&model);
    match mode {
        StreamMode::Normal => {
            let body = format!("{chunk}data: [DONE]\n\n");
            write_http(&mut socket, 200, "text/event-stream", &body).await;
        }
        StreamMode::MidFail => {
            let body = format!("{chunk}data: not-json\n\n");
            write_http(&mut socket, 200, "text/event-stream", &body).await;
        }
        StreamMode::HangAfterChunk => {
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{chunk}"
            );
            let _ = socket.write_all(headers.as_bytes()).await;
            let _ = socket.flush().await;
            // Keep the stream open until the client drops it. Do not substitute a
            // short timeout that would close the connection while the caller is
            // still supposed to be hanging.
            wait_for_client_disconnect(&mut socket).await;
        }
    }
}

async fn wait_for_client_disconnect(socket: &mut TcpStream) {
    let mut buf = [0_u8; 8];
    loop {
        match socket.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

struct MockServer {
    addr: String,
    accept: AbortHandle,
    connections: Arc<Mutex<Vec<AbortHandle>>>,
}

impl MockServer {
    fn stop(&self) {
        self.accept.abort();
        let handles: Vec<_> = std::mem::take(&mut *self.connections.lock().unwrap());
        for handle in handles {
            handle.abort();
        }
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn spawn_mock(ctl: Arc<MockCtl>) -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock should bind");
    let addr = listener.local_addr().expect("mock address").to_string();
    let connections = Arc::new(Mutex::new(Vec::new()));
    let connection_handles = connections.clone();
    let accept = tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                break;
            };
            let ctl = ctl.clone();
            let task = tokio::spawn(async move { handle_conn(socket, ctl).await });
            connection_handles.lock().unwrap().push(task.abort_handle());
        }
    });
    MockServer {
        addr,
        accept: accept.abort_handle(),
        connections,
    }
}

fn restore_deployments(router: &UnifiedRouter) {
    for id in router.list_deployments() {
        let Some(deployment) = router.get_deployment(&id) else {
            continue;
        };
        deployment.state.health.store(1, Ordering::Relaxed);
        deployment.state.active_requests.store(0, Ordering::Relaxed);
        deployment.state.fail_requests.store(0, Ordering::Relaxed);
        deployment
            .state
            .fails_this_minute
            .store(0, Ordering::Relaxed);
        deployment.state.cooldown_until.store(0, Ordering::Relaxed);
        deployment
            .state
            .success_requests
            .store(0, Ordering::Relaxed);
        deployment.state.total_requests.store(0, Ordering::Relaxed);
        deployment.state.tpm_current.store(0, Ordering::Relaxed);
        deployment.state.rpm_current.store(0, Ordering::Relaxed);
        deployment
            .state
            .consecutive_successes
            .store(0, Ordering::Relaxed);
    }
}

async fn wait_idle(router: &UnifiedRouter) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let busy: Vec<_> = router
            .list_deployments()
            .into_iter()
            .filter(|id| {
                router
                    .get_deployment(id)
                    .map(|d| d.state.active_requests.load(Ordering::Relaxed) != 0)
                    .unwrap_or(false)
            })
            .collect();
        if busy.is_empty() {
            return;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("leases still held on {busy:?}");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn factory_provider(router: &UnifiedRouter, prefix: &str) -> litellm_rs::core::providers::Provider {
    let needle = format!("{prefix}-");
    let id = router
        .list_deployments()
        .into_iter()
        .find(|id| id == prefix || id.starts_with(&needle))
        .unwrap_or_else(|| {
            panic!(
                "factory deployment '{prefix}' missing: {:?}",
                router.list_deployments()
            )
        });
    router
        .get_deployment(&id)
        .expect("factory deployment")
        .provider
        .clone()
}

fn regroup(router: &UnifiedRouter) {
    let ids = router.list_deployments();
    let template = router
        .get_deployment(ids.first().expect("factory deployments"))
        .expect("cfg")
        .config
        .clone();
    let with_priority = |priority: u32| {
        let mut config = template.clone();
        config.priority = priority;
        config
    };
    let mut non_chat = Deployment::new(
        "non-chat".into(),
        factory_provider(router, "non-chat"),
        "nova-3".into(),
        GROUP.into(),
    );
    non_chat.config = with_priority(0);
    let mut rewrite = Deployment::new(
        "rewrite-chat".into(),
        factory_provider(router, "rewrite-chat"),
        REWRITE_UPSTREAM.into(),
        GROUP.into(),
    );
    rewrite.config = with_priority(10);
    let mut non_stream = Deployment::new(
        "non-stream-chat".into(),
        factory_provider(router, "non-stream-chat"),
        NON_STREAM_UPSTREAM.into(),
        GROUP.into(),
    );
    non_stream.config = with_priority(20);
    let mut fallback = Deployment::new(
        "stream-fallback".into(),
        factory_provider(router, "stream-fallback"),
        STREAM_FALLBACK_UPSTREAM.into(),
        GROUP.into(),
    );
    fallback.config = with_priority(30);
    assert!(
        non_stream
            .provider
            .supports_capability_for_model(&non_stream.model, &ProviderCapability::ChatCompletion)
    );
    assert!(!non_stream.provider.supports_capability_for_model(
        &non_stream.model,
        &ProviderCapability::ChatCompletionStream
    ));
    let mut audio_only = Deployment::new(
        "audio-only".into(),
        factory_provider(router, "non-chat"),
        "nova-3".into(),
        AUDIO_ONLY.into(),
    );
    audio_only.config = with_priority(0);
    assert!(
        !audio_only
            .provider
            .supports_capability_for_model(&audio_only.model, &ProviderCapability::ChatCompletion)
    );
    let mut no_stream = Deployment::new(
        "no-stream-only".into(),
        factory_provider(router, "non-stream-chat"),
        NON_STREAM_UPSTREAM.into(),
        NO_STREAM_MODEL.into(),
    );
    no_stream.config = with_priority(0);
    assert!(!no_stream.provider.supports_capability_for_model(
        &no_stream.model,
        &ProviderCapability::ChatCompletionStream
    ));
    router.set_model_list(vec![
        non_chat, rewrite, non_stream, fallback, audio_only, no_stream,
    ]);
}

fn sdk_messages() -> Vec<SdkMessage> {
    vec![SdkMessage {
        role: Role::User,
        content: Some(Content::Text("hello".into())),
        name: None,
        tool_calls: None,
    }]
}

fn map_http(status: StatusCode, body: &Value) -> ErrorKind {
    let error_type = body
        .pointer("/error/type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let code = body
        .pointer("/error/code")
        .and_then(Value::as_str)
        .unwrap_or("");
    match (status.as_u16(), error_type, code) {
        (401, ..) | (_, "authentication_error", _) => ErrorKind::Authentication,
        (404, ..) | (_, _, "model_not_found") => ErrorKind::ModelNotFound,
        (400, ..) | (_, "invalid_request_error" | "invalid_request", _) => {
            ErrorKind::InvalidRequest
        }
        (408 | 504, ..) | (_, _, "timeout") => ErrorKind::Timeout,
        _ => ErrorKind::Other,
    }
}

fn sse_reports_error(text: &str) -> bool {
    if text.contains("event: error") {
        return true;
    }
    text.lines().any(|line| {
        let Some(data) = line.strip_prefix("data: ") else {
            return false;
        };
        serde_json::from_str::<Value>(data)
            .ok()
            .and_then(|value| value.get("error").cloned())
            .is_some()
    })
}

fn classify_http_stream(status: StatusCode, bytes: &[u8]) -> Result<(), ErrorKind> {
    let text = String::from_utf8_lossy(bytes);
    if !status.is_success() {
        let body: Value = serde_json::from_slice(bytes).unwrap_or(json!({}));
        return Err(map_http(status, &body));
    }
    if sse_reports_error(&text) || text.contains("not-json") {
        return Err(ErrorKind::StreamFailed);
    }
    if text.contains("data: [DONE]") {
        Ok(())
    } else {
        Err(ErrorKind::StreamFailed)
    }
}

fn map_sdk(err: &SDKError) -> ErrorKind {
    match err {
        SDKError::AuthError(_) => ErrorKind::Authentication,
        SDKError::ModelNotFound(_) => ErrorKind::ModelNotFound,
        SDKError::InvalidRequest(_) => ErrorKind::InvalidRequest,
        SDKError::NetworkError(_) => ErrorKind::Timeout,
        _ => ErrorKind::Other,
    }
}

fn map_completion(err: &GatewayError) -> ErrorKind {
    match err {
        GatewayError::Provider(ProviderError::Authentication { .. }) => ErrorKind::Authentication,
        GatewayError::Provider(ProviderError::ModelNotFound { .. }) => ErrorKind::ModelNotFound,
        GatewayError::Provider(ProviderError::InvalidRequest { .. }) => ErrorKind::InvalidRequest,
        GatewayError::Provider(ProviderError::Timeout { .. }) => ErrorKind::Timeout,
        _ => ErrorKind::Other,
    }
}

fn selected(attempts: &[(String, String)]) -> Result<(String, String), ErrorKind> {
    attempts.last().cloned().ok_or(ErrorKind::Other)
}

fn outcome_ok(attempts: Vec<(String, String)>) -> Outcome {
    Outcome {
        result: selected(&attempts),
        attempts,
    }
}

fn outcome_err(attempts: Vec<(String, String)>, kind: ErrorKind) -> Outcome {
    Outcome {
        attempts,
        result: Err(kind),
    }
}

fn assert_same_selection(case: &str, http: &Outcome, sdk: &Outcome, completion: &Outcome) {
    if http == sdk && sdk == completion {
        return;
    }
    let drifted = if sdk == completion {
        "http"
    } else if http == completion {
        "sdk"
    } else if http == sdk {
        "completion"
    } else {
        "http, sdk, and completion"
    };
    panic!(
        "conformance drift in {case}: {drifted} drifted\n  http: {http:?}\n  sdk: {sdk:?}\n  completion: {completion:?}"
    );
}

fn bind_runtime(router: &Arc<UnifiedRouter>) -> RuntimeBinding {
    let binding = RuntimeBinding::new(router.clone());
    if install_default_runtime(binding.clone()).is_err() {
        replace_default_runtime(binding.clone()).expect("replace default runtime");
    }
    binding
}

fn prepare(ctl: &MockCtl, router: &UnifiedRouter, binding: &RuntimeBinding) {
    restore_deployments(router);
    ctl.reset();
    replace_default_runtime(binding.clone()).expect("refresh default runtime");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sdk_and_completion_chat_entries_conform() {
    let ctl = MockCtl::new();
    let mock = spawn_mock(ctl.clone()).await;
    let addr = mock.addr.clone();
    let base = |id: &str| format!("http://{addr}/{id}/v1");

    let mut config = Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = Some("config/model_prices_extended.json".into());
    config.gateway.router.strategy = RoutingStrategy::PriorityBased;
    config.gateway.router.circuit_breaker.failure_threshold = 100;
    config.gateway.providers = vec![
        ProviderConfig {
            name: "non-chat".into(),
            provider_type: "deepgram".into(),
            api_key: "deepgram-test-key".into(),
            base_url: Some(base("non-chat")),
            endpoint_access: ProviderEndpointAccess::PrivateNetwork,
            models: vec!["nova-3".into()],
            priority: 0,
            retry: fast_retry(),
            ..Default::default()
        },
        openai_cfg("rewrite-chat", &base("rewrite-chat"), REWRITE_UPSTREAM, 10),
        openai_cfg(
            "non-stream-chat",
            &base("non-stream-chat"),
            NON_STREAM_UPSTREAM,
            20,
        ),
        openai_cfg(
            "stream-fallback",
            &base("stream-fallback"),
            STREAM_FALLBACK_UPSTREAM,
            30,
        ),
    ];

    let server = GatewayHttpServer::new(&config)
        .await
        .unwrap_or_else(|err| panic!("gateway test server: {err}"));
    let state = server.state().clone();
    regroup(&state.unified_router);
    let router = state.unified_router.clone();
    let binding = bind_runtime(&router);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .wrap(AuthMiddleware)
            .configure(configure_routes),
    )
    .await;

    let rewrite = ("rewrite-chat".to_string(), REWRITE_UPSTREAM.to_string());
    let unary_retry = vec![
        rewrite.clone(),
        ("non-stream-chat".into(), NON_STREAM_UPSTREAM.into()),
    ];
    let stream_retry = vec![
        rewrite.clone(),
        ("stream-fallback".into(), STREAM_FALLBACK_UPSTREAM.into()),
    ];
    let cases = [
        Case {
            name: "unary_capability_and_rewrite",
            stream: false,
            expected: vec![rewrite.clone()],
            ..Case::happy(GROUP)
        },
        Case {
            name: "stream_capability_and_rewrite",
            stream: true,
            expected: vec![rewrite.clone()],
            ..Case::happy(GROUP)
        },
        Case {
            name: "unary_retry_failover",
            setup: MockCtl::fail_rewrite_retryable,
            expected: unary_retry,
            ..Case::happy(GROUP)
        },
        Case {
            name: "stream_retry_failover",
            stream: true,
            setup: MockCtl::fail_rewrite_retryable,
            expected: stream_retry,
            ..Case::happy(GROUP)
        },
        Case {
            name: "unary_non_retryable",
            setup: MockCtl::fail_rewrite_auth,
            expected: vec![rewrite.clone()],
            expect_err: Some(ErrorKind::Authentication),
            ..Case::happy(GROUP)
        },
        Case {
            name: "stream_non_retryable",
            stream: true,
            setup: MockCtl::fail_rewrite_auth,
            expected: vec![rewrite.clone()],
            expect_err: Some(ErrorKind::Authentication),
            ..Case::happy(GROUP)
        },
        Case {
            name: "unary_unknown_model",
            expected: vec![],
            ..Case::happy(MISSING_MODEL)
        },
        Case {
            name: "stream_unknown_model",
            stream: true,
            expected: vec![],
            ..Case::happy(MISSING_MODEL)
        },
        Case {
            name: "unary_no_capability",
            expected: vec![],
            expect_err: Some(ErrorKind::InvalidRequest),
            ..Case::happy(AUDIO_ONLY)
        },
        Case {
            name: "stream_no_capability",
            stream: true,
            expected: vec![],
            expect_err: Some(ErrorKind::InvalidRequest),
            ..Case::happy(NO_STREAM_MODEL)
        },
        Case {
            name: "stream_mid_fail",
            stream: true,
            setup: set_mid_fail,
            expected: vec![rewrite.clone()],
            expect_err: Some(ErrorKind::StreamFailed),
            ..Case::happy(GROUP)
        },
        Case {
            name: "stream_drop_before_finish",
            stream: true,
            drop_after_start: true,
            setup: set_hang_after_chunk,
            expected: vec![rewrite],
            compare: false,
            ..Case::happy(GROUP)
        },
    ];
    for case in cases {
        run_case(&app, &ctl, &router, &binding, case).await;
    }
    mock.stop();
}

struct Case {
    name: &'static str,
    model: &'static str,
    stream: bool,
    drop_after_start: bool,
    setup: fn(&MockCtl),
    expected: Vec<(String, String)>,
    expect_err: Option<ErrorKind>,
    compare: bool,
}

impl Case {
    fn happy(model: &'static str) -> Self {
        Self {
            name: "",
            model,
            stream: false,
            drop_after_start: false,
            setup: noop,
            expected: Vec::new(),
            expect_err: None,
            compare: true,
        }
    }
}

fn noop(_: &MockCtl) {}

fn set_mid_fail(ctl: &MockCtl) {
    *ctl.stream_mode.lock().unwrap() = StreamMode::MidFail;
}

fn set_hang_after_chunk(ctl: &MockCtl) {
    *ctl.stream_mode.lock().unwrap() = StreamMode::HangAfterChunk;
}

async fn run_case(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
        Error = actix_web::Error,
    >,
    ctl: &MockCtl,
    router: &UnifiedRouter,
    binding: &RuntimeBinding,
    case: Case,
) {
    let invoke = || {
        prepare(ctl, router, binding);
        (case.setup)(ctl);
    };
    invoke();
    let http = drive_http(app, ctl, case.model, case.stream, case.drop_after_start).await;
    wait_idle(router).await;
    invoke();
    let sdk_out = drive_sdk(binding, ctl, case.model, case.stream, case.drop_after_start).await;
    wait_idle(router).await;
    invoke();
    let completion_out =
        drive_completion(ctl, case.model, case.stream, case.drop_after_start).await;
    wait_idle(router).await;
    assert_eq!(http.attempts, case.expected, "{} http attempts", case.name);
    if case.expected.is_empty() && case.expect_err.is_none() {
        assert!(
            matches!(
                http.result,
                Err(ErrorKind::ModelNotFound | ErrorKind::InvalidRequest)
            ),
            "{} unknown-model http error {:?}",
            case.name,
            http.result
        );
    }
    if case.compare {
        if let Some(kind) = case.expect_err {
            assert_eq!(http.result, Err(kind), "{} http error", case.name);
        }
        assert_same_selection(case.name, &http, &sdk_out, &completion_out);
    } else {
        assert_eq!(
            sdk_out.attempts, http.attempts,
            "{} sdk attempts",
            case.name
        );
        assert_eq!(
            completion_out.attempts, http.attempts,
            "{} completion attempts",
            case.name
        );
    }
}

async fn drive_http(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
        Error = actix_web::Error,
    >,
    ctl: &MockCtl,
    model: &str,
    stream: bool,
    drop_body: bool,
) -> Outcome {
    if stream {
        stream_http(app, ctl, model, drop_body).await
    } else {
        unary_http_raw(app, ctl, model).await
    }
}

async fn drive_sdk(
    binding: &RuntimeBinding,
    ctl: &MockCtl,
    model: &str,
    stream: bool,
    drop_after_start: bool,
) -> Outcome {
    let sdk = LLMClient::from_runtime(binding.clone(), model).expect("sdk runtime client");
    if stream {
        stream_sdk(&sdk, ctl, drop_after_start).await
    } else {
        unary_sdk(&sdk, ctl, model).await
    }
}

async fn drive_completion(
    ctl: &MockCtl,
    model: &str,
    stream: bool,
    drop_after_start: bool,
) -> Outcome {
    if stream {
        stream_completion(ctl, model, drop_after_start).await
    } else {
        unary_completion(ctl, model).await
    }
}

async fn unary_http_raw(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
        Error = actix_web::Error,
    >,
    ctl: &MockCtl,
    model: &str,
) -> Outcome {
    let req = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_json(json!({
            "model": model,
            "messages": [{"role":"user","content":"hello"}]
        }))
        .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let bytes = test::read_body(resp).await;
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
    let attempts = ctl.take_attempts();
    if status.is_success() {
        outcome_ok(attempts)
    } else {
        outcome_err(attempts, map_http(status, &body))
    }
}

async fn stream_http(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
        Error = actix_web::Error,
    >,
    ctl: &MockCtl,
    model: &str,
    drop_body: bool,
) -> Outcome {
    let req = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_json(json!({
            "model": model,
            "messages": [{"role":"user","content":"hello"}],
            "stream": true
        }))
        .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    if drop_body {
        drop(resp);
        return outcome_ok(ctl.take_attempts());
    }
    let bytes = test::read_body(resp).await;
    let attempts = ctl.take_attempts();
    match classify_http_stream(status, &bytes) {
        Ok(()) => outcome_ok(attempts),
        Err(kind) => outcome_err(attempts, kind),
    }
}

async fn unary_sdk(sdk: &LLMClient, ctl: &MockCtl, model: &str) -> Outcome {
    let request = SdkChatRequest {
        model: model.to_string(),
        messages: sdk_messages(),
        options: ChatOptions::default(),
    };
    match sdk.chat_with_options(request).await {
        Ok(_) => outcome_ok(ctl.take_attempts()),
        Err(err) => outcome_err(ctl.take_attempts(), map_sdk(&err)),
    }
}

async fn stream_sdk(sdk: &LLMClient, ctl: &MockCtl, drop_after_start: bool) -> Outcome {
    match sdk.chat_stream(sdk_messages()).await {
        Ok(mut stream) => {
            if drop_after_start {
                let _ = tokio::time::timeout(Duration::from_millis(200), stream.next()).await;
                drop(stream);
                return outcome_ok(ctl.take_attempts());
            }
            let mut failed = false;
            while let Some(item) = stream.next().await {
                if item.is_err() {
                    failed = true;
                    break;
                }
            }
            let attempts = ctl.take_attempts();
            if failed {
                outcome_err(attempts, ErrorKind::StreamFailed)
            } else {
                outcome_ok(attempts)
            }
        }
        Err(err) => outcome_err(ctl.take_attempts(), map_sdk(&err)),
    }
}

async fn unary_completion(ctl: &MockCtl, model: &str) -> Outcome {
    match completion(model, vec![user_message("hello")], None).await {
        Ok(_) => outcome_ok(ctl.take_attempts()),
        Err(err) => outcome_err(ctl.take_attempts(), map_completion(&err)),
    }
}

async fn stream_completion(ctl: &MockCtl, model: &str, drop_after_start: bool) -> Outcome {
    match completion_stream(model, vec![user_message("hello")], None).await {
        Ok(mut stream) => {
            if drop_after_start {
                let _ = tokio::time::timeout(Duration::from_millis(200), stream.next()).await;
                drop(stream);
                return outcome_ok(ctl.take_attempts());
            }
            let mut failed = false;
            while let Some(item) = stream.next().await {
                if item.is_err() {
                    failed = true;
                    break;
                }
            }
            let attempts = ctl.take_attempts();
            if failed {
                outcome_err(attempts, ErrorKind::StreamFailed)
            } else {
                outcome_ok(attempts)
            }
        }
        Err(err) => outcome_err(ctl.take_attempts(), map_completion(&err)),
    }
}
