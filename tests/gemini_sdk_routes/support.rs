use actix_web::{App, HttpRequest, HttpResponse, HttpServer, web};
use bytes::Bytes;
use litellm_rs::Config;
use litellm_rs::config::models::provider::ProviderConfig;
use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
use litellm_rs::core::net::ProviderEndpointAccess;
use litellm_rs::server::HttpServer as GatewayHttpServer;
use litellm_rs::server::state::AppState;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::provider_fixtures::{mock_provider_config, route_policy_bootstrap_providers};

#[derive(Clone, Debug)]
pub(crate) struct CapturedGeminiRequest {
    pub(crate) path_and_query: String,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) body: Vec<u8>,
}

#[derive(Clone)]
struct MockGeminiState {
    captured_requests: Arc<Mutex<Vec<CapturedGeminiRequest>>>,
}

pub(crate) struct MockGeminiServer {
    pub(crate) base_url: String,
    captured_requests: Arc<Mutex<Vec<CapturedGeminiRequest>>>,
    handle: actix_web::dev::ServerHandle,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl MockGeminiServer {
    pub(crate) async fn launch() -> Self {
        let captured_requests = Arc::new(Mutex::new(Vec::new()));
        let state = MockGeminiState {
            captured_requests: Arc::clone(&captured_requests),
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have address");
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(state.clone()))
                .default_service(web::post().to(mock_gemini))
        })
        .listen(listener)
        .expect("mock server should listen")
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        wait_for_server(address).await;

        Self {
            base_url: format!("http://{address}"),
            captured_requests,
            handle,
            task,
        }
    }

    pub(crate) fn requests(&self) -> Vec<CapturedGeminiRequest> {
        self.captured_requests.lock().unwrap().clone()
    }

    pub(crate) async fn shutdown(self) {
        self.handle.stop(true).await;
        let result = self.task.await.expect("mock server task should join");
        if let Err(error) = result {
            panic!("mock server should stop cleanly: {error}");
        }
    }
}

pub(crate) struct BrokenGeminiStreamServer {
    pub(crate) base_url: String,
    task: tokio::task::JoinHandle<()>,
}

impl BrokenGeminiStreamServer {
    pub(crate) async fn launch() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("broken stream server should bind");
        let address = listener
            .local_addr()
            .expect("broken stream server should have address");
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener
                .accept()
                .await
                .expect("broken stream server should accept");
            let mut buffer = [0_u8; 4096];
            let _ = socket.read(&mut buffer).await;
            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "content-type: text/event-stream\r\n",
                "content-length: 4096\r\n",
                "connection: close\r\n",
                "\r\n",
                "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"partial\"}]}}]}\n\n"
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("broken stream server should write partial response");
            let _ = socket.shutdown().await;
        });

        Self {
            base_url: format!("http://{address}"),
            task,
        }
    }

    pub(crate) async fn shutdown(self) {
        self.task
            .await
            .expect("broken stream server task should join");
    }
}

pub(crate) struct DelayedGeminiStreamServer {
    pub(crate) base_url: String,
    task: tokio::task::JoinHandle<()>,
}

impl DelayedGeminiStreamServer {
    pub(crate) async fn launch(delay: Duration) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("delayed stream server should bind");
        let address = listener
            .local_addr()
            .expect("delayed stream server should have address");
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener
                .accept()
                .await
                .expect("delayed stream server should accept");
            let mut buffer = [0_u8; 4096];
            let _ = socket.read(&mut buffer).await;
            socket
                .write_all(
                    concat!(
                        "HTTP/1.1 200 OK\r\n",
                        "content-type: text/event-stream\r\n",
                        "transfer-encoding: chunked\r\n",
                        "connection: close\r\n",
                        "\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .expect("delayed stream server should write headers");
            tokio::time::sleep(delay).await;
            let body =
                "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"delayed\"}]}}]}\n\n";
            socket
                .write_all(format!("{:x}\r\n{body}\r\n0\r\n\r\n", body.len()).as_bytes())
                .await
                .expect("delayed stream server should write body");
            let _ = socket.shutdown().await;
        });

        Self {
            base_url: format!("http://{address}"),
            task,
        }
    }

    pub(crate) async fn shutdown(self) {
        self.task
            .await
            .expect("delayed stream server task should join");
    }
}

async fn wait_for_server(address: std::net::SocketAddr) {
    for _ in 0..20 {
        if tokio::net::TcpStream::connect(address).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("mock server did not accept connections at {address}");
}

async fn mock_gemini(
    state: web::Data<MockGeminiState>,
    request: HttpRequest,
    body: Bytes,
) -> HttpResponse {
    let headers = request
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect();
    state
        .captured_requests
        .lock()
        .unwrap()
        .push(CapturedGeminiRequest {
            path_and_query: request
                .uri()
                .path_and_query()
                .expect("request should have path")
                .as_str()
                .to_string(),
            headers,
            body: body.to_vec(),
        });

    if request.path().ends_with(":streamGenerateContent") {
        let should_fail_stream = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|value| value.get("forceUpstreamError").and_then(Value::as_bool))
            .unwrap_or(false);
        if should_fail_stream {
            return HttpResponse::BadGateway()
                .insert_header(("content-type", "text/event-stream"))
                .body(format!(
                    "event: error\n\
                     data: {{\"error\":{{\"message\":\"upstream failed at {}\"}}}}\n\n",
                    request.uri()
                ));
        }
        return HttpResponse::Ok()
            .insert_header(("content-type", "text/event-stream"))
            .body(
                "event: message\n\
                 data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]}}]}\n\n\
                 event: message\n\
                 data: {\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":5,\"totalTokenCount\":15}}\n\n",
            );
    }

    let should_fail = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|value| value.get("forceUpstreamError").and_then(Value::as_bool))
        .unwrap_or(false);
    if should_fail {
        return HttpResponse::BadGateway().json(json!({
            "error": {
                "message": format!("upstream failed at {}", request.uri())
            }
        }));
    }

    HttpResponse::Ok().json(json!({
        "candidates": [{"content": {"parts": [{"text": "ok"}]}}],
        "usageMetadata": {
            "promptTokenCount": 10,
            "candidatesTokenCount": 5,
            "totalTokenCount": 15,
            "cachedContentTokenCount": 2,
            "thoughtsTokenCount": 1
        }
    }))
}

pub(crate) async fn build_test_state(providers: Vec<ProviderConfig>) -> AppState {
    let mut config = Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.providers = route_policy_bootstrap_providers(&providers);

    let state = GatewayHttpServer::new(&config)
        .await
        .expect("gateway server should initialize")
        .state()
        .clone();
    let mut runtime_config = state.config().as_ref().clone();
    runtime_config.gateway.providers = providers;
    state.config.store(runtime_config);
    state
}

pub(crate) async fn build_auth_required_state(providers: Vec<ProviderConfig>) -> AppState {
    let mut config = Config::default();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.providers = route_policy_bootstrap_providers(&providers);

    let state = GatewayHttpServer::new(&config)
        .await
        .expect("gateway server should initialize")
        .state()
        .clone();
    let mut runtime_config = state.config().as_ref().clone();
    runtime_config.gateway.providers = providers;
    state.config.store(runtime_config);
    state
}

pub(crate) fn gemini_provider(name: &str, base_url: &str, models: Vec<String>) -> ProviderConfig {
    let mut provider = mock_provider_config(
        name,
        "openai_compatible",
        "test-api-key-12345678901234567890",
        base_url,
        models,
    );
    provider.settings = HashMap::from([
        (
            "headers".to_string(),
            json!({
                "X-Base-Header": "base-value",
                "X-Ignored-Non-String": false
            }),
        ),
        (
            "custom_headers".to_string(),
            json!({
                "X-Custom-Header": "custom-value"
            }),
        ),
    ]);
    provider.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    provider
}

pub(crate) fn gemini_body() -> Value {
    json!({
        "contents": [{
            "role": "user",
            "parts": [{"text": "hello from the Gemini SDK"}]
        }],
        "generationConfig": {"maxOutputTokens": 8}
    })
}

pub(crate) fn gemini_body_without_generation_config() -> Value {
    json!({
        "contents": [{
            "role": "user",
            "parts": [{"text": "hello from the Gemini SDK"}]
        }]
    })
}

pub(crate) fn api_key_with_max_tokens_per_request(limit: u32) -> ApiKey {
    api_key_with_allowed_model_and_max_tokens("gemini-*", limit)
}

pub(crate) fn api_key_with_allowed_model_and_max_tokens(allowed_model: &str, limit: u32) -> ApiKey {
    let mut metadata = Metadata::new();
    metadata.extra.insert(
        "__core_keys".to_string(),
        json!({
            "permissions": {
                "allowed_models": [allowed_model],
                "allowed_endpoints": [],
                "max_tokens_per_request": limit,
                "custom_permissions": []
            }
        }),
    );
    ApiKey {
        metadata,
        name: "gemini-test-key".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-gemini".to_string(),
        user_id: None,
        team_id: None,
        permissions: Vec::new(),
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    }
}

pub(crate) fn api_key_with_invalid_runtime_permissions() -> ApiKey {
    let mut metadata = Metadata::new();
    metadata.extra.insert(
        "__core_keys".to_string(),
        json!({
            "permissions": {
                "allowed_models": "gemini-*"
            }
        }),
    );
    ApiKey {
        metadata,
        name: "gemini-invalid-policy-key".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-gemini-invalid".to_string(),
        user_id: None,
        team_id: None,
        permissions: Vec::new(),
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    }
}

pub(crate) fn gemini_upstream_error_body() -> Value {
    let mut body = gemini_body();
    body["forceUpstreamError"] = json!(true);
    body
}
